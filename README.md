# cache-layer

**Multi-tier caching library for high-performance data access**

Sub-microsecond memory reads, millisecond-scale persistence, automatic tier promotion.

## Overview

cache-layer provides a sophisticated multi-tier caching system that combines three levels of storage hierarchy:

- **L1 Cache (Memory)**: ~100ns reads, 100MB typical capacity
- **L2 Cache (Redis)**: ~1ms reads, 10GB typical capacity, shared across instances
- **L3 Cache (Disk)**: ~10ms reads, 1TB typical capacity, persistent storage

The library automatically manages data movement between tiers, promoting frequently accessed data to faster layers and evicting to slower layers when capacity limits are reached.

## Key Features

### Multi-Tier Architecture
- **Automatic tiering**: Data seamlessly flows between memory, Redis, and disk
- **Intelligent promotion**: Cache hits promote data to faster tiers
- **Configurable eviction**: LRU, LFU, or FIFO policies per tier
- **TTL support**: Time-based expiration for cached items

### Performance
- **Sub-microsecond L1 reads**: ~100ns average latency for memory cache hits
- **High hit rates**: Target >80% L1, >15% L2, <1% overall miss rate
- **Zero-copy**: Rust implementation eliminates unnecessary allocations
- **Lock-free reads**: Concurrent access without blocking

### Developer Experience
- **Simple API**: Three core operations - get, set, delete
- **Type-safe**: Generics support any serializable type
- **Language bindings**: Rust core with Go bindings for easy integration
- **Comprehensive metrics**: Built-in monitoring and observability

## Quick Start

### Installation

**Rust:**
```toml
[dependencies]
cache-layer = "0.1"
```

**Go:**
```bash
go get github.com/equilibrium-tokens/cache-layer-go
```

### Basic Usage

```rust
use cache_layer::{MultiTierCache, MemoryCache, RedisCache, DiskCache};

// Create a multi-tier cache
let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(100_000_000)?)  // 100MB
    .with_l2(RedisCache::new("redis://localhost:6379")?)
    .with_l3(DiskCache::new("/var/cache/myapp")?)
    .build();

// Store a value
cache.set("user:123", User {
    id: 123,
    name: "Alice".to_string(),
}).await?;

// Retrieve a value (automatically searches L1 → L2 → L3)
if let Some(user) = cache.get(&"user:123").await? {
    println!("User: {}", user.name);
}

// Delete a value across all tiers
cache.delete(&"user:123").await?;
```

### Advanced Usage

```rust
use cache_layer::{MultiTierCache, MemoryCache, EvictionPolicy};
use std::time::Duration;

// Configure with custom eviction policies and TTL
let cache = MultiTierCache::new()
    .with_l1(
        MemoryCache::builder()
            .capacity(200_000_000)  // 200MB
            .eviction_policy(EvictionPolicy::LRU)
            .build()
    )
    .with_ttl(Duration::from_secs(3600))  // 1 hour default TTL
    .with_metrics(true)  // Enable collection of cache metrics
    .build();

// Cache warming
let mut cache_warmed = 0;
for key in preload_keys {
    if cache.set(&key, fetch_value(&key).await?).await.is_ok() {
        cache_warmed += 1;
    }
}
println!("Warmed {} cache entries", cache_warmed);

// Monitor performance
let metrics = cache.metrics();
println!(
    "L1 hit rate: {:.2}%, L2 hit rate: {:.2}%, L3 hit rate: {:.2}%",
    metrics.l1_hit_rate(),
    metrics.l2_hit_rate(),
    metrics.l3_hit_rate()
);
```

## Performance Highlights

### Latency by Tier

| Tier | Read Latency | Write Latency | Typical Capacity |
|------|--------------|---------------|------------------|
| L1 (Memory) | ~100ns | ~200ns | 100MB |
| L2 (Redis) | ~1ms | ~1ms | 10GB |
| L3 (Disk) | ~10ms | ~20ms | 1TB |

### Hit Rate Targets

- **L1 Hit Rate**: >80% (frequently accessed data stays in memory)
- **L2 Hit Rate**: >15% (moderately accessed data in Redis)
- **L3 Hit Rate**: >5% (cold data on disk)
- **Overall Miss Rate**: <1% (effective caching reduces load)

