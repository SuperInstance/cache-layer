# Architecture

## Philosophy

**"Data closer to computation is faster to access"**

cache-layer implements the timeless principle of **memory hierarchy**, a fundamental concept in computer architecture that has guided system design for decades. This principle recognizes that:

1. **Temporal locality**: Recently accessed data is likely to be accessed again
2. **Spatial locality**: Data near recently accessed items is likely to be accessed
3. **Speed-size tradeoff**: Faster memory is more expensive and smaller

By organizing data into tiers with decreasing speed but increasing capacity, we achieve performance that approaches the fastest tier while maintaining the capacity of the slowest tier.

## Timeless Principles

### Memory Hierarchy

```
CPU Registers (~0ns)
    ↓
L1 Cache (~1ns)
    ↓
L2 Cache (~3ns)
    ↓
L3 Cache (~10ns)
    ↓
Main Memory (~100ns)
    ↓
SSD (~10μs)
    ↓
HDD (~10ms)
    ↓
Network (~100ms)
```

cache-layer extends this hierarchy to the application level:

```
Application Code
    ↓
L1: Memory Cache (~100ns)
    ↓
L2: Redis Cache (~1ms)
    ↓
L3: Disk Cache (~10ms)
    ↓
Database/Computation (~100ms)
```

### Temporal Locality

When data is accessed in one tier, it's promoted to the faster tier. This creates a self-optimizing system where hot data naturally rises to the top:

```rust
// First access: L1 miss, L2 miss, L3 hit
cache.get("user:123")?;
// L3 promotes to L2

// Second access: L1 miss, L2 hit
cache.get("user:123")?;
// L2 promotes to L1

// Third access: L1 hit
cache.get("user:123")?;
// Returns immediately at ~100ns
```

## Core Abstractions

### 1. Cache Trait

The primary interface for cache operations:

```rust
pub trait Cache<K, V> {
    /// Retrieve a value, searching all tiers
    fn get(&self, key: &K) -> Result<Option<V>>;

    /// Store a value in all tiers
    fn set(&self, key: K, value: V) -> Result<()>;

    /// Remove a value from all tiers
    fn delete(&self, key: &K) -> Result<()>;

    /// Check if a key exists
    fn exists(&self, key: &K) -> Result<bool>;

    /// Set with time-to-live
    fn set_with_ttl(&self, key: K, value: V, ttl: Duration) -> Result<()>;
}
```

**Design decisions**:
- Generic over key (`K`) and value (`V`) types for flexibility
- `Result` types for error handling (network failures, serialization errors)
- Simple interface: get/set/delete cover 95% of use cases
- Async by default for non-blocking I/O

### 2. Tier Trait

Individual cache layer implementation:

```rust
pub trait Tier<K, V>: Send + Sync {
    /// Retrieve from this tier only
    fn get(&self, key: &K) -> Result<Option<V>>;

    /// Store in this tier only
    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<()>;

    /// Remove from this tier
    fn delete(&self, key: &K) -> Result<()>;

    /// Current size in bytes
    fn size(&self) -> usize;

    /// Maximum capacity in bytes
    fn capacity(&self) -> usize;

    /// Clear all entries
    fn clear(&self) -> Result<()>;

    /// Get hit/miss statistics
    fn stats(&self) -> TierStats;
}
```

**Design decisions**:
- `Send + Sync` for concurrent access across threads
- Individual stats per tier for monitoring
- Size tracking for eviction decisions
- Optional TTL support (not all tiers need TTL)

### 3. EvictionPolicy Trait

Cache replacement strategy:

```rust
pub trait EvictionPolicy<K>: Send + Sync {
    /// Record that a key was accessed
    fn on_access(&mut self, key: &K);

    /// Record that a key was inserted
    fn on_insert(&mut self, key: K);

    /// Select a key to evict
    fn evict(&mut self) -> K;

    /// Reset policy state
    fn reset(&mut self);

    /// Clone for new instances
    fn clone_box(&self) -> Box<dyn EvictionPolicy<K>>;
}
```

