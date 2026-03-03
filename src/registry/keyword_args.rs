use crate::types::{SharedName, Type};

/// Keyword arguments observed at a call site.
///
/// Stored as a name-sorted, name-unique `Vec`: call sites carry 0-3 keywords in
/// the overwhelming majority of cases, so a `Vec` costs far less than a hash
/// table shell and binary search beats hashing at that size. The sort invariant
/// makes the derived `PartialEq` a set comparison, matching the `HashMap` this
/// replaced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeywordArgTypes {
    pairs: Vec<(SharedName, Type)>,
}

impl KeywordArgTypes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Type> {
        // `binary_search` is only sound while the sort invariant holds. The type is
        // immutable after construction, so this can only fire if a new mutation
        // path is added without re-sorting.
        debug_assert!(
            self.pairs.windows(2).all(|w| w[0].0 <= w[1].0),
            "keyword_arg_types must stay name-sorted for lookup"
        );
        self.pairs
            .binary_search_by(|probe| probe.0.as_ref().cmp(name))
            .ok()
            .map(|idx| &self.pairs[idx].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SharedName, &Type)> {
        self.pairs.iter().map(|(name, ty)| (name, ty))
    }

    pub fn keys(&self) -> impl Iterator<Item = &SharedName> {
        self.pairs.iter().map(|(name, _)| name)
    }

    pub fn values(&self) -> impl Iterator<Item = &Type> {
        self.pairs.iter().map(|(_, ty)| ty)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (SharedName, Type)> + '_ {
        self.pairs.drain(..)
    }

    pub fn shrink_to_fit(&mut self) {
        self.pairs.shrink_to_fit();
    }

    /// Name-sorted pairs — already the storage order.
    pub fn sorted_pairs(&self) -> &[(SharedName, Type)] {
        &self.pairs
    }

    pub fn shell_bytes(&self) -> usize {
        self.pairs.capacity() * std::mem::size_of::<(SharedName, Type)>()
    }
}

impl FromIterator<(SharedName, Type)> for KeywordArgTypes {
    fn from_iter<I: IntoIterator<Item = (SharedName, Type)>>(iter: I) -> Self {
        let mut pairs: Vec<(SharedName, Type)> = iter.into_iter().collect();
        // stable sort + keep the last of each name reproduces `HashMap::insert`'s
        // last-wins rule for repeated keywords.
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let mut deduped: Vec<(SharedName, Type)> = Vec::with_capacity(pairs.len());
        for pair in pairs {
            match deduped.last_mut() {
                Some(last) if last.0 == pair.0 => *last = pair,
                _ => deduped.push(pair),
            }
        }
        Self { pairs: deduped }
    }
}

impl IntoIterator for KeywordArgTypes {
    type Item = (SharedName, Type);
    type IntoIter = std::vec::IntoIter<(SharedName, Type)>;

    fn into_iter(self) -> Self::IntoIter {
        self.pairs.into_iter()
    }
}

impl<'a> IntoIterator for &'a KeywordArgTypes {
    type Item = (&'a SharedName, &'a Type);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (SharedName, Type)>,
        fn(&'a (SharedName, Type)) -> (&'a SharedName, &'a Type),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.pairs.iter().map(|(name, ty)| (name, ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> SharedName {
        SharedName::from(s)
    }

    #[test]
    fn duplicate_names_dedup_last_wins_like_hashmap() {
        let kw: KeywordArgTypes = vec![
            (name("a"), Type::Nil),
            (name("b"), Type::Untyped),
            (name("a"), Type::Bot),
        ]
        .into_iter()
        .collect();
        assert_eq!(kw.len(), 2);
        assert_eq!(kw.get("a"), Some(&Type::Bot));
        assert_eq!(kw.get("b"), Some(&Type::Untyped));
    }

    #[test]
    fn storage_is_name_sorted_and_lookups_hit() {
        let kw: KeywordArgTypes = vec![
            (name("zeta"), Type::Nil),
            (name("alpha"), Type::Bot),
            (name("mid"), Type::Untyped),
        ]
        .into_iter()
        .collect();
        let keys: Vec<_> = kw.keys().map(|k| k.as_ref().to_string()).collect();
        assert_eq!(keys, vec!["alpha", "mid", "zeta"]);
        assert_eq!(kw.get("mid"), Some(&Type::Untyped));
        assert_eq!(kw.get("absent"), None);
    }

    /// The sort invariant is what makes derived equality a set comparison.
    #[test]
    fn equality_ignores_insertion_order() {
        let a: KeywordArgTypes = vec![(name("x"), Type::Nil), (name("y"), Type::Bot)]
            .into_iter()
            .collect();
        let b: KeywordArgTypes = vec![(name("y"), Type::Bot), (name("x"), Type::Nil)]
            .into_iter()
            .collect();
        assert_eq!(a, b);
    }
}
