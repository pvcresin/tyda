//! Capped FIFO cache of parsed file slots (merged data lives in the workspace registry instead).

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

// A cap of 128 causes scan thrash that worsens peak RSS; 2048 is a defensive upper bound based on observed workloads.
pub const DEFAULT_FILE_CACHE_CAP: usize = 2048;

pub struct BoundedFileCache<K, V>
where
    K: Eq + Hash + Clone,
{
    cap: usize,
    entries: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K, V> BoundedFileCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key)
    }

    /// Insert for the cache-miss path only (assumes a hit was already checked; may evict).
    pub fn insert(&mut self, key: K, value: V) -> &V {
        if self.entries.contains_key(&key) {
            // Repeat insert without a prior `get` hit: replace in place but
            // keep the old position in `order` so we don't double-track.
            self.entries.insert(key.clone(), value);
            return self.entries.get(&key).expect("just inserted");
        }
        self.entries.insert(key.clone(), value);
        self.order.push_back(key.clone());
        self.evict_to_cap();
        self.entries.get(&key).expect("just inserted")
    }

    fn evict_to_cap(&mut self) {
        while self.entries.len() > self.cap {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.entries.shrink_to_fit();
        self.order.shrink_to_fit();
    }

    /// Drop a specific key, keeping the rest of the cache intact. Used by
    /// `reload_path` to invalidate just the file whose contents changed.
    pub fn remove(&mut self, key: &K) {
        if self.entries.remove(key).is_some() {
            self.order.retain(|stored| stored != key);
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_eviction_drops_oldest_first() {
        let mut cache: BoundedFileCache<String, u32> = BoundedFileCache::with_cap(3);
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        cache.insert("c".to_string(), 3);
        cache.insert("d".to_string(), 4);
        assert_eq!(cache.get(&"a".to_string()), None);
        assert_eq!(cache.get(&"b".to_string()), Some(&2));
        assert_eq!(cache.get(&"c".to_string()), Some(&3));
        assert_eq!(cache.get(&"d".to_string()), Some(&4));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn re_insert_replaces_value_without_growing() {
        let mut cache: BoundedFileCache<String, u32> = BoundedFileCache::with_cap(2);
        cache.insert("a".to_string(), 1);
        cache.insert("a".to_string(), 99);
        assert_eq!(cache.get(&"a".to_string()), Some(&99));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn clear_resets_state() {
        let mut cache: BoundedFileCache<String, u32> = BoundedFileCache::with_cap(2);
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.get(&"a".to_string()), None);
    }

    #[test]
    fn is_empty_tracks_entry_count() {
        let mut cache: BoundedFileCache<String, u32> = BoundedFileCache::with_cap(2);
        assert!(cache.is_empty());
        cache.insert("a".to_string(), 1);
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn cap_zero_is_treated_as_one() {
        let mut cache: BoundedFileCache<String, u32> = BoundedFileCache::with_cap(0);
        assert_eq!(cache.cap(), 1);
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        assert_eq!(cache.get(&"a".to_string()), None);
        assert_eq!(cache.get(&"b".to_string()), Some(&2));
    }
}
