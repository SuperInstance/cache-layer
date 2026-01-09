//! Error types for cache-layer operations

use std::io;

/// Errors that can occur in cache operations
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    /// Key not found in cache
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Key exceeds maximum length
    #[error("Key too long: {len} bytes (max {max} bytes)")]
    KeyTooLong {
        /// Actual length of the key
        len: usize,
        /// Maximum allowed length
        max: usize,
    },

    /// Serialization error
    #[error("Serialization failed: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization failed: {0}")]
    DeserializationError(String),

    /// Tier operation error
    #[error("Tier error: {0}")]
    TierError(#[from] TierError),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Generic error
    #[error("Cache error: {0}")]
    Other(String),
}

/// Errors that can occur in tier operations
#[derive(thiserror::Error, Debug)]
pub enum TierError {
    /// Capacity exceeded
    #[error("Capacity exceeded: {size} bytes (max {max} bytes)")]
    CapacityExceeded {
        /// Current size in bytes
        size: usize,
        /// Maximum capacity in bytes
        max: usize
    },

    /// Invalid TTL
    #[error("Invalid TTL: {0}")]
    InvalidTTL(String),

    /// Eviction failed
    #[error("Eviction failed: {0}")]
    EvictionFailed(String),

    /// Tier not available
    #[error("Tier not available: {0}")]
    TierNotAvailable(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CacheError::KeyNotFound("test".to_string());
        assert!(err.to_string().contains("Key not found"));

        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let err = CacheError::Io(io_err);
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_tier_error_display() {
        let err = TierError::CapacityExceeded {
            size: 2048,
            max: 1024,
        };
        assert!(err.to_string().contains("Capacity exceeded"));
    }

    #[test]
    fn test_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "test");
        let cache_err: CacheError = io_err.into();
        assert!(matches!(cache_err, CacheError::Io(_)));
    }
}