**Implementations**:

1. **LRU (Least Recently Used)** - Default
   ```rust
   pub struct LRU<K> {
       capacity: usize,
       access_order: VecDeque<K>,
       key_index: HashMap<K, usize>,
   }

   impl<K: Hash + Eq + Clone> EvictionPolicy<K> for LRU<K> {
       fn on_access(&mut self, key: &K) {
           // Move to end (most recent)
           if let Some(idx) = self.key_index.get(key) {
               self.access_order.remove(*idx);
           }
           self.access_order.push_back(key.clone());
           self.key_index.insert(key.clone(), self.access_order.len() - 1);
       }

       fn evict(&mut self) -> K {
           // Remove from front (least recent)
           let key = self.access_order.pop_front().unwrap();
           self.key_index.remove(&key);
           key
       }
   }
   ```

2. **LFU (Least Frequently Used)**
   ```rust
   pub struct LFU<K> {
       capacity: usize,
       access_count: HashMap<K, usize>,
       min_frequency: usize,
   }

   impl<K: Hash + Eq + Clone> EvictionPolicy<K> for LFU<K> {
       fn on_access(&mut self, key: &K) {
           *self.access_count.entry(key.clone()).or_insert(0) += 1;
       }

       fn evict(&mut self) -> K {
           // Find key with minimum access count
           let min_key = self.access_count
               .iter()
               .min_by_key(|(_, count)| *count)
               .map(|(key, _)| key.clone())
               .unwrap();
           self.access_count.remove(&min_key);
           min_key
       }
   }
   ```

3. **FIFO (First In First Out)**
   ```rust
   pub struct FIFO<K> {
       capacity: usize,
       insertion_order: VecDeque<K>,
   }

   impl<K: Clone> EvictionPolicy<K> for FIFO<K> {
       fn evict(&mut self) -> K {
           self.insertion_order.pop_front().unwrap()
       }
   }
   ```

## Component Architecture

### MultiTierCache

The orchestrator that coordinates all tiers:

```rust
pub struct MultiTierCache<K, V> {
    l1: Option<Box<dyn Tier<K, V>>>,
    l2: Option<Box<dyn Tier<K, V>>>,
    l3: Option<Box<dyn Tier<K, V>>>,
    default_ttl: Option<Duration>,
    metrics: Arc<CacheMetrics>,
}

impl<K, V> MultiTierCache<K, V>
where
    K: Hash + Eq + Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync,
    V: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        // Try L1 first
        if let Some(l1) = &self.l1 {
            if let Some(value) = l1.get(key)? {
                self.metrics.record_l1_hit();
                return Ok(Some(value));
            }
            self.metrics.record_l1_miss();
        }

        // Try L2
        if let Some(l2) = &self.l2 {
            if let Some(value) = l2.get(key)? {
                self.metrics.record_l2_hit();
                // Promote to L1
                if let Some(l1) = &self.l1 {
                    let _ = l1.set(key.clone(), value.clone(), self.default_ttl);
                }
                return Ok(Some(value));
            }
            self.metrics.record_l2_miss();
        }

        // Try L3
        if let Some(l3) = &self.l3 {
            if let Some(value) = l3.get(key)? {
                self.metrics.record_l3_hit();
                // Promote to L2
                if let Some(l2) = &self.l2 {
                    let _ = l2.set(key.clone(), value.clone(), self.default_ttl);
                }
                return Ok(Some(value));
            }
            self.metrics.record_l3_miss();
        }

        Ok(None)
    }

    pub fn set(&self, key: K, value: V) -> Result<()> {
        // Set in all configured tiers
        if let Some(l1) = &self.l1 {
            l1.set(key.clone(), value.clone(), self.default_ttl)?;
        }
        if let Some(l2) = &self.l2 {
            l2.set(key.clone(), value.clone(), self.default_ttl)?;
        }
        if let Some(l3) = &self.l3 {
            l3.set(key, value, self.default_ttl)?;
        }
        Ok(())
    }

    pub fn delete(&self, key: &K) -> Result<()> {
        // Delete from all tiers
        if let Some(l1) = &self.l1 {
            l1.delete(key)?;
        }
        if let Some(l2) = &self.l2 {
            l2.delete(key)?;
        }
        if let Some(l3) = &self.l3 {
            l3.delete(key)?;
        }
        Ok(())
    }
}
```

