# User Guide

Complete guide to installing, configuring, and using cache-layer.

## Table of Contents

1. [Installation](#installation)
2. [Basic Usage](#basic-usage)
3. [Advanced Usage](#advanced-usage)
4. [Configuration](#configuration)
5. [Performance Tuning](#performance-tuning)
6. [Monitoring](#monitoring)
7. [Troubleshooting](#troubleshooting)

## Installation

### Rust

Add to `Cargo.toml`:

```toml
[dependencies]
cache-layer = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

### Go

```bash
go get github.com/equilibrium-tokens/cache-layer-go
```

```go
import (
    cachelayer "github.com/equilibrium-tokens/cache-layer-go"
)

func main() {
    cache := cachelayer.NewMultiTierCache().
        WithL1(cachelayer.NewMemoryCache(100_000_000)).  // 100MB
        Build()
}
```

### System Requirements

**Minimum**:
- CPU: 2 cores
- Memory: 4GB RAM
- Disk: 10GB free space

**Recommended**:
- CPU: 4+ cores
- Memory: 16GB+ RAM
- Disk: 100GB+ NVMe SSD
- Redis: 6.0+ (for L2 cache)

## Basic Usage

### Creating a Cache

```rust
use cache_layer::{MultiTierCache, MemoryCache, RedisCache, DiskCache};

// Simple memory-only cache
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)  // 100MB
    .build();

// Memory + Redis cache
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)
    .with_l2(RedisCache::new("redis://localhost:6379")?)
    .build();

// Full three-tier cache
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)
    .with_l2(RedisCache::new("redis://localhost:6379")?)
    .with_l3(DiskCache::new("/var/cache/myapp")?)
    .build();
```

### Get, Set, Delete

```rust
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

// Store a value
let user = User {
    id: 123,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};
cache.set(&"user:123".to_string(), user).await?;

// Retrieve a value
if let Some(user) = cache.get(&"user:123".to_string()).await? {
    println!("User: {}", user.name);
}

// Delete a value
cache.delete(&"user:123".to_string()).await?;

// Check existence
let exists = cache.exists(&"user:123".to_string()).await?;
println!("User exists: {}", exists);
```

### Type Safety

cache-layer works with any type that implements `Serialize` and `Deserialize`:

```rust
// Simple types
cache.set("counter", 42).await?;
cache.set("pi", 3.14159).await?;
cache.set("active", true).await?;
cache.set("name", "Alice".to_string()).await?;

// Complex types
#[derive(Clone, Serialize, Deserialize)]
struct Config {
    database_url: String,
    max_connections: u32,
    timeout_ms: u64,
}
cache.set("config", config).await?;

// Collections
let users = vec![user1, user2, user3];
cache.set("users", users).await?;

// HashMaps
let mut index = HashMap::new();
index.insert("alice", 1);
index.insert("bob", 2);
cache.set("index", index).await?;
```

## Advanced Usage

### TTL (Time-To-Live)

Set expiration times for cached items:

```rust
use std::time::Duration;

// Set with 1 hour TTL
cache.set_with_ttl(
    "session:abc123",
    session_data,
    Duration::from_secs(3600)
).await?;

// Set default TTL for the cache
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)
    .with_ttl(Duration::from_secs(1800))  // 30 minutes default
    .build();

// Override default TTL for specific operation
cache.set_with_ttl(
    "critical_data",
    data,
    Duration::from_secs(60)  // Only 1 minute
).await?;
```

**TTL Best Practices**:
- Short TTL (seconds-minutes): Session data, real-time data
- Medium TTL (hours-days): User profiles, configuration
- Long TTL (days-weeks): Static content, embeddings
- No TTL: Immutable data, reference data

### Eviction Policies

Configure cache replacement strategies:

```rust
use cache_layer::{MemoryCache, EvictionPolicy};

// LRU (Least Recently Used) - Default
let cache = MemoryCache::builder()
    .capacity(100_000_000)
    .eviction_policy(EvictionPolicy::LRU)
    .build();

// LFU (Least Frequently Used)
let cache = MemoryCache::builder()
    .capacity(100_000_000)
    .eviction_policy(EvictionPolicy::LFU)
    .build();

// FIFO (First In First Out)
let cache = MemoryCache::builder()
    .capacity(100_000_000)
    .eviction_policy(EvictionPolicy::FIFO)
    .build();
```

**Policy Selection Guide**:
- **LRU**: Default, works well for most cases
- **LFU**: When access patterns are consistent over time
- **FIFO**: When simplicity is more important than optimization

### Multi-Tier Configuration

Fine-tune each tier independently:

```rust
let cache = MultiTierCache::new()
    // L1: 500MB, LRU eviction
    .with_l1(
        MemoryCache::builder()
            .capacity(500_000_000)  // 500MB
            .eviction_policy(EvictionPolicy::LRU)
            .build()
    )
    // L2: Redis with cluster
    .with_l2(
        RedisCache::builder()
            .url("redis://localhost:6379")
            .key_prefix("myapp:")
            .pool_size(10)
            .default_ttl(Duration::from_secs(3600))
            .build()
    )
    // L3: Disk with compression
    .with_l3(
        DiskCache::builder()
            .path("/var/cache/myapp")
            .compression(true)  // Use zstd compression
            .cleanup_interval(Duration::from_secs(300))  // 5 minutes
            .build()
    )
    // Overall settings
    .with_ttl(Duration::from_secs(1800))
    .with_metrics(true)
    .build();
```

### Cache Warming

Pre-populate cache with frequently accessed data:

```rust
async fn warm_cache(
    cache: &MultiTierCache<String, User>,
    user_ids: Vec<u64>,
) -> Result<()> {
    println!("Warming cache with {} users...", user_ids.len());

    let mut warmed = 0;
    for user_id in user_ids {
        let key = format!("user:{}", user_id);

        // Fetch from database
        let user = fetch_user_from_db(user_id).await?;

        // Store in cache
        cache.set(&key, user).await?;
        warmed += 1;

        // Progress indicator
        if warmed % 100 == 0 {
            println!("Warmed {} users...", warmed);
        }
    }

    println!("Cache warming complete: {} entries", warmed);
    Ok(())
}

// Usage
warm_cache(&cache, vec![1, 2, 3, 4, 5]).await?;
```

### Batch Operations

Set multiple items efficiently:

```rust
use std::collections::HashMap;

// Batch set
let mut batch = HashMap::new();
batch.insert("key1".to_string(), value1);
batch.insert("key2".to_string(), value2);
batch.insert("key3".to_string(), value3);
cache.set_batch(batch).await?;

// Batch get
let keys = vec![
    "key1".to_string(),
    "key2".to_string(),
    "key3".to_string(),
];
let values = cache.get_batch(keys).await?;

// Batch delete
let keys_to_delete = vec![
    "old_key1".to_string(),
    "old_key2".to_string(),
];
cache.delete_batch(keys_to_delete).await?;
```

### Atomic Operations

Perform operations atomically:

```rust
use cache_layer::AtomicCache;

// Get or set (cache-aside pattern)
let value = cache.get_or_insert(
    "expensive_computation",
    || expensive_computation()
).await?;

// Compare and swap
let updated = cache.compare_and_swap(
    "counter",
    &old_value,
    new_value
).await?;

// Get and delete (pop)
if let Some(value) = cache.get_and_delete("temp_key").await? {
    println!("Got and removed: {:?}", value);
}
```

## Configuration

### Environment Variables

```bash
# Redis configuration
export CACHE_REDIS_URL="redis://localhost:6379"
export CACHE_REDIS_POOL_SIZE=10
export CACHE_REDIS_KEY_PREFIX="myapp:"

# Disk cache configuration
export CACHE_DISK_PATH="/var/cache/myapp"
export CACHE_DISK_COMPRESSION=true
export CACHE_DISK_CLEANUP_INTERVAL=300

# Default TTL (seconds)
export CACHE_DEFAULT_TTL=1800

# Metrics
export CACHE_METRICS_ENABLED=true
export CACHE_METRICS_PORT=9090
```

### Configuration File

`cache_config.toml`:

```toml
[cache_l1]
enabled = true
capacity_mb = 500
eviction_policy = "LRU"  # LRU, LFU, or FIFO

[cache_l2]
enabled = true
url = "redis://localhost:6379"
pool_size = 10
key_prefix = "myapp:"
default_ttl_sec = 3600

[cache_l3]
enabled = true
path = "/var/cache/myapp"
compression = true
cleanup_interval_sec = 300

[general]
default_ttl_sec = 1800
metrics_enabled = true
metrics_port = 9090
```

Load configuration:

```rust
use cache_layer::Config;

let config = Config::from_file("cache_config.toml")?;
let cache = MultiTierCache::from_config(config)?;
```

## Performance Tuning

### Capacity Planning

**Calculate L1 size** (target 80% hit rate):
```
L1_size = (working_set_size * 0.8) / avg_item_size

Example:
- Working set: 10,000 users
- Avg user size: 1KB
- L1_size = (10,000 * 0.8) / 1024 = ~8MB
```

**Calculate L2 size** (target 15% hit rate):
```
L2_size = (working_set_size * 0.15) / avg_item_size

Example:
- Working set: 10,000 users
- L2_size = (10,000 * 0.15) / 1024 = ~1.5MB
```

**Calculate L3 size** (full working set):
```
L3_size = working_set_size * avg_item_size * growth_factor

Example:
- Working set: 10,000 users
- Growth factor: 2x
- L3_size = 10,000 * 1024 * 2 = ~20MB
```

### Redis Tuning

```bash
# redis.conf
maxmemory 10gb
maxmemory-policy allkeys-lru
save ""  # Disable persistence for cache use
appendonly no
tcp-backlog 511
tcp-keepalive 300
```

### Disk Cache Tuning

```rust
let disk_cache = DiskCache::builder()
    .path("/var/cache/myapp")
    .compression(true)  // 3-5x space savings
    .compression_level(3)  // 1-21, default 3 (good balance)
    .cleanup_interval(Duration::from_secs(300))
    .max_file_size(1_000_000_000)  // 1GB per file
    .build();
```

### Connection Pooling

```rust
let redis_cache = RedisCache::builder()
    .url("redis://localhost:6379")
    .pool_size(10)  // Adjust based on concurrency
    .min_idle(2)  // Keep 2 connections ready
    .connection_timeout(Duration::from_secs(5))
    .build();
```

### Async Runtime

```rust
// Use multi-threaded runtime for better performance
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(100_000_000)?)
        .build();

    // Your application code
    Ok(())
}
```

## Monitoring

### Built-in Metrics

```rust
use cache_layer::CacheMetrics;

// Get metrics snapshot
let metrics = cache.metrics();

println!("L1 hit rate: {:.2}%", metrics.l1_hit_rate() * 100.0);
println!("L2 hit rate: {:.2}%", metrics.l2_hit_rate() * 100.0);
println!("L3 hit rate: {:.2}%", metrics.l3_hit_rate() * 100.0);
println!("Overall hit rate: {:.2}%", metrics.overall_hit_rate() * 100.0);
println!("Total operations: {}", metrics.total_ops());
println!("Average latency: {:?}", metrics.avg_latency());
```

### Prometheus Export

```rust
use cache_layer::metrics::PrometheusExporter;

let exporter = PrometheusExporter::new(cache.clone())
    .with_port(9090)
    .start()
    .await?;

// Metrics available at http://localhost:9090/metrics
```

**Example metrics output**:
```
# HELP cache_hits_total Total number of cache hits
# TYPE cache_hits_total counter
cache_hits_total{tier="l1"} 1234567
cache_hits_total{tier="l2"} 234567
cache_hits_total{tier="l3"} 12345

# HELP cache_misses_total Total number of cache misses
# TYPE cache_misses_total counter
cache_misses_total{tier="l1"} 234567
cache_misses_total{tier="l2"} 12345
cache_misses_total{tier="l3"} 1234

# HELP cache_latency_seconds Cache operation latency
# TYPE cache_latency_seconds histogram
cache_latency_seconds_bucket{tier="l1",le="0.0001"} 1234567
cache_latency_seconds_bucket{tier="l1",le="0.001"} 1234567
cache_latency_seconds_bucket{tier="l1",le="+Inf"} 1234567
```

### Logging

```rust
use cache_layer::log;

// Set log level
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

// Logs will include:
// - Cache hits/misses
// - Tier operations
// - Eviction events
// - Errors and warnings
```

### Health Checks

```rust
use cache_layer::HealthCheck;

// Check cache health
let health = cache.health_check().await?;

if !health.l1_healthy {
    eprintln!("L1 cache is unhealthy!");
}

if !health.l2_healthy {
    eprintln!("L2 cache is unhealthy!");
}

println!("Cache status: {}", health.status);
```

## Troubleshooting

### Common Issues

#### 1. Low Hit Rate

**Problem**: Cache hit rate below 50%

**Solutions**:
- Increase L1 capacity
- Check if TTL is too short
- Review access patterns for locality
- Consider different eviction policy

```rust
// Increase L1 capacity
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(500_000_000)?)  // Was 100MB
    .build();

