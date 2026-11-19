//! The registry's class map. Every mutable access moves the table to a new generation, so a
//! reader can tell that no class changed since it last looked by comparing generations.
//!
//! Classes are shared copy-on-write: a class merged unchanged from a shared declaration, or a
//! registry clone, points at the same data until one side writes to it.

use super::ClassData;
use crate::types::Sym;
use rustc_hash::FxHashMap;
use std::cell::Cell;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

type Map = FxHashMap<Sym, Arc<ClassData>>;

/// Generations are unique across the process: each thread takes a range from a shared
/// counter and counts within it. Two tables share a generation only when one was cloned
/// from the other and neither changed since, so they hold the same classes.
static NEXT_RANGE: AtomicU64 = AtomicU64::new(1);
const RANGE_SIZE: u64 = 1 << 32;

thread_local! {
    static LOCAL_RANGE: Cell<(u64, u64)> = const { Cell::new((0, 0)) };
}

fn next_generation() -> u64 {
    LOCAL_RANGE.with(|range| {
        let (mut next, mut end) = range.get();
        if next == end {
            next = NEXT_RANGE.fetch_add(1, Ordering::Relaxed) * RANGE_SIZE;
            end = next + RANGE_SIZE;
        }
        range.set((next + 1, end));
        next
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ClassTable {
    map: Map,
    generation: u64,
    /// The classes holding call sites, in map order; dropped on any write, call sites included.
    with_call_sites: std::sync::OnceLock<Arc<[Sym]>>,
}

impl Default for ClassTable {
    fn default() -> Self {
        Self {
            map: Map::default(),
            generation: next_generation(),
            with_call_sites: std::sync::OnceLock::new(),
        }
    }
}

impl Deref for ClassTable {
    type Target = Map;

    #[inline]
    fn deref(&self) -> &Map {
        &self.map
    }
}

impl<'a> IntoIterator for &'a ClassTable {
    type Item = (&'a Sym, &'a Arc<ClassData>);
    type IntoIter = std::collections::hash_map::Iter<'a, Sym, Arc<ClassData>>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl ClassTable {
    /// Changes whenever any class may have changed.
    #[inline]
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    #[inline]
    fn touch(&mut self) {
        self.generation = next_generation();
        self.with_call_sites = std::sync::OnceLock::new();
    }

    /// The classes that hold call sites, in the map's iteration order.
    pub(crate) fn with_call_sites(&self) -> Arc<[Sym]> {
        Arc::clone(self.with_call_sites.get_or_init(|| {
            self.map
                .iter()
                .filter(|(_, data)| !data.call_sites.is_empty())
                .map(|(name, _)| *name)
                .collect()
        }))
    }

    pub(crate) fn get_mut(&mut self, class_name: &str) -> Option<&mut ClassData> {
        self.touch();
        self.map.get_mut(class_name).map(write)
    }

    /// Mutable access that keeps the generation. Only for writes to a class's call sites and
    /// their bookkeeping, which nothing that compares generations reads.
    pub(crate) fn get_mut_for_call_sites(&mut self, class_name: &str) -> Option<&mut ClassData> {
        self.with_call_sites = std::sync::OnceLock::new();
        self.map.get_mut(class_name).map(write)
    }

    /// The class, created empty when missing.
    pub(crate) fn get_or_insert_default(&mut self, class_name: Sym) -> &mut ClassData {
        self.touch();
        write(self.map.entry(class_name).or_default())
    }

    /// Puts `data` at `class_name`, shared with the caller's copy.
    pub(crate) fn insert_shared(&mut self, class_name: Sym, data: Arc<ClassData>) {
        self.touch();
        self.map.insert(class_name, data);
    }

    pub(crate) fn remove(&mut self, class_name: &str) -> Option<Arc<ClassData>> {
        self.touch();
        self.map.remove(class_name)
    }

    /// Like `HashMap::retain`, but `keep` reads the class and gets `ClassRef` to write it, so a
    /// shared class is copied only when `keep` actually writes.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&Sym, &mut ClassRef<'_>) -> bool) {
        self.touch();
        self.map
            .retain(|name, data| keep(name, &mut ClassRef { data }));
    }

    /// Every class, copied out of sharing only when the caller writes through `ClassRef`.
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = ClassRef<'_>> {
        self.touch();
        self.map.values_mut().map(|data| ClassRef { data })
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }
}

/// The class for writing, copied first when shared. Values derived from it are dropped.
fn write(data: &mut Arc<ClassData>) -> &mut ClassData {
    let data = Arc::make_mut(data);
    data.derived.clear();
    data
}

/// A value computed from a class and kept with it. A write through the table drops it, and a
/// copied class starts without it.
pub(crate) struct DerivedSlot<T>(std::sync::OnceLock<T>);

impl<T> DerivedSlot<T> {
    pub(crate) fn get_or_init(&self, init: impl FnOnce() -> T) -> &T {
        self.0.get_or_init(init)
    }

    fn clear(&mut self) {
        self.0.take();
    }
}

impl<T> Default for DerivedSlot<T> {
    fn default() -> Self {
        Self(std::sync::OnceLock::new())
    }
}

impl<T> Clone for DerivedSlot<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

// Class fingerprints hash the `Debug` text, so this prints the same whether it is filled.
impl<T> std::fmt::Debug for DerivedSlot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DerivedSlot")
    }
}

/// A class in the table: reads see it in place, and `write` copies it first when shared.
pub(crate) struct ClassRef<'a> {
    data: &'a mut Arc<ClassData>,
}

impl ClassRef<'_> {
    #[inline]
    pub(crate) fn write(&mut self) -> &mut ClassData {
        write(self.data)
    }
}

impl Deref for ClassRef<'_> {
    type Target = ClassData;

    #[inline]
    fn deref(&self) -> &ClassData {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::ClassTable;
    use crate::types::Sym;
    use std::sync::Arc;

    #[test]
    fn mutable_access_moves_the_generation() {
        let mut table = ClassTable::default();
        let start = table.generation();
        table.get_or_insert_default(Sym::new("A"));
        let after_insert = table.generation();
        assert_ne!(start, after_insert);
        let _ = table.get("A");
        assert_eq!(table.generation(), after_insert);
        let _ = table.get_mut("A");
        assert_ne!(table.generation(), after_insert);
    }

    #[test]
    fn clones_share_a_generation_until_one_changes() {
        let mut table = ClassTable::default();
        table.get_or_insert_default(Sym::new("A"));
        let mut copy = table.clone();
        assert_eq!(copy.generation(), table.generation());
        copy.get_or_insert_default(Sym::new("B"));
        assert_ne!(copy.generation(), table.generation());
        let other = ClassTable::default();
        assert_ne!(other.generation(), table.generation());
    }

    #[test]
    fn generations_differ_across_threads() {
        let here = ClassTable::default().generation();
        let there = std::thread::spawn(|| ClassTable::default().generation())
            .join()
            .expect("thread");
        assert_ne!(here, there);
    }

    #[test]
    fn writes_copy_a_shared_class_only() {
        let mut table = ClassTable::default();
        table.get_or_insert_default(Sym::new("A")).is_module = true;
        let copy = table.clone();
        assert!(Arc::ptr_eq(&table["A"], &copy["A"]));
        for data in table.values_mut() {
            assert!(data.is_module);
        }
        assert!(Arc::ptr_eq(&table["A"], &copy["A"]));
        table.get_mut("A").expect("A").is_module = false;
        assert!(!Arc::ptr_eq(&table["A"], &copy["A"]));
        assert!(copy["A"].is_module);
        assert!(!table["A"].is_module);
    }
}
