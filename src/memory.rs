//! In-memory cache tier implementation
//!
//! Fast, thread-safe in-memory cache using DashMap for concurrent access.

use crate::error::{CacheError, TierError};
use crate::eviction::{EvictionPolicy, LRU};
use crate::tier::{Tier, TierStats};
use dashmap::DashMap;
use parking_lot::Mutex;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Cached value with metadata
#[derive(Debug, Clone)]
struct CachedValue<V> {
    /// The cached value
    value: V,
    /// Expiration time (None = no expiration)
    expires_at: Option<Instant>,
    /// Size in bytes
    size_bytes: usize,
    /// Last access time
    last_access: Instant,
}

impl<V> CachedValue<V> {
    /// Check if the value has expired
    fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Instant::now() > expires
        } else {
            false
        }
    }
}

/// In-memory cache tier
///
/// Fast concurrent cache using DashMap for lock-free reads.
///
/// # Examples
/// ```
/// use cache_layer::memory::MemoryCache;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = MemoryCache::new(1024 * 1024); // 1MB capacity
///
/// // Set a value
/// cache.set("key".to_string(), "value".to_string(), None).await?;
///
/// // Get it back
/// if let Some(value) = cache.get(&"key".to_string()).await? {
///     assert_eq!(value, "value");
/// }
/// # Ok(())
/// # }
/// ```
pub struct MemoryCache<K, V>
where
    K: Hash + Eq + Clone,
{
    /// The underlying map
    entries: DashMap<K, CachedValue<V>>,
    /// Eviction policy
    eviction_policy: Mutex<Box<dyn EvictionPolicy<K>>>,
    /// Maximum capacity in bytes
    capacity_bytes: usize,
    /// Current size in bytes
    size_bytes: Mutex<usize>,
    /// Statistics
    stats: Mutex<TierStats>,
}

impl<K, V> MemoryCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new memory cache
    ///
    /// # Parameters
    /// - `capacity_bytes`: Maximum size in bytes
    ///
    /// # Panics
    /// Panics if capacity_bytes is 0
    ///
    /// # Examples
    /// ```
    /// use cache_layer::memory::MemoryCache;
    ///
    /// let cache = MemoryCache::<String, String>::new(1024);
    /// assert_eq!(cache.capacity(), 1024);
    /// ```
    pub fn new(capacity_bytes: usize) -> Self {
        assert!(capacity_bytes > 0, "Capacity must be > 0");

        Self {
            entries: DashMap::new(),
            eviction_policy: Mutex::new(Box::new(LRU::new(1024))), // Track up to 1024 keys for eviction
            capacity_bytes,
            size_bytes: Mutex::new(0),
            stats: Mutex::new(TierStats {
                capacity_bytes,
                ..Default::default()
            }),
        }
    }

    /// Create with custom eviction policy
    ///
    /// # Examples
    /// ```
    /// use cache_layer::memory::MemoryCache;
    /// use cache_layer::eviction::LFU;
    ///
    /// let cache = MemoryCache::with_policy(1024, LFU::new(100));
    /// ```
    pub fn with_policy(capacity_bytes: usize, policy: impl EvictionPolicy<K> + 'static) -> Self {
        assert!(capacity_bytes > 0, "Capacity must be > 0");

        Self {
            entries: DashMap::new(),
            eviction_policy: Mutex::new(Box::new(policy)),
            capacity_bytes,
            size_bytes: Mutex::new(0),
            stats: Mutex::new(TierStats {
                capacity_bytes,
                ..Default::default()
            }),
        }
    }

    /// Estimate size of a value in bytes
    ///
    /// This is a simplified estimation. For production use, you might want
    /// to measure actual serialized size.
    fn estimate_size(key: &K, value: &V) -> usize {
        // Simplified: use size of string representation
        // In production, use actual serialized size
        let key_size = std::mem::size_of_val(key);
        let value_size = std::mem::size_of_val(value);
        key_size + value_size
    }

    /// Evict entries until there's space for the requested size
    /// NOTE: Must NOT hold size_bytes lock when calling this
    fn evict_for_space(&self, required_bytes: usize) -> Result<(), TierError> {
        loop {
            let current_size = *self.size_bytes.lock();

            if current_size + required_bytes <= self.capacity_bytes {
                return Ok(());
            }

            // Need to evict something
            let key_to_evict = {
                let mut policy_guard = self.eviction_policy.lock();
                policy_guard.evict()
            };

            match key_to_evict {
                Some(key) => {
                    if let Some((_entry_key, entry)) = self.entries.remove(&key) {
                        // Update size and stats
                        let mut size_guard = self.size_bytes.lock();
                        *size_guard = size_guard.saturating_sub(entry.size_bytes);

                        let mut stats_guard = self.stats.lock();
                        stats_guard.evictions += 1;
                        stats_guard.entries -= 1;
                        stats_guard.size_bytes = *size_guard;
                    }
                    // If key not found in entries, it was already removed, continue loop
                }
                None => {
                    // No more entries to evict
                    let final_size = *self.size_bytes.lock();
                    return Err(TierError::CapacityExceeded {
                        size: final_size + required_bytes,
                        max: self.capacity_bytes,
                    });
                }
            }
        }
    }
}