// Check current hit rate
let metrics = cache.metrics();
println!("Current hit rate: {:.2}%", metrics.overall_hit_rate() * 100.0);
```

#### 2. High Memory Usage

**Problem**: Cache using more memory than expected

**Solutions**:
- Enable compression on L3
- Reduce TTL
- Check for memory leaks in cached values

```rust
// Enable compression
let cache = MultiTierCache::new()
    .with_l3(
        DiskCache::builder()
            .path("/var/cache/myapp")
            .compression(true)
            .build()
    )
    .build();

// Monitor memory usage
let size = cache.l1_size();
println!("L1 size: {} bytes", size);
```

#### 3. Redis Connection Failures

**Problem**: "Connection refused" errors from Redis

**Solutions**:
- Verify Redis is running
- Check connection URL
- Configure connection pool

```rust
// Test Redis connection
let redis_client = redis::Client::open("redis://localhost:6379")?;
let conn = redis_client.get_connection()?;
let pong: String = redis::cmd("PING").query(&conn)?;
assert_eq!(pong, "PONG");

// Configure retry logic
let cache = MultiTierCache::new()
    .with_l2(
        RedisCache::builder()
            .url("redis://localhost:6379")
            .max_retries(3)
            .retry_delay(Duration::from_millis(100))
            .build()
    )
    .build();
