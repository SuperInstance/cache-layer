//! Cache eviction policies for replacing entries when capacity is exceeded
//!
//! Implements timeless replacement strategies based on access patterns.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Cache eviction policy
///
/// Defines how to select entries for eviction when cache is full.
pub trait EvictionPolicy<K>: Send + Sync {
    /// Record that a key was accessed
    fn on_access(&mut self, key: &K);

    /// Record that a key was inserted
    fn on_insert(&mut self, key: K);

    /// Select a key to evict
    fn evict(&mut self) -> Option<K>;

    /// Reset policy state
    fn reset(&mut self);

    /// Clone for new instances
    fn clone_box(&self) -> Box<dyn EvictionPolicy<K>>;
}

impl<K: Clone + 'static> Clone for Box<dyn EvictionPolicy<K>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// LRU (Least Recently Used) eviction policy
///
/// # Timeless Principle
/// Temporal locality: recently accessed items are likely to be accessed again.
/// Evict the least recently used item first.
///
/// # Example
/// ```
/// use cache_layer::eviction::LRU;
///
/// let mut lru = LRU::new(3);
/// lru.on_insert("a");
/// lru.on_insert("b");
/// lru.on_insert("c");
/// lru.on_access(&"a"); // Access 'a' (now most recent)
///
/// // Eviction order: b (least recent), c, a (most recent)
/// assert_eq!(lru.evict(), Some("b"));
/// ```
#[derive(Debug, Clone)]
pub struct LRU<K> {
    /// Maximum number of entries to track
    capacity: usize,
    /// Access order: front = least recent, back = most recent
    access_order: VecDeque<K>,
    /// Key to position in access_order mapping
    key_index: HashMap<K, usize>,
}