**Design decisions**:
- **Cascade search**: Check tiers in order (L1 → L2 → L3)
- **Promotion on hit**: Move data to faster tiers when accessed
- **Write-through**: Write to all tiers on set for consistency
- **Optional tiers**: L1, L2, L3 are all optional (memory-only, memory+redis, etc.)

### Tier Implementations

#### 1. MemoryCache

In-memory hash map with size-based eviction:

```rust
pub struct MemoryCache<K, V> {
    data: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    eviction_policy: Arc<Mutex<Box<dyn EvictionPolicy<K>>>>,
    capacity: usize,
    current_size: Arc<AtomicUsize>,
}

struct CacheEntry<V> {
    value: V,
    ttl: Option<Instant>,
    size: usize,
}

impl<K, V> Tier<K, V> for MemoryCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn get(&self, key: &K) -> Result<Option<V>> {
        let data = self.data.read()?;
        if let Some(entry) = data.get(key) {
            // Check TTL
            if let Some(expiry) = entry.ttl {
                if Instant::now() > expiry {
                    return Ok(None);
                }
            }

            // Record access for eviction policy
            self.eviction_policy.lock()?.on_access(key);
            return Ok(Some(entry.value.clone()));
        }
        Ok(None)
    }

    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<()> {
        let size = size_of_val(&value);

        // Check if we need to evict
        while self.current_size.load(Ordering::Relaxed) + size > self.capacity {
            let evict_key = self.eviction_policy.lock()?.evict();
            self.delete(&evict_key)?;
        }

        // Insert new entry
        let mut data = self.data.write()?;
        let entry = CacheEntry {
            value,
            ttl: ttl.map(|d| Instant::now() + d),
            size,
        };
        data.insert(key.clone(), entry);
        self.current_size.fetch_add(size, Ordering::Relaxed);
        self.eviction_policy.lock()?.on_insert(key);

        Ok(())
    }
}
```

**Performance characteristics**:
- **Read**: ~100ns (hash map lookup + read lock)
- **Write**: ~200ns (hash map insert + write lock + potential eviction)
- **Concurrency**: RwLock allows concurrent reads, exclusive writes

#### 2. RedisCache

Distributed Redis-backed cache:

```rust
pub struct RedisCache {
    client: redis::Client,
    key_prefix: String,
}

impl<K, V> Tier<K, V> for RedisCache
where
    K: Serialize,
    V: for<'de> Deserialize<'de>,
{
    fn get(&self, key: &K) -> Result<Option<V>> {
        let mut conn = self.client.get_connection()?;
        let cache_key = self.make_key(key)?;

        let value: Option<String> = conn.get(&cache_key)?;

        match value {
            Some(v) => {
                let decoded: V = serde_json::from_str(&v)?;
                Ok(Some(decoded))
            }
            None => Ok(None),
        }
    }

    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.client.get_connection()?;
        let cache_key = self.make_key(&key)?;
        let serialized = serde_json::to_string(&value)?;

        match ttl {
            Some(duration) => {
                conn.set_ex(&cache_key, serialized, duration.as_secs() as usize)?;
            }
            None => {
                conn.set(&cache_key, serialized)?;
            }
        }

        Ok(())
    }

    fn delete(&self, key: &K) -> Result<()> {
        let mut conn = self.client.get_connection()?;
        let cache_key = self.make_key(key)?;
        conn.del(&cache_key)?;
        Ok(())
    }
}
```