impl<K, V> Tier<K, V> for MemoryCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        // First check if entry exists and if it's expired
        let is_expired = {
            let entry_opt = self.entries.get(key);

            match entry_opt {
                Some(entry) => entry.is_expired(),
                None => false,
            }
        };

        // If expired, remove it and return None
        if is_expired {
            self.entries.remove(key);
            let mut stats_guard = self.stats.lock();
            stats_guard.misses += 1;
            return Ok(None);
        }

        // Get the value (immutable reference will be dropped after this block)
        let value = {
            let entry_opt = self.entries.get(key);
            match entry_opt {
                Some(entry) => Some(entry.value.clone()),
                None => None,
            }
        };

        match value {
            Some(value) => {
                // Record access in eviction policy
                {
                    let mut policy_guard = self.eviction_policy.lock();
                    policy_guard.on_access(key);
                }

                // Update last access time (now that we've dropped the immutable reference)
                if let Some(mut entry_mut) = self.entries.get_mut(key) {
                    entry_mut.last_access = Instant::now();
                }

                // Update stats
                let mut stats_guard = self.stats.lock();
                stats_guard.hits += 1;
                Ok(Some(value))
            }
            None => {
                let mut stats_guard = self.stats.lock();
                stats_guard.misses += 1;
                Ok(None)
            }
        }
    }

    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<(), CacheError> {
        let size_bytes = Self::estimate_size(&key, &value);

        // Check if key already exists and remove it
        let old_size = if let Some((_, entry)) = self.entries.remove(&key) {
            let mut size_guard = self.size_bytes.lock();
            *size_guard = size_guard.saturating_sub(entry.size_bytes);
            Some(entry.size_bytes)
        } else {
            None
        };

        // Evict if necessary (must not hold size_bytes lock when calling this)
        self.evict_for_space(size_bytes)?;

        // Create new cached value
        let cached = CachedValue {
            value: value.clone(),
            expires_at: ttl.map(|d| Instant::now() + d),
            size_bytes,
            last_access: Instant::now(),
        };

        // Insert
        self.entries.insert(key.clone(), cached);

        // Update size
        let mut size_guard = self.size_bytes.lock();
        *size_guard += size_bytes;
        let final_size = *size_guard;
        drop(size_guard);

        // Update stats
        let mut stats_guard = self.stats.lock();
        if old_size.is_some() {
            // Updating existing entry, don't increment count
            stats_guard.size_bytes = final_size;
        } else {
            // New entry
            stats_guard.entries += 1;
            stats_guard.size_bytes = final_size;
        }
        drop(stats_guard);

        // Record in eviction policy
        let mut policy_guard = self.eviction_policy.lock();
        policy_guard.on_insert(key);

        Ok(())
    }

    fn delete(&self, key: &K) -> Result<(), CacheError> {
        if let Some((_, entry)) = self.entries.remove(key) {
            let mut size_guard = self.size_bytes.lock();
            *size_guard = size_guard.saturating_sub(entry.size_bytes);

            let mut stats_guard = self.stats.lock();
            stats_guard.entries -= 1;
            stats_guard.size_bytes = *size_guard;
        }
        Ok(())
    }

    fn exists(&self, key: &K) -> Result<bool, CacheError> {
        Ok(self.entries.get(key).map(|e| !e.is_expired()).unwrap_or(false))
    }

    fn size(&self) -> usize {
        *self.size_bytes.lock()
    }

    fn capacity(&self) -> usize {
        self.capacity_bytes
    }

    fn clear(&self) -> Result<(), CacheError> {
        self.entries.clear();
        let mut size_guard = self.size_bytes.lock();
        *size_guard = 0;

        let mut stats_guard = self.stats.lock();
        stats_guard.entries = 0;
        stats_guard.size_bytes = 0;

        let mut policy_guard = self.eviction_policy.lock();
        policy_guard.reset();

        Ok(())
    }

    fn stats(&self) -> TierStats {
        self.stats.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_cache_creation() {
        let cache = MemoryCache::<String, String>::new(1024);
        assert_eq!(cache.capacity(), 1024);
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_memory_cache_set_get() {
        let cache = MemoryCache::new(1024);

        cache
            .set("key".to_string(), "value".to_string(), None)
            .unwrap();

        let result = cache.get(&"key".to_string()).unwrap();
        assert_eq!(result, Some("value".to_string()));
    }

    #[test]
    fn test_memory_cache_get_miss() {
        let cache: MemoryCache<String, String> = MemoryCache::new(1024);
        let result = cache.get(&"nonexistent".to_string()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_memory_cache_delete() {
        let cache = MemoryCache::new(1024);

        cache
            .set("key".to_string(), "value".to_string(), None)
            .unwrap();

        cache.delete(&"key".to_string()).unwrap();
        let result = cache.get(&"key".to_string()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_memory_cache_exists() {
        let cache = MemoryCache::new(1024);

        assert!(!cache.exists(&"key".to_string()).unwrap());

        cache
            .set("key".to_string(), "value".to_string(), None)
            .unwrap();

        assert!(cache.exists(&"key".to_string()).unwrap());
    }

    #[test]
    fn test_memory_cache_clear() {
        let cache = MemoryCache::new(1024);

        cache
            .set("key1".to_string(), "value1".to_string(), None)
            .unwrap();
        cache
            .set("key2".to_string(), "value2".to_string(), None)
            .unwrap();

        cache.clear().unwrap();

        assert_eq!(cache.get(&"key1".to_string()).unwrap(), None);
        assert_eq!(cache.get(&"key2".to_string()).unwrap(), None);
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_memory_cache_stats() {
        let cache = MemoryCache::new(1024);

        cache
            .set("key".to_string(), "value".to_string(), None)
            .unwrap();

        cache.get(&"key".to_string()).unwrap(); // Hit
        cache.get(&"nonexistent".to_string()).unwrap(); // Miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate(), Some(0.5));
    }

    #[test]
    fn test_memory_cache_ttl() {
        let cache = MemoryCache::new(1024);

        // Set with 1ms TTL
        cache
            .set(
                "key".to_string(),
                "value".to_string(),
                Some(Duration::from_millis(1)),
            )
            .unwrap();

        // Should exist immediately
        assert!(cache.exists(&"key".to_string()).unwrap());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Should be expired now
        assert!(!cache.exists(&"key".to_string()).unwrap());
        assert_eq!(
            cache.get(&"key".to_string()).unwrap(),
            None
        );
    }

    #[test]
    fn test_memory_cache_update() {
        let cache = MemoryCache::new(1024);

        cache
            .set("key".to_string(), "value1".to_string(), None)
            .unwrap();

        cache
            .set("key".to_string(), "value2".to_string(), None)
            .unwrap();

        let result = cache.get(&"key".to_string()).unwrap();
        assert_eq!(result, Some("value2".to_string()));
    }

    #[test]
    fn test_memory_cache_with_policy() {
        let cache: MemoryCache<String, String> = MemoryCache::with_policy(1024, LRU::new(10));
        assert_eq!(cache.capacity(), 1024);
    }
}
