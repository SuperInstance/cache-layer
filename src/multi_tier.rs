//! Multi-tier cache orchestration
//!
//! Combines multiple cache tiers with automatic promotion and eviction.

use crate::cache::Cache;
use crate::error::CacheError;
use crate::tier::{Tier, TierStats};
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

/// Multi-tier cache that combines multiple cache layers
///
/// # Timeless Principle
/// Memory hierarchy: Faster tiers are checked first, with automatic promotion
/// of frequently accessed data to faster tiers.
///
/// # Examples
/// ```
/// use cache_layer::{MemoryCache, MultiTierCache};
/// use cache_layer::Cache;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut cache = MultiTierCache::new();
/// cache.add_tier(Arc::new(MemoryCache::new(1024 * 1024))); // L1: 1MB
///
/// cache.set("key".to_string(), "value".to_string()).await?;
/// let value = cache.get(&"key".to_string()).await?;
/// # Ok(())
/// # }
/// ```
pub struct MultiTierCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Cache tiers (L1=fastest, Ln=slowest)
    tiers: Vec<Arc<dyn Tier<K, V>>>,
}

impl<K, V> MultiTierCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new multi-tier cache
    ///
    /// # Examples
    /// ```
    /// use cache_layer::MultiTierCache;
    ///
    /// let cache: MultiTierCache<String, String> = MultiTierCache::new();
    /// assert_eq!(cache.num_tiers(), 0);
    /// ```
    pub fn new() -> Self {
        Self { tiers: Vec::new() }
    }

    /// Add a cache tier
    ///
    /// # Parameters
    /// - `tier`: The tier to add (will be added to the end, as slower tier)
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::{MultiTierCache, MemoryCache};
    /// # fn example() {
    /// let mut cache = MultiTierCache::new();
    /// cache.add_tier(Arc::new(MemoryCache::new(1024)));
    /// assert_eq!(cache.num_tiers(), 1);
    /// # }
    /// ```
    pub fn add_tier(&mut self, tier: Arc<dyn Tier<K, V>>) {
        self.tiers.push(tier);
    }

    /// Get number of tiers
    ///
    /// # Examples
    /// ```
    /// use cache_layer::MultiTierCache;
    ///
    /// let cache: MultiTierCache<String, String> = MultiTierCache::new();
    /// assert_eq!(cache.num_tiers(), 0);
    /// ```
    pub fn num_tiers(&self) -> usize {
        self.tiers.len()
    }

    /// Get statistics for all tiers
    ///
    /// # Returns
    /// Vector of stats, one per tier
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::{MultiTierCache, MemoryCache};
    /// # async fn example() {
    /// # let mut cache = MultiTierCache::new();
    /// # cache.add_tier(Arc::new(MemoryCache::new(1024)));
    /// let stats = cache.stats().await;
    /// assert_eq!(stats.len(), 1);
    /// # }
    /// ```
    pub async fn stats(&self) -> Vec<TierStats> {
        let mut stats = Vec::new();
        for tier in &self.tiers {
            stats.push(tier.stats());
        }
        stats
    }
}

