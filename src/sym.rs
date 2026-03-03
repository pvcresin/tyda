//! Process-wide interner for identifiers. `Eq` is pointer comparison; `Hash`/`Ord` use the string content (same as the old String representation).
//! Unbounded payloads such as string-literal contents are not interned.
//!
//! Also holds the `::`-separator helpers ([`NamePath`], [`join_scope`]) that the
//! namespace walks use: `str`'s `&str`-pattern methods build a `TwoWaySearcher`
//! per call, which dominated the ancestor-resolution paths.
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::RwLock;

/// Byte-level equivalents of the `"::"` pattern searches on constant paths.
/// Each `str` method taking a `&str` pattern constructs a `TwoWaySearcher`
/// first, which is pure overhead for a two-byte needle.
pub trait NamePath {
    /// `self.trim_start_matches("::")`
    fn trim_scope_prefix(&self) -> &str;
    /// `self.rfind("::")`
    fn rfind_scope_sep(&self) -> Option<usize>;
    /// `self.contains("::")`
    fn contains_scope_sep(&self) -> bool;
}

impl NamePath for str {
    #[inline]
    fn trim_scope_prefix(&self) -> &str {
        let mut rest = self;
        // Bytes 0 and 1 are ASCII when they match, so byte 2 is a char boundary.
        while rest.as_bytes().starts_with(b"::") {
            match rest.get(2..) {
                Some(tail) => rest = tail,
                None => break,
            }
        }
        rest
    }

    #[inline]
    fn rfind_scope_sep(&self) -> Option<usize> {
        let bytes = self.as_bytes();
        let mut idx = bytes.len().checked_sub(2)?;
        loop {
            if bytes.get(idx) == Some(&b':') && bytes.get(idx + 1) == Some(&b':') {
                return Some(idx);
            }
            idx = idx.checked_sub(1)?;
        }
    }

    #[inline]
    fn contains_scope_sep(&self) -> bool {
        self.as_bytes()
            .windows(2)
            .any(|pair| pair == b"::".as_slice())
    }
}

/// `format!("{scope}::{name}")` with one exactly-sized allocation. The
/// `format!` machinery plus the `String` regrowth it causes was the largest
/// single allocation site in the CLI's per-file analysis phase.
#[inline]
pub fn join_scope(scope: &str, name: &str) -> String {
    let mut joined = String::with_capacity(scope.len() + 2 + name.len());
    joined.push_str(scope);
    joined.push_str("::");
    joined.push_str(name);
    joined
}

struct Entry(Box<str>);

impl Entry {
    #[inline]
    fn as_str(&'static self) -> &'static str {
        &self.0
    }
}

#[derive(Clone, Copy)]
struct EntryRef(&'static Entry);

impl Borrow<str> for EntryRef {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0.0
    }
}

impl PartialEq for EntryRef {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.0 == other.0.0
    }
}

impl Eq for EntryRef {}

impl std::hash::Hash for EntryRef {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.0.hash(state);
    }
}

// Sharded to reduce lock contention from parallel rayon workers; a string always
// hashes to the same shard, so pointer identity per unique string still holds.
const SHARD_COUNT: usize = 16;

// Fx, not the default SipHash: the set is membership-only (`interner_stats`
// aggregates order-independently), and `intern` already pays one Fx hash to
// pick the shard.
type Shard = RwLock<Option<HashSet<EntryRef, rustc_hash::FxBuildHasher>>>;

static SHARDS: [Shard; SHARD_COUNT] = [
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
    RwLock::new(None),
];

#[inline]
fn shard_for(s: &str) -> &'static Shard {
    let mut hasher = rustc_hash::FxHasher::default();
    hasher.write(s.as_bytes());
    let idx = (hasher.finish() as usize) % SHARD_COUNT;
    &SHARDS[idx]
}

fn intern(s: &str) -> &'static Entry {
    let shard = shard_for(s);
    {
        let guard = shard.read().unwrap_or_else(|e| e.into_inner());
        if let Some(set) = guard.as_ref()
            && let Some(existing) = set.get(s)
        {
            return existing.0;
        }
    }
    let mut guard = shard.write().unwrap_or_else(|e| e.into_inner());
    let set = guard.get_or_insert_with(HashSet::default);
    if let Some(existing) = set.get(s) {
        return existing.0;
    }
    let leaked: &'static Entry = Box::leak(Box::new(Entry(s.to_owned().into_boxed_str())));
    set.insert(EntryRef(leaked));
    leaked
}

/// Used for memory attribution.
pub fn interner_stats() -> (usize, usize) {
    let mut count = 0;
    let mut bytes = 0;
    for shard in &SHARDS {
        let guard = shard.read().unwrap_or_else(|e| e.into_inner());
        if let Some(set) = guard.as_ref() {
            count += set.len();
            bytes += set.iter().map(|e| e.0.0.len()).sum::<usize>();
        }
    }
    (count, bytes)
}

#[derive(Clone, Copy)]
pub struct Sym(&'static Entry);

impl Sym {
    pub fn new(s: impl AsRef<str>) -> Sym {
        Sym(intern(s.as_ref()))
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        self.0.as_str()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.0.is_empty()
    }
}

impl Deref for Sym {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Sym {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for Sym {
    #[inline]
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq for Sym {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Sym {}

impl std::hash::Hash for Sym {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for Sym {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Sym {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if std::ptr::eq(self.0, other.0) {
            return std::cmp::Ordering::Equal;
        }
        self.as_str().cmp(other.as_str())
    }
}

impl fmt::Display for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl From<&str> for Sym {
    fn from(s: &str) -> Sym {
        Sym::new(s)
    }
}

impl From<&String> for Sym {
    fn from(s: &String) -> Sym {
        Sym::new(s)
    }
}

impl From<String> for Sym {
    fn from(s: String) -> Sym {
        Sym::new(s)
    }
}

impl From<std::borrow::Cow<'_, str>> for Sym {
    fn from(s: std::borrow::Cow<'_, str>) -> Sym {
        Sym::new(s)
    }
}

impl From<Sym> for String {
    fn from(s: Sym) -> String {
        s.as_str().to_owned()
    }
}

impl PartialEq<str> for Sym {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Sym {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Sym {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Sym> for str {
    #[inline]
    fn eq(&self, other: &Sym) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Sym> for &str {
    #[inline]
    fn eq(&self, other: &Sym) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Sym> for String {
    #[inline]
    fn eq(&self, other: &Sym) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::Sym;

    #[test]
    fn interning_dedupes_equal_contents() {
        let a = Sym::new("Foo::Bar");
        let b = Sym::new(String::from("Foo::Bar"));
        assert_eq!(a, b);
        assert!(std::ptr::eq(a.as_str().as_ptr(), b.as_str().as_ptr()));
    }

    #[test]
    fn ordering_matches_string_ordering() {
        let a = Sym::new("Alpha");
        let b = Sym::new("Beta");
        assert!(a < b);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn hash_matches_str_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let sym = Sym::new("User");
        let mut h1 = DefaultHasher::new();
        sym.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        "User".hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn str_like_ergonomics() {
        let sym = Sym::new("MatchData[abc]");
        assert!(sym.starts_with("MatchData["));
        assert_eq!(sym, "MatchData[abc]");
        assert_eq!(sym.to_string(), "MatchData[abc]");
        assert_eq!(format!("{sym}"), "MatchData[abc]");
    }
}