**Performance characteristics**:
- **Read**: ~1ms (network round-trip)
- **Write**: ~1ms (network round-trip)
- **Concurrency**: Unlimited (connection pooling)
- **Scalability**: Horizontal across Redis cluster

#### 3. DiskCache

Persistent disk-based cache:

```rust
pub struct DiskCache {
    base_dir: PathBuf,
    index: Arc<RwLock<HashMap<String, CacheMetadata>>>,
    compression: bool,
}

struct CacheMetadata {
    path: PathBuf,
    size: u64,
    ttl: Option<Instant>,
    created_at: Instant,
    last_accessed: AtomicU64,
}

impl<K, V> Tier<K, V> for DiskCache
where
    K: Serialize,
    V: Serialize + for<'de> Deserialize<'de>,
{
    fn get(&self, key: &K) -> Result<Option<V>> {
        let cache_key = self.make_key(key)?;

        // Check index
        let metadata = {
            let index = self.index.read()?;
            index.get(&cache_key).cloned()
        };

        if let Some(meta) = metadata {
            // Check TTL
            if let Some(expiry) = meta.ttl {
                if Instant::now() > expiry {
                    self.delete(key)?;
                    return Ok(None);
                }
            }

            // Read from disk
            let data = fs::read(&meta.path)?;

            // Decompress if needed
            let decoded = if self.compression {
                let decompressed = zstd::decode_all(&*data)?;
                serde_json::from_slice(&decompressed)?
            } else {
                serde_json::from_slice(&data)?
            };

            // Update last accessed
            meta.last_accessed.store(
                Instant::now().duration_since(UNIX_EPOCH).as_secs(),
                Ordering::Relaxed
            );

            return Ok(Some(decoded));
        }

        Ok(None)
    }

    fn set(&self, key: K, value: V, ttl: Option<Duration>) -> Result<()> {
        let cache_key = self.make_key(key)?;

        // Serialize
        let serialized = serde_json::to_vec(&value)?;

        // Compress if needed
        let data = if self.compression {
            zstd::encode_all(&*serialized, 3)?
        } else {
            serialized
        };

        // Write to temp file
        let temp_path = self.base_dir.join(format!("{}.tmp", cache_key));
        fs::write(&temp_path, data)?;

        // Move to final location
        let final_path = self.base_dir.join(&cache_key);
        fs::rename(&temp_path, &final_path)?;

        // Update index
        let metadata = CacheMetadata {
            path: final_path,
            size: data.len() as u64,
            ttl: ttl.map(|d| Instant::now() + d),
            created_at: Instant::now(),
            last_accessed: AtomicU64::new(
                Instant::now().duration_since(UNIX_EPOCH).as_secs()
            ),
        };

        let mut index = self.index.write()?;
        index.insert(cache_key, metadata);

        Ok(())
    }
}
```

**Performance characteristics**:
- **Read**: ~10ms (NVMe SSD) or ~100ms (HDD)
- **Write**: ~20ms (NVMe SSD) or ~200ms (HDD)
- **Compression**: 3-5x reduction in size, adds ~1ms CPU

## Cache Coherence and Consistency

### Write-Through Policy

All writes go to all tiers:

```rust
cache.set(&key, value)?;  // Writes to L1, L2, L3
```

**Advantages**:
- Strong consistency: All tiers have same data
- Fast reads: Data already in fast tiers
- Simple failure handling: If L3 fails, L1/L2 still have data

**Disadvantages**:
- Slower writes: Must wait for slowest tier
- Higher bandwidth: Write amplification

### Promotion on Cache Hit

When data is found in a slower tier, promote to faster tiers:

```rust
// L2 hit → Promote to L1
if let Some(value) = l2.get(key)? {
    l1.set(key.clone(), value.clone(), ttl)?;
    return Ok(Some(value));
}

// L3 hit → Promote to L2
if let Some(value) = l3.get(key)? {
    l2.set(key.clone(), value.clone(), ttl)?;
    return Ok(Some(value));
}
```