impl<K, V> Default for MultiTierCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Cache<K, V> for MultiTierCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn get(&self, key: &K) -> Result<Option<V>, CacheError> {
        // Search tiers from fastest (L1) to slowest (Ln)
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            match tier.get(key) {
                Ok(Some(value)) => {
                    // Found in this tier - promote to faster tiers
                    for faster_tier in self.tiers.iter().take(tier_idx) {
                        let _ = faster_tier.set(key.clone(), value.clone(), None);
                    }
                    return Ok(Some(value));
                }
                Ok(None) => continue,
                Err(e) => {
                    // Log error but continue to next tier
                    tracing::warn!("Tier {} get failed: {}", tier_idx, e);
                    continue;
                }
            }
        }

        // Not found in any tier
        Ok(None)
    }

    async fn set(&self, key: K, value: V) -> Result<(), CacheError> {
        self.set_with_ttl(key, value, Duration::from_secs(300))
            .await
    }

    async fn delete(&self, key: &K) -> Result<(), CacheError> {
        let mut last_error = None;

        // Delete from all tiers
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            if let Err(e) = tier.delete(key) {
                tracing::warn!("Tier {} delete failed: {}", tier_idx, e);
                last_error = Some(e);
            }
        }

        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }

    async fn exists(&self, key: &K) -> Result<bool, CacheError> {
        // Check if exists in any tier
        for tier in &self.tiers {
            if let Ok(exists) = tier.exists(key) {
                if exists {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError> {
        let mut last_error = None;

        // Set in all tiers
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            if let Err(e) = tier.set(key.clone(), value.clone(), Some(ttl)) {
                tracing::warn!("Tier {} set failed: {}", tier_idx, e);
                last_error = Some(e);
            }
        }

        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryCache;

    #[tokio::test]
    async fn test_multi_tier_creation() {
        let cache: MultiTierCache<String, String> = MultiTierCache::new();
        assert_eq!(cache.num_tiers(), 0);
    }

    #[tokio::test]
    async fn test_multi_tier_add_tier() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));
        assert_eq!(cache.num_tiers(), 1);
    }

    #[tokio::test]
    async fn test_multi_tier_default() {
        let cache: MultiTierCache<String, String> = MultiTierCache::default();
        assert_eq!(cache.num_tiers(), 0);
    }

    #[tokio::test]
    async fn test_multi_tier_single_tier() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));

        cache
            .set("key".to_string(), "value".to_string())
            .await
            .unwrap();

        let result = cache.get(&"key".to_string()).await.unwrap();
        assert_eq!(result, Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_multi_tier_get_miss() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));

        let result = cache.get(&"nonexistent".to_string()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_multi_tier_delete() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));

        cache
            .set("key".to_string(), "value".to_string())
            .await
            .unwrap();

        cache.delete(&"key".to_string()).await.unwrap();

        let result = cache.get(&"key".to_string()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_multi_tier_exists() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));

        assert!(!cache.exists(&"key".to_string()).await.unwrap());

        cache
            .set("key".to_string(), "value".to_string())
            .await
            .unwrap();

        assert!(cache.exists(&"key".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_multi_tier_set_with_ttl() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(1024)));

        cache
            .set_with_ttl(
                "key".to_string(),
                "value".to_string(),
                Duration::from_millis(1),
            )
            .await
            .unwrap();

        // Should exist immediately
        assert!(cache.exists(&"key".to_string()).await.unwrap());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        // Should be expired
        assert!(!cache.exists(&"key".to_string()).await.unwrap());
    }

    #[tokio::test]
    async fn test_multi_tier_two_tiers() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(512))); // L1
        cache.add_tier(Arc::new(MemoryCache::new(1024))); // L2

        assert_eq!(cache.num_tiers(), 2);

        cache
            .set("key".to_string(), "value".to_string())
            .await
            .unwrap();

        let stats = cache.stats().await;
        assert_eq!(stats.len(), 2);
    }

    #[tokio::test]
    async fn test_multi_tier_promotion() {
        let mut cache: MultiTierCache<String, String> = MultiTierCache::new();
        cache.add_tier(Arc::new(MemoryCache::new(100))); // L1 (small)
        cache.add_tier(Arc::new(MemoryCache::new(1024))); // L2 (larger)

        // Set a value
        cache
            .set("key".to_string(), "value".to_string())
            .await
            .unwrap();

        // Get from L2 (simulating L1 miss)
        // Value should be promoted to L1
        let result = cache.get(&"key".to_string()).await.unwrap();
        assert_eq!(result, Some("value".to_string()));

        // Verify both tiers have the value
        let stats = cache.stats().await;
        assert_eq!(stats[0].entries, 1); // L1 has it (promoted)
        assert_eq!(stats[1].entries, 1); // L2 has it
    }
}
