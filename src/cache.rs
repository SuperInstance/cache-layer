//! Cache trait for high-level cache operations

use crate::error::CacheError;
use std::time::Duration;

/// High-level cache interface
///
/// Provides simple get/set/delete operations that work across all tiers.
pub trait Cache<K, V>: Send + Sync {
    /// Retrieve a value, searching all tiers
    ///
    /// # Timeless Principle
    /// Temporal locality: When data is accessed in one tier, promote it to faster tiers.
    ///
    /// # Returns
    /// - Ok(Some(value)) if found in any tier
    /// - Ok(None) if not found
    /// - Err if all tiers failed
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::Cache;
    /// # use cache_layer::multi_tier::MultiTierCache;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = MultiTierCache::new();
    /// if let Some(value) = cache.get(&"key").await? {
    ///     println!("Found: {}", value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn get(&self, key: &K) -> Result<Option<V>, CacheError>;

    /// Store a value in all tiers
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::Cache;
    /// # use cache_layer::multi_tier::MultiTierCache;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = MultiTierCache::new();
    /// cache.set("key".to_string(), "value".to_string()).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set(&self, key: K, value: V) -> Result<(), CacheError>;

    /// Remove a value from all tiers
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::Cache;
    /// # use cache_layer::multi_tier::MultiTierCache;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = MultiTierCache::new();
    /// cache.delete(&"key").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn delete(&self, key: &K) -> Result<(), CacheError>;

    /// Check if a key exists in any tier
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::Cache;
    /// # use cache_layer::multi_tier::MultiTierCache;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = MultiTierCache::new();
    /// if cache.exists(&"key").await? {
    ///     println!("Key exists");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn exists(&self, key: &K) -> Result<bool, CacheError>;

    /// Set with time-to-live
    ///
    /// # Parameters
    /// - `key`: The key to store
    /// - `value`: The value to store
    /// - `ttl`: Time-to-live (None = no expiration)
    ///
    /// # Examples
    /// ```
    /// # use cache_layer::Cache;
    /// # use cache_layer::multi_tier::MultiTierCache;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = MultiTierCache::new();
    /// cache.set_with_ttl("key".to_string(), "value".to_string(), Duration::from_secs(60)).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<(), CacheError>;
}