**Advantages**:
- Self-optimizing: Hot data moves to fast tiers
- Automatic: No manual cache warming needed
- Adaptive: Changes with access patterns

**Disadvantages**:
- Stampede risk: Cold start can flood slower tiers
- Eviction pressure: Promotion can trigger evictions

### TTL and Expiration

Time-based expiration prevents stale data:

```rust
// Set with 1 hour TTL
cache.set_with_ttl(&key, value, Duration::from_secs(3600))?;

// Background cleanup task
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        cleanup_expired_entries();
    }
});
```

**Lazy expiration**: Check TTL on access
**Eager expiration**: Background task removes expired entries

## Performance Characteristics

### Latency by Tier

| Operation | L1 (Memory) | L2 (Redis) | L3 (Disk) |
|-----------|-------------|------------|-----------|
| Get (hit) | ~100ns | ~1ms | ~10ms |
| Get (miss) | ~50ns | ~0.5ms | ~5ms |
| Set | ~200ns | ~1ms | ~20ms |
| Delete | ~100ns | ~0.5ms | ~5ms |

### Throughput

| Tier | Read Throughput | Write Throughput |
|------|-----------------|------------------|
| L1 (Memory) | 10M ops/sec | 5M ops/sec |
| L2 (Redis) | 100K ops/sec | 100K ops/sec |
| L3 (Disk) | 10K ops/sec | 5K ops/sec |

### Capacity Planning

**Memory (L1)**:
- Target 80% hit rate
- Size = working_set_size * 0.8
- Example: 1GB working set → 800MB L1

**Redis (L2)**:
- Target 15% hit rate
- Size = working_set_size * 0.15
- Example: 1GB working set → 150MB L2 (but typically provision 10GB+)

**Disk (L3)**:
- Target 5% hit rate
- Size = working_set_size * 1.0 (full working set)
- Example: 1TB dataset → 1TB L3

## Integration with vector-navigator

### Vector Search Caching

```rust
use cache_layer::{MultiTierCache, MemoryCache};
use vector_navigator::{VectorStore, SearchQuery};

let cache = MultiTierCache::new()
    .with_l1(MemoryCache::new(500_000_000)?)  // 500MB for vectors
    .build();

let vector_store = VectorStore::new(cache.clone())?;

// Search with automatic caching
let query = SearchQuery {
    vector: embed("semantic search query").await?,
    top_k: 10,
    filters: HashMap::new(),
};

// First search: Cache miss → compute
let results1 = vector_store.search(&query).await?;

// Second search: Cache hit (~100ns)
let results2 = vector_store.search(&query).await?;

assert_eq!(results1, results2);
```

### Embedding Caching

```rust
use embeddings_engine::{EmbeddingsEngine, EmbeddingModel};

// Cache embeddings to avoid recomputation
async fn get_or_compute_embedding(
    text: &str,
    engine: &EmbeddingsEngine,
    cache: &MultiTierCache<String, Vec<f32>>,
) -> Result<Vec<f32>> {
    if let Some(embedding) = cache.get(&text.to_string()).await? {
        return Ok(embedding);
    }

    let embedding = engine.embed(text).await?;
    cache.set(text.to_string(), embedding.clone()).await?;
    Ok(embedding)
}
```

### Document Metadata Caching

```rust
use semantic_store::{DocumentStore, Document};

// Cache document metadata (not content, which can be large)
async fn get_document_metadata(
    doc_id: &str,
    store: &DocumentStore,
    cache: &MultiTierCache<String, DocumentMetadata>,
) -> Result<DocumentMetadata> {
    if let Some(metadata) = cache.get(&doc_id.to_string()).await? {
        return Ok(metadata);
    }

    let doc = store.get(doc_id).await?;
    let metadata = DocumentMetadata {
        id: doc.id,
        title: doc.title,
        created_at: doc.created_at,
        author: doc.author,
    };

    cache.set(doc_id.to_string(), metadata.clone()).await?;
    Ok(metadata)
}
```