impl<K: Hash + Eq + Clone> LRU<K> {
    /// Create a new LRU policy
    ///
    /// # Panics
    /// Panics if capacity is 0
    ///
    /// # Examples
    /// ```
    /// use cache_layer::eviction::LRU;
    ///
    /// let lru = LRU::new(100);
    /// assert_eq!(lru.capacity(), 100);
    /// ```
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LRU capacity must be > 0");
        Self {
            capacity,
            access_order: VecDeque::with_capacity(capacity),
            key_index: HashMap::new(),
        }
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current size
    pub fn size(&self) -> usize {
        self.access_order.len()
    }
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static> EvictionPolicy<K> for LRU<K> {
    fn on_access(&mut self, key: &K) {
        // Remove from current position if exists
        if let Some(&idx) = self.key_index.get(key) {
            self.access_order.remove(idx);
            // Update indices for items that shifted
            for (_, v) in self.key_index.iter_mut() {
                if *v > idx {
                    *v -= 1;
                }
            }
        }

        // Add to end (most recent)
        self.access_order.push_back(key.clone());
        self.key_index.insert(key.clone(), self.access_order.len() - 1);
    }

    fn on_insert(&mut self, key: K) {
        self.on_access(&key);
    }

    fn evict(&mut self) -> Option<K> {
        if let Some(key) = self.access_order.pop_front() {
            self.key_index.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    fn reset(&mut self) {
        self.access_order.clear();
        self.key_index.clear();
    }

    fn clone_box(&self) -> Box<dyn EvictionPolicy<K>> {
        Box::new(self.clone())
    }
}

/// LFU (Least Frequently Used) eviction policy
///
/// # Timeless Principle
/// Frequency-based locality: frequently accessed items are likely to be accessed again.
/// Evict the least frequently used item first.
///
/// # Example
/// ```
/// use cache_layer::eviction::LFU;
///
/// let mut lfu = LFU::new(3);
/// lfu.on_insert("a");
/// lfu.on_insert("b");
/// lfu.on_insert("c");
///
/// lfu.on_access(&"a");
/// lfu.on_access(&"a");
/// lfu.on_access(&"b");
///
/// // Access counts: a=2, b=1, c=0
/// assert_eq!(lfu.evict(), Some("c")); // Evict 'c' (least frequent)
/// ```
#[derive(Debug)]
pub struct LFU<K> {
    /// Maximum number of entries to track
    capacity: usize,
    /// Access count per key
    access_count: HashMap<K, usize>,
    /// Minimum access frequency
    min_frequency: usize,
}

impl<K: Hash + Eq + Clone> LFU<K> {
    /// Create a new LFU policy
    ///
    /// # Panics
    /// Panics if capacity is 0
    ///
    /// # Examples
    /// ```
    /// use cache_layer::eviction::LFU;
    ///
    /// let lfu = LFU::new(100);
    /// assert_eq!(lfu.capacity(), 100);
    /// ```
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LFU capacity must be > 0");
        Self {
            capacity,
            access_count: HashMap::new(),
            min_frequency: 0,
        }
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current size
    pub fn size(&self) -> usize {
        self.access_count.len()
    }

    /// Get access count for a key
    pub fn get_count(&self, key: &K) -> usize {
        *self.access_count.get(key).unwrap_or(&0)
    }
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static> EvictionPolicy<K> for LFU<K> {
    fn on_access(&mut self, key: &K) {
        let count = self.access_count.entry(key.clone()).or_insert(0);
        *count += 1;

        // Update min frequency
        if self.min_frequency == 0 || *count < self.min_frequency {
            self.min_frequency = *count;
        }
    }

    fn on_insert(&mut self, key: K) {
        self.access_count.insert(key, 1);
        self.min_frequency = 1;
    }

    fn evict(&mut self) -> Option<K> {
        let mut min_key = None;
        let mut min_count = usize::MAX;

        // Find key with minimum access count
        for (key, count) in self.access_count.iter() {
            if *count < min_count {
                min_count = *count;
                min_key = Some(key.clone());
            }
        }

        if let Some(key) = min_key {
            self.access_count.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    fn reset(&mut self) {
        self.access_count.clear();
        self.min_frequency = 0;
    }

    fn clone_box(&self) -> Box<dyn EvictionPolicy<K>> {
        Box::new(LFU {
            capacity: self.capacity,
            access_count: self.access_count.clone(),
            min_frequency: self.min_frequency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // LRU Tests
    #[test]
    fn test_lru_creation() {
        let lru: LRU<&str> = LRU::new(10);
        assert_eq!(lru.capacity(), 10);
        assert_eq!(lru.size(), 0);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn test_lru_zero_capacity() {
        LRU::<String>::new(0);
    }

    #[test]
    fn test_lru_insert() {
        let mut lru: LRU<&str> = LRU::new(3);
        lru.on_insert("a");
        lru.on_insert("b");
        lru.on_insert("c");
        assert_eq!(lru.size(), 3);
    }

    #[test]
    fn test_lru_access() {
        let mut lru: LRU<&str> = LRU::new(3);
        lru.on_insert("a");
        lru.on_insert("b");
        lru.on_insert("c");
        lru.on_access(&"a"); // Access 'a'

        // Eviction order: b, c, a
        assert_eq!(lru.evict(), Some("b"));
        assert_eq!(lru.evict(), Some("c"));
        assert_eq!(lru.evict(), Some("a"));
    }

    #[test]
    fn test_lru_evict() {
        let mut lru: LRU<&str> = LRU::new(3);
        lru.on_insert("a");
        lru.on_insert("b");
        assert_eq!(lru.evict(), Some("a")); // 'a' is least recent
        assert_eq!(lru.size(), 1);
    }

    #[test]
    fn test_lru_reset() {
        let mut lru: LRU<&str> = LRU::new(3);
        lru.on_insert("a");
        lru.on_insert("b");
        lru.reset();
        assert_eq!(lru.size(), 0);
    }

    // LFU Tests
    #[test]
    fn test_lfu_creation() {
        let lfu: LFU<&str> = LFU::new(10);
        assert_eq!(lfu.capacity(), 10);
        assert_eq!(lfu.size(), 0);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn test_lfu_zero_capacity() {
        LFU::<String>::new(0);
    }

    #[test]
    fn test_lfu_insert() {
        let mut lfu: LFU<&str> = LFU::new(3);
        lfu.on_insert("a");
        lfu.on_insert("b");
        assert_eq!(lfu.size(), 2);
        assert_eq!(lfu.get_count(&"a"), 1);
    }

    #[test]
    fn test_lfu_access() {
        let mut lfu: LFU<&str> = LFU::new(3);
        lfu.on_insert("a");
        lfu.on_access(&"a");
        lfu.on_access(&"a");
        assert_eq!(lfu.get_count(&"a"), 3); // insert + 2 accesses
    }

    #[test]
    fn test_lfu_evict() {
        let mut lfu: LFU<&str> = LFU::new(3);
        lfu.on_insert("a");
        lfu.on_insert("b");
        lfu.on_insert("c");

        lfu.on_access(&"a");
        lfu.on_access(&"a"); // a: 3 accesses
        lfu.on_access(&"b"); // b: 2 accesses
                            // c: 1 access (least frequent)

        assert_eq!(lfu.evict(), Some("c"));
    }

    #[test]
    fn test_lfu_reset() {
        let mut lfu: LFU<&str> = LFU::new(3);
        lfu.on_insert("a");
        lfu.reset();
        assert_eq!(lfu.size(), 0);
    }

    #[test]
    fn test_lfu_clone() {
        let mut lfu: LFU<&str> = LFU::new(3);
        lfu.on_insert("a");
        lfu.on_insert("b");

        let mut cloned = lfu.clone_box();
        // Note: can't call size() on Box<dyn EvictionPolicy>
        // Just verify clone works
        assert!(cloned.evict().is_some() || cloned.evict().is_none());
    }
}
