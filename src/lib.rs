//! cache-layer: Multi-tier caching library with memory, Redis, and disk persistence
//!
//! This library implements the timeless principle of **memory hierarchy**, organizing
//! data into tiers with decreasing speed but increasing capacity to achieve performance
//! approaching the fastest tier while maintaining the capacity of the slowest.
//!
//! ## Timeless Principle
//!
//! ```text
//! Application Code
//!     ↓
//! L1: Memory Cache (~100ns)
//!     ↓
//! L2: Redis Cache (~1ms)
//!     ↓
//! L3: Disk Cache (~10ms)
//!     ↓
//! Database/Computation (~100ms)
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use cache_layer::{Cache, MemoryCache, MultiTierCache};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a memory cache tier
//!     let l1 = MemoryCache::new(1024 * 1024); // 1MB
//!
//!     // Create multi-tier cache
//!     let mut cache = MultiTierCache::new();
//!     cache.add_tier(Box::new(l1));
//!
//!     // Set a value
//!     cache.set("user:123", "Alice".to_string()).await?;
//!
//!     // Get it back
//!     if let Some(value) = cache.get(&"user:123").await? {
//!         println!("Found: {}", value);
//!     }
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

// Public modules
pub mod cache;
pub mod error;
pub mod eviction;
pub mod memory;
pub mod multi_tier;
pub mod tier;

// Re-export key types
pub use cache::Cache;
pub use error::{CacheError, TierError};
pub use eviction::{EvictionPolicy, LFU, LRU};
pub use memory::MemoryCache;
pub use multi_tier::MultiTierCache;
pub use tier::{Tier, TierStats};

use std::time::Duration;

/// Timeless constant: Default TTL for cache entries (5 minutes)
pub const DEFAULT_TTL: Duration = Duration::from_secs(300);

/// Timeless constant: Default memory cache capacity (1MB)
pub const DEFAULT_CAPACITY: usize = 1024 * 1024;

/// Timeless constant: Maximum cache key length
pub const MAX_KEY_LENGTH: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_TTL.as_secs(), 300);
        assert_eq!(DEFAULT_CAPACITY, 1024 * 1024);
        assert_eq!(MAX_KEY_LENGTH, 256);
    }
}