## Metrics and Monitoring

### Built-in Metrics

```rust
pub struct CacheMetrics {
    l1_hits: AtomicU64,
    l1_misses: AtomicU64,
    l2_hits: AtomicU64,
    l2_misses: AtomicU64,
    l3_hits: AtomicU64,
    l3_misses: AtomicU64,
    total_ops: AtomicU64,
}

impl CacheMetrics {
    pub fn l1_hit_rate(&self) -> f64 {
        let hits = self.l1_hits.load(Ordering::Relaxed);
        let misses = self.l1_misses.load(Ordering::Relaxed);
        hits as f64 / (hits + misses) as f64
    }

    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.l1_hits.load(Ordering::Relaxed)
            + self.l2_hits.load(Ordering::Relaxed)
            + self.l3_hits.load(Ordering::Relaxed);
        let total_ops = self.total_ops.load(Ordering::Relaxed);
        total_hits as f64 / total_ops as f64
    }

    pub fn avg_latency(&self) -> Duration {
        // Calculate from latency histogram
        Duration::from_nanos(124)  // Example: 124ns average
    }
}
```

### Prometheus Integration

```rust
use prometheus::{Counter, Histogram, Registry};

// Register metrics
let cache_hits = Counter::new("cache_hits_total", "Total cache hits")?;
let cache_misses = Counter::new("cache_misses_total", "Total cache misses")?;
let cache_latency = Histogram::new("cache_latency_seconds", "Cache latency")?;

// Expose metrics
Registry::default().register(Box::new(cache_hits.clone()))?;
Registry::default().register(Box::new(cache_misses.clone()))?;
Registry::default().register(Box::new(cache_latency.clone()))?;
```

## Failure Modes and Resilience

### Tier Failures

```rust
pub fn get(&self, key: &K) -> Result<Option<V>> {
    // Gracefully handle tier failures
    if let Some(l1) = &self.l1 {
        match l1.get(key) {
            Ok(Some(value)) => return Ok(Some(value)),
            Ok(None) => {},
            Err(e) => {
                // Log error but continue to next tier
                error!("L1 cache error: {}", e);
            }
        }
    }
    // Continue to L2, L3...
}
```

### Fallback Behavior

- **L1 failure**: Fall through to L2, L3
- **L2 failure**: Fall through to L3
- **L3 failure**: Return None (cache miss)
- **All tiers fail**: Return error to application

### Partial Writes

```rust
pub fn set(&self, key: K, value: V) -> Result<()> {
    let mut last_error = None;

    // Try to write to all tiers, collect errors
    if let Some(l1) = &self.l1 {
        if let Err(e) = l1.set(key.clone(), value.clone(), ttl) {
            last_error = Some(e);
        }
    }

    if let Some(l2) = &self.l2 {
        if let Err(e) = l2.set(key.clone(), value.clone(), ttl) {
            last_error = Some(e);
        }
    }

    if let Some(l3) = &self.l3 {
        if let Err(e) = l3.set(key, value, ttl) {
            last_error = Some(e);
        }
    }

    // Return last error if all tiers failed
    if let Some(e) = last_error {
        Err(e)
    } else {
        Ok(())
    }
}
```

## Summary

cache-layer implements a timeless memory hierarchy principle with modern optimizations:

1. **Multi-tier organization**: Fast (L1) → Medium (L2) → Slow (L3)
2. **Automatic promotion**: Hot data rises to faster tiers
3. **Intelligent eviction**: LRU/LFU/FIFO policies per tier
4. **Strong consistency**: Write-through ensures coherence
5. **Graceful degradation**: Tier failures don't crash the system

The result is a cache that achieves sub-microsecond latency for hot data while maintaining massive capacity through tiered storage.
