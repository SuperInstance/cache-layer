//! Cache tier implementations
//!
//! Individual cache layers (memory, Redis, disk) that can be composed
//! into a multi-tier cache hierarchy.

use crate::error::CacheError;
use std::time::Duration;

/// Statistics for a cache tier
#[derive(Debug, Clone, Default)]
pub struct TierStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Current size in bytes
    pub size_bytes: usize,
    /// Maximum capacity in bytes
    pub capacity_bytes: usize,
    /// Number of entries
    pub entries: usize,
}

impl TierStats {
    /// Calculate hit rate
    ///
    /// # Returns
    /// Hit rate as a float between 0.0 and 1.0, or None if no requests
    ///
    /// # Examples
    /// ```
    /// use cache_layer::tier::TierStats;
    ///
    /// let stats = TierStats {
    ///     hits: 80,
    ///     misses: 20,
    ///     ..Default::default()
    /// };
    /// assert_eq!(stats.hit_rate(), Some(0.8));
    /// ```
    pub fn hit_rate(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        if total == 0 {
            None
        } else {
            Some(self.hits as f64 / total as f64)
        }
    }

    /// Get total number of requests
    pub fn total_requests(&self) -> u64 {
        self.hits + self.misses
    }
}

/// Individual cache tier
///
/// Represents a single layer in the cache hierarchy (e.g., memory, Redis, disk).
pub trait Tier<K, V>: Send + Sync {
    /// Retrieve from this tier only
    ///
    /// # Returns
    /// - Ok(Some(value)) if found
    /// - Ok(None) if not found
    /// - Err if tier operation failed
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// if let Some(value) = tier.get(&"key").await? {
    ///     println!("Found: {}", value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn get(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// Store in this tier only
    ///
    /// # Parameters
    /// - `key`: The key to store
    /// - `value`: The value to store
    /// - `ttl`: Optional time-to-live (None = no expiration)
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # use std::time::Duration;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// tier.set("key".to_string(), "value".to_string(), Some(Duration::from_secs(60))).await?;
    /// # Ok(())
    /// # }
    /// ```
    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<(), CacheError>;

    /// Remove from this tier
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// tier.delete(&"key").await?;
    /// # Ok(())
    /// # }
    /// ```
    fn delete(&self, key: &K) -> Result<(), CacheError>;

    /// Check if key exists in this tier
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// if tier.exists(&"key").await? {
    ///     println!("Key exists");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn exists(&self, key: &K) -> Result<bool, CacheError>;

    /// Current size in bytes
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # fn example() {
    /// let tier: MemoryCache<String, String> = MemoryCache::new(1024);
    /// assert_eq!(tier.size(), 0);
    /// # }
    /// ```
    fn size(&self) -> usize;

    /// Maximum capacity in bytes
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # fn example() {
    /// let tier: MemoryCache<String, String> = MemoryCache::new(1024);
    /// assert_eq!(tier.capacity(), 1024);
    /// # }
    /// ```
    fn capacity(&self) -> usize;

    /// Clear all entries from this tier
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// tier.clear().await?;
    /// # Ok(())
    /// # }
    /// ```
    fn clear(&self) -> Result<(), CacheError>;

    /// Get hit/miss statistics
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::tier::Tier;
    /// # use cache_layer::memory::MemoryCache;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let tier = MemoryCache::<String, String>::new(1024);
    /// let stats = tier.stats().await;
    /// println!("Hit rate: {:?}", stats.hit_rate());
    /// # Ok(())
    /// # }
    /// ```
    fn stats(&self) -> TierStats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_stats_hit_rate() {
        let stats = TierStats {
            hits: 80,
            misses: 20,
            ..Default::default()
        };
        assert_eq!(stats.hit_rate(), Some(0.8));
    }

    #[test]
    fn test_tier_stats_hit_rate_no_requests() {
        let stats = TierStats::default();
        assert_eq!(stats.hit_rate(), None);
    }

    #[test]
    fn test_tier_stats_total_requests() {
        let stats = TierStats {
            hits: 80,
            misses: 20,
            ..Default::default()
        };
        assert_eq!(stats.total_requests(), 100);
    }

    #[test]
    fn test_tier_stats_hit_rate_perfect() {
        let stats = TierStats {
            hits: 100,
            misses: 0,
            ..Default::default()
        };
        assert_eq!(stats.hit_rate(), Some(1.0));
    }

    #[test]
    fn test_tier_stats_hit_rate_zero() {
        let stats = TierStats {
            hits: 0,
            misses: 100,
            ..Default::default()
        };
        assert_eq!(stats.hit_rate(), Some(0.0));
    }
}
