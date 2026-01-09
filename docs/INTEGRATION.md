# Integration Guide

Examples and patterns for integrating cache-layer with ecosystem tools and general applications.

## Table of Contents

1. [vector-navigator Integration](#vector-navigator-integration)
2. [embeddings-engine Integration](#embeddings-engine-integration)
3. [semantic-store Integration](#semantic-store-integration)
4. [General-Purpose Caching](#general-purpose-caching)
5. [Complete Application Example](#complete-application-example)

## vector-navigator Integration

### Scenario: Vector Similarity Search Caching

Cache search results to avoid recomputing similarity scores for repeated queries.

### Basic Integration

```rust
use cache_layer::{MultiTierCache, MemoryCache, RedisCache};
use vector_navigator::{VectorStore, SearchQuery, SearchResult};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize multi-tier cache for vector search results
    let cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(500_000_000)?)  // 500MB for vectors
        .with_l2(RedisCache::new("redis://localhost:6379")?)
        .with_ttl(Duration::from_secs(3600))  // 1 hour TTL
        .build();

    // Create vector store with cache
    let vector_store = VectorStore::builder()
        .cache(cache.clone())
        .dimension(768)  // BERT base dimension
        .build()
        .await?;

    // Load vectors from storage
    vector_store.load_from_path("./vectors/").await?;

    Ok(())
}
```

### Cached Search Function

```rust
use std::collections::HashMap;

async fn search_with_cache(
    vector_store: &VectorStore<MultiTierCache<String, Vec<SearchResult>>>,
    query: Vec<f32>,
    top_k: usize,
    filters: HashMap<String, String>,
) -> Result<Vec<SearchResult>> {
    // Create cache key from query parameters
    let cache_key = format!(
        "search:{}:{}:{}",
        serde_json::to_string(&query)?,
        top_k,
        serde_json::to_string(&filters)?
    );

    // Try cache first
    if let Some(cached_results) = vector_store.cache().get(&cache_key).await? {
        println!("Cache hit for search query");
        return Ok(cached_results);
    }

    // Cache miss - perform search
    println!("Cache miss - computing search");
    let results = vector_store
        .search(&SearchQuery {
            vector: query,
            top_k,
            filters,
        })
        .await?;

    // Store in cache
    vector_store.cache().set(&cache_key, results.clone()).await?;

    Ok(results)
}
```

### Batch Search with Cache

```rust
async fn batch_search(
    vector_store: &VectorStore<MultiTierCache<String, Vec<SearchResult>>>,
    queries: Vec<Vec<f32>>,
    top_k: usize,
) -> Result<Vec<Vec<SearchResult>>> {
    let mut results = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for query in queries {
        let cache_key = format!("search:{:?}", query);

        match vector_store.cache().get(&cache_key).await? {
            Some(cached) => {
                results.push(cached);
                cache_hits += 1;
            }
            None => {
                let search_results = vector_store
                    .search(&SearchQuery {
                        vector: query,
                        top_k,
                        filters: HashMap::new(),
                    })
                    .await?;

                vector_store.cache().set(&cache_key, search_results.clone()).await?;
                results.push(search_results);
                cache_misses += 1;
            }
        }
    }

    println!(
        "Batch search: {} hits, {} misses, {:.1}% hit rate",
        cache_hits,
        cache_misses,
        (cache_hits as f64 / (cache_hits + cache_misses) as f64) * 100.0
    );

    Ok(results)
}
```

### Cache Warming for Common Queries

```rust
async fn warm_vector_cache(
    vector_store: &VectorStore<MultiTierCache<String, Vec<SearchResult>>>,
    common_queries: Vec<Vec<f32>>,
) -> Result<()> {
    println!("Warming vector cache with {} queries...", common_queries.len());

    for (i, query) in common_queries.iter().enumerate() {
        let cache_key = format!("search:{:?}", query);

        // Check if already cached
        if vector_store.cache().exists(&cache_key).await? {
            continue;
        }

        // Compute and cache
        let results = vector_store
            .search(&SearchQuery {
                vector: query.clone(),
                top_k: 100,  // Cache more results than needed
                filters: HashMap::new(),
            })
            .await?;

        vector_store.cache().set(&cache_key, results).await?;

        if (i + 1) % 100 == 0 {
            println!("Warmed {}/{} queries", i + 1, common_queries.len());
        }
    }

    println!("Vector cache warming complete");
    Ok(())
}
```

## embeddings-engine Integration

### Scenario: Embedding Computation Caching

Cache computed embeddings to avoid recomputing for the same text.

### Basic Integration

```rust
use cache_layer::{MultiTierCache, MemoryCache, DiskCache};
use embeddings_engine::{EmbeddingsEngine, EmbeddingModel};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize cache for embeddings
    let cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(1_000_000_000)?)  // 1GB for embeddings
        .with_l3(DiskCache::new("/var/cache/embeddings")?)  // Persistent storage
        .with_ttl(Duration::from_secs(86400 * 7))  // 7 days
        .build();

    // Create embeddings engine
    let engine = EmbeddingsEngine::builder()
        .model(EmbeddingModel::BERTBase)
        .cache(cache.clone())
        .build()
        .await?;

    Ok(())
}
```

### Embedding Computation with Cache-Aside Pattern

```rust
async fn get_embedding(
    text: &str,
    engine: &EmbeddingsEngine,
    cache: &MultiTierCache<String, Vec<f32>>,
) -> Result<Vec<f32>> {
    // Try cache first
    let cache_key = format!("embed:{}", text);

    if let Some(embedding) = cache.get(&cache_key).await? {
        println!("Cache hit for embedding: '{}'", text);
        return Ok(embedding);
    }

    // Cache miss - compute embedding
    println!("Cache miss - computing embedding for '{}'", text);
    let embedding = engine.embed(text).await?;

    // Store in cache
    cache.set(&cache_key, embedding.clone()).await?;

    Ok(embedding)
}
```

### Batch Embedding Computation

```rust
async fn batch_get_embeddings(
    texts: Vec<&str>,
    engine: &EmbeddingsEngine,
    cache: &MultiTierCache<String, Vec<f32>>,
) -> Result<Vec<Vec<f32>>> {
    let mut embeddings = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for text in texts {
        match get_embedding(text, engine, cache).await {
            Ok(embedding) => {
                embeddings.push(embedding);
                // Check if it was a cache hit (no computation performed)
                let was_cached = cache.exists(&format!("embed:{}", text)).await?;
                if was_cached {
                    cache_hits += 1;
                } else {
                    cache_misses += 1;
                }
            }
            Err(e) => {
                eprintln!("Error getting embedding for '{}': {}", text, e);
                // Return zero vector on error
                embeddings.push(vec![0.0; 768]);
            }
        }
    }

    println!(
        "Batch embeddings: {} hits, {} misses, {:.1}% hit rate",
        cache_hits,
        cache_misses,
        (cache_hits as f64 / (cache_hits + cache_misses) as f64) * 100.0
    );

    Ok(embeddings)
}
```

### Semantic Search with Cached Embeddings

```rust
async fn semantic_search(
    query: &str,
    documents: Vec<String>,
    engine: &EmbeddingsEngine,
    cache: &MultiTierCache<String, Vec<f32>>,
) -> Result<Vec<(String, f32)>> {
    // Get query embedding (cached)
    let query_embedding = get_embedding(query, engine, cache).await?;

    // Get document embeddings (cached)
    let mut doc_embeddings = Vec::new();
    for doc in &documents {
        let embedding = get_embedding(doc, engine, cache).await?;
        doc_embeddings.push((doc.clone(), embedding));
    }

    // Compute similarities
    let mut similarities = Vec::new();
    for (doc, doc_embedding) in doc_embeddings {
        let similarity = cosine_similarity(&query_embedding, &doc_embedding);
        similarities.push((doc, similarity));
    }

    // Sort by similarity (descending)
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    Ok(similarities)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}
```

## semantic-store Integration

### Scenario: Document Metadata and Content Caching

Cache frequently accessed document metadata and content.

### Basic Integration

```rust
use cache_layer::{MultiTierCache, MemoryCache, RedisCache, DiskCache};
use semantic_store::{SemanticStore, Document};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Cache for metadata (small, fast access)
    let metadata_cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(100_000_000)?)  // 100MB
        .with_l2(RedisCache::new("redis://localhost:6379")?)
        .with_ttl(Duration::from_secs(3600))
        .build();

    // Cache for full document content (large, persistent)
    let content_cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(500_000_000)?)  // 500MB
        .with_l3(DiskCache::new("/var/cache/documents")?)
        .with_ttl(Duration::from_secs(86400))  // 24 hours
        .build();

    // Create semantic store with caches
    let store = SemanticStore::builder()
        .metadata_cache(metadata_cache)
        .content_cache(content_cache)
        .build()
        .await?;

    Ok(())
}
```

### Document Retrieval with Caching

```rust
async fn get_document_with_cache(
    doc_id: &str,
    store: &SemanticStore,
) -> Result<Document> {
    // Try metadata cache first
    let metadata_key = format!("metadata:{}", doc_id);

    if let Some(metadata) = store.metadata_cache().get(&metadata_key).await? {
        // Try content cache
        let content_key = format!("content:{}", doc_id);

        if let Some(content) = store.content_cache().get(&content_key).await? {
            println!("Cache hit for document {}", doc_id);
            return Ok(Document { metadata, content });
        }
    }

    // Cache miss - fetch from store
    println!("Cache miss - fetching document {}", doc_id);
    let document = store.get_document(doc_id).await?;

    // Store in caches
    store.metadata_cache().set(&metadata_key, document.metadata.clone()).await?;
    store.content_cache().set(&content_key, document.content.clone()).await?;

    Ok(document)
}
```

### Batch Document Retrieval

```rust
async fn batch_get_documents(
    doc_ids: Vec<String>,
    store: &SemanticStore,
) -> Result<Vec<Document>> {
    let mut documents = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for doc_id in doc_ids {
        match get_document_with_cache(&doc_id, store).await {
            Ok(doc) => {
                documents.push(doc);

                // Check if was cached
                let was_cached = store
                    .metadata_cache()
                    .exists(&format!("metadata:{}", doc_id))
                    .await?;

                if was_cached {
                    cache_hits += 1;
                } else {
                    cache_misses += 1;
                }
            }
            Err(e) => {
                eprintln!("Error fetching document {}: {}", doc_id, e);
            }
        }
    }

    println!(
        "Batch retrieval: {} hits, {} misses, {:.1}% hit rate",
        cache_hits,
        cache_misses,
        (cache_hits as f64 / (cache_hits + cache_misses) as f64) * 100.0
    );

    Ok(documents)
}
```

### Document Index with Cached Metadata

```rust
async fn search_documents(
    query: &str,
    store: &SemanticStore,
) -> Result<Vec<Document>> {
    // Search index (fast)
    let doc_ids = store.search_index(query).await?;

    println!("Found {} documents matching query", doc_ids.len());

    // Retrieve documents with caching
    let mut documents = Vec::new();
    for doc_id in doc_ids {
        if let Ok(doc) = get_document_with_cache(&doc_id, store).await {
            documents.push(doc);
        }
    }

    Ok(documents)
}
```

## General-Purpose Caching

### Database Query Caching

```rust
use cache_layer::{MultiTierCache, MemoryCache};
use sqlx::Postgres;

async fn get_user(
    user_id: u64,
    pool: &PgPool,
    cache: &MultiTierCache<String, User>,
) -> Result<User> {
    let cache_key = format!("user:{}", user_id);

    // Try cache
    if let Some(user) = cache.get(&cache_key).await? {
        return Ok(user);
    }

    // Cache miss - query database
    let user = sqlx::query_as::<_, User>(
        "SELECT id, name, email FROM users WHERE id = $1"
    )
    .bind(user_id as i32)
    .fetch_one(pool)
    .await?;

    // Cache for 5 minutes
    cache.set_with_ttl(
        &cache_key,
        user.clone(),
        Duration::from_secs(300)
    ).await?;

    Ok(user)
}
```

### API Response Caching

```rust
use reqwest::Client;
use cache_layer::MultiTierCache;

async fn fetch_url(
    url: &str,
    client: &Client,
    cache: &MultiTierCache<String, String>,
) -> Result<String> {
    // Try cache
    if let Some(response) = cache.get(&url.to_string()).await? {
        println!("Cache hit for URL: {}", url);
        return Ok(response);
    }

    // Cache miss - fetch from network
    println!("Cache miss - fetching URL: {}", url);
    let response = client.get(url).send().await?;
    let text = response.text().await?;

    // Cache for 1 hour
    cache.set_with_ttl(
        &url.to_string(),
        text.clone(),
        Duration::from_secs(3600)
    ).await?;

    Ok(text)
}
```

### Expensive Computation Caching

```rust
use cache_layer::MultiTierCache;

async fn compute_fibonacci(
    n: u64,
    cache: &MultiTierCache<u64, u64>,
) -> u64 {
    if n <= 1 {
        return n;
    }

    let cache_key = n;

    // Try cache
    if let Some(result) = cache.get(&cache_key).await.unwrap() {
        return result;
    }

    // Compute recursively
    let result = compute_fibonacci(n - 1, cache).await
        + compute_fibonacci(n - 2, cache).await;

    // Cache result
    cache.set(&cache_key, result).await.unwrap();

    result
}
```

### Session Storage

```rust
use cache_layer::{MultiTierCache, MemoryCache, RedisCache};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    user_id: u64,
    username: String,
    created_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
}

async fn get_session(
    session_id: &str,
    cache: &MultiTierCache<String, Session>,
) -> Result<Option<Session>> {
    let key = format!("session:{}", session_id);

    // Try cache (with automatic expiration)
    let session = cache.get(&key).await?;

    // Update last activity
    if let Some(mut s) = session {
        s.last_activity = Utc::now();
        cache.set(&key, s.clone()).await?;
        return Ok(Some(s));
    }

    Ok(None)
}

async fn create_session(
    user_id: u64,
    username: String,
    cache: &MultiTierCache<String, Session>,
) -> Result<String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let session = Session {
        user_id,
        username,
        created_at: Utc::now(),
        last_activity: Utc::now(),
    };

    let key = format!("session:{}", session_id);

    // Cache for 24 hours
    cache.set_with_ttl(
        &key,
        session,
        Duration::from_secs(86400)
    ).await?;

    Ok(session_id)
}
```

## Complete Application Example

### Web Service with Multi-Layer Caching

```rust
use actix_web::{web, App, HttpServer};
use cache_layer::{MultiTierCache, MemoryCache, RedisCache, DiskCache};
use std::time::Duration;

struct AppState {
    // L1: Application state (in-memory)
    config_cache: MultiTierCache<String, Config>,

    // L2: User data (Redis)
    user_cache: MultiTierCache<String, User>,

    // L3: Document content (disk)
    document_cache: MultiTierCache<String, Document>,
}

#[actix_web::main]
async fn main() -> Result<()> {
    // Initialize caches
    let config_cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(10_000_000)?)  // 10MB
        .build();

    let user_cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(100_000_000)?)  // 100MB
        .with_l2(RedisCache::new("redis://localhost:6379")?)
        .with_ttl(Duration::from_secs(300))  // 5 minutes
        .build();

    let document_cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(500_000_000)?)  // 500MB
        .with_l3(DiskCache::new("/var/cache/documents")?)
        .with_ttl(Duration::from_secs(3600))  // 1 hour
        .build();

    let state = web::Data::new(AppState {
        config_cache,
        user_cache,
        document_cache,
    });

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/config", web::get().to(get_config))
            .route("/user/{id}", web::get().to(get_user))
            .route("/document/{id}", web::get().to(get_document))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;

    Ok(())
}

async fn get_config(
    state: web::Data<AppState>,
) -> Result<web::Json<Config>> {
    let key = "config:default".to_string();

    let config = if let Some(cached) = state.config_cache.get(&key).await? {
        cached
    } else {
        let config = load_config_from_db().await?;
        state.config_cache.set(&key, config.clone()).await?;
        config
    };

    Ok(web::Json(config))
}

async fn get_user(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<web::Json<User>> {
    let user_id = path.into_inner();
    let key = format!("user:{}", user_id);

    let user = if let Some(cached) = state.user_cache.get(&key).await? {
        cached
    } else {
        let user = load_user_from_db(&user_id).await?;
        state.user_cache.set(&key, user.clone()).await?;
        user
    };

    Ok(web::Json(user))
}

async fn get_document(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<web::Json<Document>> {
    let doc_id = path.into_inner();
    let key = format!("document:{}", doc_id);

    let document = if let Some(cached) = state.document_cache.get(&key).await? {
        cached
    } else {
        let document = load_document_from_storage(&doc_id).await?;
        state.document_cache.set(&key, document.clone()).await?;
        document
    };

    Ok(web::Json(document))
}
```

## Monitoring and Metrics

### Track Cache Performance

```rust
use cache_layer::CacheMetrics;

async fn print_cache_metrics(cache: &MultiTierCache<String, String>) {
    let metrics = cache.metrics();

    println!("=== Cache Metrics ===");
    println!("L1 hit rate: {:.2}%", metrics.l1_hit_rate() * 100.0);
    println!("L2 hit rate: {:.2}%", metrics.l2_hit_rate() * 100.0);
    println!("L3 hit rate: {:.2}%", metrics.l3_hit_rate() * 100.0);
    println!("Overall hit rate: {:.2}%", metrics.overall_hit_rate() * 100.0);
    println!("Total operations: {}", metrics.total_ops());
    println!("Average latency: {:?}", metrics.avg_latency());
}

// Run periodically
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        print_cache_metrics(&cache).await;
    }
});
```

## Best Practices

1. **Use appropriate TTL**: Balance freshness with cache hit rate
2. **Monitor hit rates**: Adjust cache sizes based on metrics
3. **Warm critical data**: Pre-populate cache on startup
4. **Handle cache failures**: Degrade gracefully if cache fails
5. **Use compression**: Enable compression for large values
6. **Batch operations**: More efficient than individual ops
7. **Cache keys**: Use consistent, predictable key formats
8. **Eviction policies**: Choose based on access patterns
9. **Multi-tier strategy**: Balance speed vs capacity
10. **Test under load**: Verify performance at production scale
