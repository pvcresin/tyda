//! The registry's class map. Every mutable access moves the table to a new generation, so a
//! reader can tell that no class changed since it last looked by comparing generations.

use super::ClassData;
use crate::types::Sym;
use rustc_hash::FxHashMap;
use std::cell::Cell;
use std::collections::hash_map::{Entry, ValuesMut};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};

type Map = FxHashMap<Sym, Box<ClassData>>;

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
}

impl Default for ClassTable {
    fn default() -> Self {
        Self {
            map: Map::default(),
            generation: next_generation(),
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
    type Item = (&'a Sym, &'a Box<ClassData>);
    type IntoIter = std::collections::hash_map::Iter<'a, Sym, Box<ClassData>>;

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
    }

    pub(crate) fn get_mut(&mut self, class_name: &str) -> Option<&mut Box<ClassData>> {
        self.touch();
        self.map.get_mut(class_name)
    }

    /// Mutable access that keeps the generation. Only for writes to a class's call sites and
    /// their bookkeeping, which nothing that compares generations reads.
    pub(crate) fn get_mut_for_call_sites(
        &mut self,
        class_name: &str,
    ) -> Option<&mut Box<ClassData>> {
        self.map.get_mut(class_name)
    }

    pub(crate) fn entry(&mut self, class_name: Sym) -> Entry<'_, Sym, Box<ClassData>> {
        self.touch();
        self.map.entry(class_name)
    }

    pub(crate) fn remove(&mut self, class_name: &str) -> Option<Box<ClassData>> {
        self.touch();
        self.map.remove(class_name)
    }

    pub(crate) fn retain(&mut self, keep: impl FnMut(&Sym, &mut Box<ClassData>) -> bool) {
        self.touch();
        self.map.retain(keep);
    }

    pub(crate) fn values_mut(&mut self) -> ValuesMut<'_, Sym, Box<ClassData>> {
        self.touch();
        self.map.values_mut()
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use super::ClassTable;
    use crate::types::Sym;

    #[test]
    fn mutable_access_moves_the_generation() {
        let mut table = ClassTable::default();
        let start = table.generation();
        table.entry(Sym::new("A")).or_default();
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
        table.entry(Sym::new("A")).or_default();
        let mut copy = table.clone();
        assert_eq!(copy.generation(), table.generation());
        copy.entry(Sym::new("B")).or_default();
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
}
