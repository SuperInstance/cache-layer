# Developer Guide

Guide for contributing to, testing, and benchmarking cache-layer.

## Table of Contents

1. [Development Setup](#development-setup)
2. [Project Structure](#project-structure)
3. [Building](#building)
4. [Testing](#testing)
5. [Adding New Cache Tiers](#adding-new-cache-tiers)
6. [Benchmarking](#benchmarking)
7. [Release Process](#release-process)
8. [Code Style](#code-style)

## Development Setup

### Prerequisites

**Required**:
- Rust 1.70+ (for Rust implementation)
- Go 1.21+ (for Go bindings)
- Git

**Optional** (for testing):
- Docker (for Redis testing)
- Redis 6.0+
- PostgreSQL 14+ (for integration tests)

### Clone Repository

```bash
git clone https://github.com/equilibrium-tokens/cache-layer.git
cd cache-layer
```

### Rust Development Setup

```bash
# Install Rust toolchain
rustup install stable
rustup default stable

# Add components
rustup component add clippy rustfmt

# Install development tools
cargo install cargo-watch
cargo install cargo-expand
cargo install cargo-release

# Install test dependencies
cargo install cargo-nextest
```

### Go Development Setup

```bash
# Set up Go module
cd bindings/go
go mod download
go install golang.org/x/tools/cmd/goimports@latest
```

### Development Database

```bash
# Start Redis for testing
docker run -d -p 6379:6379 redis:7-alpine

# Verify Redis is running
redis-cli ping
# Should respond with PONG
```

### IDE Configuration

**VSCode** (.vscode/settings.json):
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "files.watcherExclude": {
        "**/target/**": true
    },
    "editor.formatOnSave": true,
    "rust-analyzer.rustfmt.extraArgs": ["+nightly"]
}
```

## Project Structure

```
cache-layer/
├── Cargo.toml                      # Rust project manifest
├── README.md                       # Project overview
├── LICENSE                         # MIT License
├── docs/                           # Documentation
│   ├── ARCHITECTURE.md            # Architecture documentation
│   ├── USER_GUIDE.md              # User guide
│   ├── DEVELOPER_GUIDE.md         # This file
│   └── INTEGRATION.md             # Integration examples
├── src/                           # Rust source code
│   ├── lib.rs                     # Library root
│   ├── cache.rs                   # Core Cache trait
│   ├── tier.rs                    # Tier trait and implementations
│   ├── eviction.rs                # Eviction policies
│   ├── multi_tier.rs              # MultiTierCache orchestrator
│   ├── metrics.rs                 # Metrics and monitoring
│   ├── memory/                    # L1 Memory cache
│   │   ├── mod.rs
│   │   ├── cache.rs
│   │   └── eviction.rs
│   ├── redis/                     # L2 Redis cache
│   │   ├── mod.rs
│   │   ├── cache.rs
│   │   └── pool.rs
│   ├── disk/                      # L3 Disk cache
│   │   ├── mod.rs
│   │   ├── cache.rs
│   │   ├── index.rs
│   │   └── compression.rs
│   └── error.rs                   # Error types
├── tests/                         # Integration tests
│   ├── common/                    # Test utilities
│   │   ├── mod.rs
│   │   └── fixtures.rs
│   ├── memory_tests.rs            # L1 cache tests
│   ├── redis_tests.rs             # L2 cache tests
│   ├── disk_tests.rs              # L3 cache tests
│   ├── multi_tier_tests.rs        # Multi-tier tests
│   └── integration_tests.rs       # Full integration tests
├── benches/                       # Benchmarks
│   ├── memory_bench.rs
│   ├── redis_bench.rs
│   ├── disk_bench.rs
│   └── multi_tier_bench.rs
├── examples/                      # Example code
│   ├── basic_usage.rs
│   ├── vector_navigator.rs
│   ├── embeddings_cache.rs
│   └── monitoring.rs
├── bindings/                      # Language bindings
│   ├── go/                        # Go bindings
│   │   ├── cache.go
│   │   ├── tier.go
│   │   └── README.md
│   └── python/                    # Python bindings (planned)
│       └── README.md
└── scripts/                       # Utility scripts
    ├── test.sh
    ├── bench.sh
    └── release.sh
```

## Building

### Debug Build

```bash
cargo build
```

### Release Build

```bash
cargo build --release
```

### With All Features

```bash
cargo build --all-features
```

### Go Bindings

```bash
cd bindings/go
go build ./...
```

### Build Verification

```bash
# Run tests
cargo test --all

# Run clippy
cargo clippy --all-targets --all-features

# Check formatting
cargo fmt -- --check

# Run documentation tests
cargo test --doc
```

## Testing

### Unit Tests

Run unit tests for specific modules:

```bash
# Test memory cache
cargo test --lib memory

# Test Redis cache
cargo test --lib redis

# Test eviction policies
cargo test --lib eviction

# Run all unit tests
cargo test --lib
```

### Integration Tests

Run integration tests with live services:

```bash
# Start test infrastructure
docker-compose up -d

# Run integration tests
cargo test --test '*'

# Run specific test
cargo test --test multi_tier_tests test_cache_hit_promotion

# Stop test infrastructure
docker-compose down
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# View report
open coverage/index.html
```

### Running Tests in Docker

```bash
# Run all tests in container
docker build -t cache-layer-test -f Dockerfile.test .
docker run --rm cache-layer-test

# Run with coverage
docker run --rm cache-layer-test cargo tarpaulin --out Html
```

### Common Test Patterns

**Test with mock Redis**:

```rust
use cache_layer::{MultiTierCache, RedisCache};
use redis_test::MockRedisServer;

#[tokio::test]
async fn test_redis_cache_with_mock() {
    let mock_server = MockRedisServer::new().await;

    let cache = MultiTierCache::new()
        .with_l2(RedisCache::new(&mock_server.client_url()).unwrap())
        .build();

    cache.set("key", "value").await.unwrap();
    let value = cache.get(&"key".to_string()).await.unwrap();

    assert_eq!(value, Some("value".to_string()));
}
```

**Test with in-memory mocks**:

```rust
use cache_layer::memory::MemoryCache;
use cache_layer::Tier;

#[tokio::test]
async fn test_memory_cache_eviction() {
    let cache = MemoryCache::new(1000).unwrap();  // 1KB

    // Fill cache beyond capacity
    for i in 0..100 {
        cache.set(i, vec![0u8; 100], None).await.unwrap();
    }

    // Verify eviction occurred
    assert!(cache.size() < 1000);
}
```

**Parameterized tests**:

```rust
use cache_layer::EvictionPolicy;

#[tokio::test]
async fn test_eviction_policies() {
    let policies = vec![
        EvictionPolicy::LRU,
        EvictionPolicy::LFU,
        EvictionPolicy::FIFO,
    ];

    for policy in policies {
        let cache = MemoryCache::builder()
            .capacity(1000)
            .eviction_policy(policy.clone())
            .build();

        // Test eviction behavior
        test_eviction(&cache).await;
    }
}

async fn test_eviction(cache: &MemoryCache<u32, Vec<u8>>) {
    // Fill cache
    for i in 0..20 {
        cache.set(i, vec![0u8; 100], None).await.unwrap();
    }

    // Access some items
    cache.get(&5).await.unwrap();
    cache.get(&10).await.unwrap();

    // Add more to trigger eviction
    for i in 20..30 {
        cache.set(i, vec![0u8; 100], None).await.unwrap();
    }

    // Verify cache size constraint
    assert!(cache.size() <= 1000);
}
```

## Adding New Cache Tiers

### Step 1: Implement the Tier Trait

Create `src/mytier/cache.rs`:

```rust
use async_trait::async_trait;
use std::time::Duration;
use crate::tier::{Tier, TierStats};
use crate::error::{CacheError, Result};

pub struct MyTier {
    // Your tier-specific fields
    config: MyTierConfig,
    stats: TierStats,
}

#[async_trait]
impl<K, V> Tier<K, V> for MyTier
where
    K: Send + Sync,
    V: Send + Sync,
{
    async fn get(&self, key: &K) -> Result<Option<V>> {
        // Implement get logic
        Ok(None)
    }

    async fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<()> {
        // Implement set logic
        Ok(())
    }

    async fn delete(&self, key: &K) -> Result<()> {
        // Implement delete logic
        Ok(())
    }

    fn size(&self) -> usize {
        // Return current size
        0
    }

    fn capacity(&self) -> usize {
        self.config.capacity
    }

    async fn clear(&self) -> Result<()> {
        // Clear all entries
        Ok(())
    }

    fn stats(&self) -> TierStats {
        self.stats.clone()
    }
}
```

### Step 2: Create Configuration Struct

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyTierConfig {
    pub capacity: usize,
    pub connection_string: String,
    pub timeout_ms: u64,
}

impl Default for MyTierConfig {
    fn default() -> Self {
        Self {
            capacity: 1_000_000_000,  // 1GB
            connection_string: "mytier://localhost".to_string(),
            timeout_ms: 5000,
        }
    }
}
```

### Step 3: Add Builder Pattern

```rust
pub struct MyTierBuilder {
    config: MyTierConfig,
}

impl MyTierBuilder {
    pub fn new() -> Self {
        Self {
            config: MyTierConfig::default(),
        }
    }

    pub fn capacity(mut self, capacity: usize) -> Self {
        self.config.capacity = capacity;
        self
    }

    pub fn connection_string(mut self, url: impl Into<String>) -> Self {
        self.config.connection_string = url.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout_ms = timeout.as_millis() as u64;
        self
    }

    pub fn build(self) -> Result<MyTier> {
        // Validate configuration
        if self.config.capacity == 0 {
            return Err(CacheError::InvalidConfig(
                "capacity must be > 0".to_string()
            ));
        }

        // Create tier
        Ok(MyTier {
            config: self.config,
            stats: TierStats::default(),
        })
    }
}

impl Default for MyTierBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

### Step 4: Write Tests

Create `tests/mytier_tests.rs`:

```rust
use cache_layer::mytier::MyTier;
use cache_layer::Tier;

#[tokio::test]
async fn test_mytier_basic_operations() {
    let tier = MyTierBuilder::new()
        .capacity(1000)
        .build()
        .unwrap();

    // Test set/get
    tier.set("key", "value", None).await.unwrap();
    let result = tier.get(&"key").await.unwrap();
    assert_eq!(result, Some("value".to_string()));

    // Test delete
    tier.delete(&"key").await.unwrap();
    let result = tier.get(&"key").await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_mytier_ttl() {
    use std::time::Duration;

    let tier = MyTierBuilder::new()
        .capacity(1000)
        .build()
        .unwrap();

    // Set with 1ms TTL
    tier.set("key", "value", Some(Duration::from_millis(1))).await.unwrap();

    // Should exist immediately
    assert!(tier.get(&"key").await.unwrap().is_some());

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Should be expired
    assert!(tier.get(&"key").await.unwrap().is_none());
}
```

### Step 5: Add Documentation

```rust
//! MyTier - Custom cache tier implementation
//!
//! This tier provides caching using [MyBackend].
//!
//! # Performance
//!
//! - Read latency: ~Xms
//! - Write latency: ~Yms
//! - Capacity: Unlimited
//!
//! # Configuration
//!
//! ```rust
//! use cache_layer::mytier::MyTierBuilder;
//!
//! let tier = MyTierBuilder::new()
//!     .capacity(1_000_000_000)  // 1GB
//!     .connection_string("mytier://localhost")
//!     .timeout(Duration::from_secs(5))
//!     .build()?;
//! ```
//!
//! # Limitations
//!
//! - Requires external backend service
//! - Not as fast as in-memory cache
//! - Network latency affects performance
```

### Step 6: Export from Lib

Update `src/lib.rs`:

```rust
pub mod mytier;

pub use mytier::{MyTier, MyTierBuilder, MyTierConfig};
```

## Benchmarking

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench memory_bench

# Run with specific filter
cargo bench --bench memory_bench -- read

# Save benchmark results
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main
```

### Writing Benchmarks

Use Criterion for comprehensive benchmarking:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use cache_layer::{MultiTierCache, MemoryCache};

fn benchmark_memory_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_cache");

    // Benchmark different cache sizes
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let cache = MemoryCache::new(size * 1024).unwrap();

                b.iter(|| {
                    cache.get(black_box(&"test_key".to_string())).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn benchmark_set_operations(c: &mut Criterion) {
    c.bench_function("cache_set", |b| {
        let cache = MemoryCache::new(1_000_000).unwrap();

        b.iter(|| {
            cache.set(
                black_box("key"),
                black_box(vec![0u8; 1024]),
                None
            ).unwrap()
        });
    });
}

criterion_group!(benches, benchmark_memory_cache, benchmark_set_operations);
criterion_main!(benches);
```

### Benchmarking Methodology

**1. Warm-up Phase**:

```rust
// Warm up the cache before measuring
for i in 0..1000 {
    cache.set(&i.to_string(), vec![0u8; 1024], None).await?;
}

// Now measure
start_timer();
for i in 0..1000 {
    cache.get(&i.to_string()).await?;
}
stop_timer();
```

**2. Measure Throughput**:

```rust
let start = Instant::now();
let mut ops = 0;

for _ in 0..iterations {
    cache.get(&key).await?;
    ops += 1;
}

let elapsed = start.elapsed();
let throughput = ops as f64 / elapsed.as_secs_f64();
println!("Throughput: {:.2} ops/sec", throughput);
```

**3. Measure Latency Distribution**:

```rust
let mut latencies = Vec::new();

for _ in 0..iterations {
    let start = Instant::now();
    cache.get(&key).await?;
    let latency = start.elapsed();
    latencies.push(latency);
}

// Calculate percentiles
latencies.sort();
let p50 = latencies[latencies.len() / 2];
let p95 = latencies[(latencies.len() * 95) / 100];
let p99 = latencies[(latencies.len() * 99) / 100];

println!("P50: {:?}", p50);
println!("P95: {:?}", p95);
println!("P99: {:?}", p99);
```

### Profiling

**CPU Profiling with Flamegraph**:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bench memory_bench

# View flamegraph
open flamegraph.svg
```

**Memory Profiling**:

```bash
# Use valgrind (Linux)
cargo build --release
valgrind --tool=massif ./target/release/bench

# View massif results
ms_print massif.out.<pid>
```

### Continuous Benchmarking

Set up CI to track performance over time:

```yaml
# .github/workflows/bench.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:

jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench -- --save-baseline main

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: cargo
          output-file-path: benchmark.txt
```

## Release Process

### Version Bump

```bash
# Update version in Cargo.toml
cargo edit version 0.2.0

# Update CHANGELOG.md
vim CHANGELOG.md

# Commit changes
git add Cargo.toml CHANGELOG.md
git commit -m "Bump version to 0.2.0"
```

### Release Checklist

- [ ] All tests passing
- [ ] Benchmarks run and documented
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version numbers updated
- [ ] Tagged commit
- [ ] Published to crates.io
- [ ] GitHub release created

### Creating a Release

```bash
# Run release script
./scripts/release.sh 0.2.0

# This will:
# 1. Run all tests
# 2. Update version numbers
# 3. Create git tag
# 4. Publish to crates.io
# 5. Create GitHub release
```

### Post-Release

```bash
# Update main branch to next version
git checkout main
cargo edit version 0.3.0-alpha.1
git add Cargo.toml
git commit -m "Start 0.3.0 development"
git push
```

## Code Style

### Rust Style Guidelines

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy --all-targets --all-features

# Fix clippy warnings
cargo clippy --fix --all-targets --all-features
```

### Naming Conventions

**Types**: PascalCase
```rust
pub struct MultiTierCache { }
pub enum EvictionPolicy { }
```

**Functions**: snake_case
```rust
pub fn get_cache(&self) -> Result<Cache> { }
pub async fn set_value(&mut self, key: K, value: V) { }
```

**Constants**: SCREAMING_SNAKE_CASE
```rust
pub const DEFAULT_CAPACITY: usize = 100_000_000;
pub const MAX_RETRIES: u32 = 3;
```

### Documentation Style

```rust
//! # Module-level documentation
//!
//! This module does X, Y, Z.

/// Brief summary of what this function does.
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `key` - The cache key
/// * `value` - The value to cache
///
/// # Returns
///
/// Returns `Ok(())` on success, `Err(CacheError)` on failure.
///
/// # Examples
///
/// ```
/// use cache_layer::Cache;
///
/// let result = cache.set("key", "value").await?;
/// # Ok::<(), CacheError>(())
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// - The cache is full
/// - Serialization fails
pub async fn set(&self, key: K, value: V) -> Result<()> {
    // Implementation
}
```

### Error Handling

Use the `Result` type for fallible operations:

```rust
use crate::error::{CacheError, Result};

pub async fn get(&self, key: &K) -> Result<Option<V>> {
    if key.is_empty() {
        return Err(CacheError::InvalidKey(
            "key cannot be empty".to_string()
        ));
    }

    // ... implementation
}
```

## Contributing Workflow

1. **Fork and clone** the repository
2. **Create a branch**: `git checkout -b feature/my-feature`
3. **Make changes** and write tests
4. **Run tests**: `cargo test --all`
5. **Run benchmarks**: `cargo bench`
6. **Format code**: `cargo fmt`
7. **Check clippy**: `cargo clippy`
8. **Commit**: `git commit -m "Add my feature"`
9. **Push**: `git push origin feature/my-feature`
10. **Open PR** on GitHub

## Getting Help

- **Documentation**: See `/docs` directory
- **Examples**: See `/examples` directory
- **Issues**: Open an issue on GitHub
- **Discussions**: Use GitHub Discussions for questions