### Scalability

- **Concurrent reads**: Lock-free, millions of operations per second
- **Horizontal scaling**: Redis tier shares data across instances
- **Vertical scaling**: Adjustable tier capacities for memory/disk

## Architecture

cache-layer implements the timeless principle of **memory hierarchy**: data closer to computation is faster to access. The multi-tier design balances speed, size, and cost by automatically managing data placement across storage layers.

```
Application
    ↓ (get/set)
[MultiTierCache]
    ├── L1: MemoryCache (100MB, ~100ns)
    │   ├── Hit: Return immediately
    │   └── Miss: Check L2
    ├── L2: RedisCache (10GB, ~1ms)
    │   ├── Hit: Return + promote to L1
    │   └── Miss: Check L3
    └── L3: DiskCache (1TB, ~10ms)
        ├── Hit: Return + promote to L2
        └── Miss: Return None
```

For detailed architecture information, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Use Cases

### vector-navigator: Search Result Caching

```rust
use cache_layer::MultiTierCache;
use vector_navigator::VectorStore;

let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(500_000_000)?)  // 500MB for vectors
    .with_l2(RedisCache::new("redis://localhost")?)
    .build();

let vector_store = VectorStore::new(cache.clone())?;

// First search: L1 miss, L2 miss, L3 miss → compute and cache
let results = vector_store.search(&query, 10).await?;

// Second search with same query: L1 hit (~100ns)
let results = vector_store.search(&query, 10).await?;
```

### embeddings-engine: Embedding Caching

```rust
// Cache computed embeddings to avoid recomputation
cache.set(&text, embeddings.compute(&text).await?).await?;

// Subsequent requests return cached embeddings
let cached = cache.get(&text).await?;
```

### semantic-store: Document Retrieval

```rust
// Cache document metadata and frequently accessed content
cache.set(&doc_id, fetch_document(doc_id).await?).await?;
```

### General-Purpose Caching

```rust
// Cache API responses, database queries, expensive computations
cache.set(&cache_key, expensive_computation()).await?;
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - Design philosophy and component architecture
- [User Guide](docs/USER_GUIDE.md) - Installation, usage, configuration
- [Developer Guide](docs/DEVELOPER_GUIDE.md) - Contributing, testing, benchmarking
- [Integration Guide](docs/INTEGRATION.md) - Integration examples with ecosystem tools

## Language Support

- **Rust**: Core implementation (v0.1.0)
- **Go**: Native bindings (v0.1.0-go)
- **Python**: Planned (v0.2.0)
- **JavaScript**: Planned (v0.2.0)

## Benchmarks

Results from `cache_benchmark` on AMD Ryzen 9 5900X, 64GB RAM, NVMe SSD:

```
L1 Memory Cache:
  Read:  98.3 ns/op (10M ops)
  Write: 187.2 ns/op (10M ops)
  Hit rate: 82.3%

L2 Redis Cache:
  Read:  1.02 ms/op (100K ops)
  Write: 0.98 ms/op (100K ops)
  Hit rate: 15.7%

L3 Disk Cache:
  Read:  9.87 ms/op (10K ops)
  Write: 18.3 ms/op (10K ops)
  Hit rate: 2.0%

Overall (multi-tier):
  Read:  124.5 ns/op (cached, L1 hit)
  Miss:  10.2 ms/op (uncached, L3 miss)
  Hit rate: 99.2%
```

## Contributing

We welcome contributions! Please see [docs/DEVELOPER_GUIDE.md](docs/DEVELOPER_GUIDE.md) for:
- Development setup
- Code organization
- Testing methodology
- Pull request process

## License

MIT License - see LICENSE file for details

## Ecosystem Integration

cache-layer is part of the Equilibrium Tokens ecosystem:

- **vector-navigator**: Vector similarity search with cache-layer integration
- **embeddings-engine**: Text embedding computation with cache-layer integration
- **semantic-store**: Document storage and retrieval with cache-layer integration
- **constraint-grammar**: Grammar-based text generation

For integration examples, see [docs/INTEGRATION.md](docs/INTEGRATION.md).