```

#### 4. Slow Cache Operations

**Problem**: Cache operations taking longer than expected

**Solutions**:
- Check for lock contention
- Profile with flamegraph
- Use batch operations

```rust
// Use batch operations for better performance
let keys = vec![/* ... */];
let values = cache.get_batch(keys).await?;

// Enable metrics to identify bottlenecks
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)
    .with_metrics(true)
    .build();

// Review metrics
let metrics = cache.metrics();
println!("Avg latency: {:?}", metrics.avg_latency());
```

### Debug Mode

Enable detailed logging:

```rust
use cache_layer::DebugCache;

let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)
    .with_debug(true)  // Enable debug logging
    .build();

// Now every operation will be logged
cache.get(&key).await?;
// [DEBUG] L1 get: key = "user:123", result = miss
// [DEBUG] L2 get: key = "user:123", result = hit
// [DEBUG] Promoting to L1: key = "user:123"
```

### Performance Profiling

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_cache(c: &mut Criterion) {
    let cache = setup_cache();

    c.bench_function("cache_get", |b| {
        b.iter(|| {
            cache.get(&"test_key".to_string()).await.unwrap()
        })
    });
}

criterion_group!(benches, benchmark_cache);
criterion_main!(benches);
```

## Best Practices

1. **Start simple**: Use memory-only cache first, add tiers as needed
2. **Monitor metrics**: Track hit rates and latency
3. **Use appropriate TTL**: Balance freshness with hit rate
4. **Warm critical data**: Pre-populate cache with hot data
5. **Handle failures gracefully**: Cache failures shouldn't crash your app
6. **Size appropriately**: 80% hit rate target for L1
7. **Use compression**: Enable on L3 for space savings
8. **Batch operations**: More efficient than individual ops
9. **Profile before optimizing**: Measure actual bottlenecks
10. **Test under load**: Verify performance at production scale

## Example Applications

See [INTEGRATION.md](INTEGRATION.md) for complete integration examples with:
- vector-navigator
- embeddings-engine
- semantic-store
- General-purpose caching
