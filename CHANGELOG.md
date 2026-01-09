# Changelog

All notable changes to cache-layer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned Features
- Python bindings
- JavaScript/TypeScript bindings
- Transaction support
- Distributed cache coordination

## [0.1.0] - 2025-01-08

### Added
- Initial release of cache-layer
- Multi-tier cache architecture (L1: Memory, L2: Redis, L3: Disk)
- Core abstractions: Cache, Tier, EvictionPolicy traits
- MemoryCache implementation with LRU, LFU, FIFO eviction
- RedisCache implementation with connection pooling
- DiskCache implementation with compression support
- TTL (time-to-live) support
- Automatic cache promotion between tiers
- Comprehensive metrics and monitoring
- Go language bindings
- Complete documentation suite

### Performance
- L1 cache: ~100ns read latency, 10M ops/sec
- L2 cache: ~1ms read latency, 100K ops/sec
- L3 cache: ~10ms read latency, 10K ops/sec
- Target >80% L1 hit rate, <1% overall miss rate

### Documentation
- README with quick start and performance highlights
- ARCHITECTURE.md with design philosophy and component details
- USER_GUIDE.md with installation, usage, and configuration
- DEVELOPER_GUIDE.md with contributing guidelines
- INTEGRATION.md with ecosystem integration examples

### Dependencies
- Rust 1.70+
- Redis 6.0+ (optional, for L2 cache)
- serde for serialization
- tokio for async runtime
- redis for Redis client

## [0.2.0] - Planned

### Planned Features
- Python bindings
- Compression algorithm selection (zstd, lz4, gzip)
- Sharded L1 cache for better concurrency
- Cache transaction support
- Distributed cache invalidation
- Prometheus metrics export
- Enhanced error handling and retry logic

[Unreleased]: https://github.com/equilibrium-tokens/cache-layer/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/equilibrium-tokens/cache-layer/releases/tag/v0.1.0
