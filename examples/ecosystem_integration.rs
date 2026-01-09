// Complete ecosystem integration example
//
// Demonstrates cache-layer integrated with:
// - vector-navigator (simulated)
// - embeddings-engine (simulated)
// - semantic-store (simulated)

use cache_layer::{MultiTierCache, MemoryCache, RedisCache, EvictionPolicy};
use std::collections::HashMap;
use std::time::Duration;

// Simulated types from ecosystem tools
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Document {
    id: String,
    title: String,
    content: String,
    embedding: Vec<f32>,
    metadata: HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct SearchResult {
    document_id: String,
    score: f32,
    highlights: Vec<String>,
}

// Simulated vector-navigator integration
struct VectorNavigator {
    cache: MultiTierCache<String, Vec<SearchResult>>,
}

impl VectorNavigator {
    fn new(cache: MultiTierCache<String, Vec<SearchResult>>) -> Self {
        Self { cache }
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
        let cache_key = format!("search:{:?}:{}", query_embedding, top_k);

        if let Some(results) = self.cache.get(&cache_key).await? {
            println!("✓ Vector search cache hit");
            return Ok(results);
        }

        println!("✗ Vector search cache miss - computing");

        // Simulate search computation
        let results = vec![
            SearchResult {
                document_id: "doc1".to_string(),
                score: 0.95,
                highlights: vec!["highlight 1".to_string()],
            },
            SearchResult {
                document_id: "doc2".to_string(),
                score: 0.87,
                highlights: vec!["highlight 2".to_string()],
            },
        ];

        self.cache.set(&cache_key, results.clone()).await?;
        Ok(results)
    }
}

// Simulated embeddings-engine integration
struct EmbeddingsEngine {
    cache: MultiTierCache<String, Vec<f32>>,
}

impl EmbeddingsEngine {
    fn new(cache: MultiTierCache<String, Vec<f32>>) -> Self {
        Self { cache }
    }

    async fn embed(
        &self,
        text: &str,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let cache_key = format!("embed:{}", text);

        if let Some(embedding) = self.cache.get(&cache_key).await? {
            println!("✓ Embedding cache hit");
            return Ok(embedding);
        }

        println!("✗ Embedding cache miss - computing");

        // Simulate embedding computation
        let embedding = vec![0.1; 768]; // BERT base size

        self.cache.set(&cache_key, embedding.clone()).await?;
        Ok(embedding)
    }
}

// Simulated semantic-store integration
struct SemanticStore {
    metadata_cache: MultiTierCache<String, HashMap<String, String>>,
    content_cache: MultiTierCache<String, Document>,
}

impl SemanticStore {
    fn new(
        metadata_cache: MultiTierCache<String, HashMap<String, String>>,
        content_cache: MultiTierCache<String, Document>,
    ) -> Self {
        Self {
            metadata_cache,
            content_cache,
        }
    }

    async fn get_document(
        &self,
        doc_id: &str,
    ) -> Result<Document, Box<dyn std::error::Error>> {
        let metadata_key = format!("metadata:{}", doc_id);
        let content_key = format!("content:{}", doc_id);

        // Try to get from cache
        if let Some(metadata) = self.metadata_cache.get(&metadata_key).await? {
            if let Some(content) = self.content_cache.get(&content_key).await? {
                println!("✓ Document cache hit");
                return Ok(content);
            }
        }

        println!("✗ Document cache miss - fetching from storage");

        // Simulate document retrieval
        let document = Document {
            id: doc_id.to_string(),
            title: "Sample Document".to_string(),
            content: "This is the document content...".to_string(),
            embedding: vec![0.1; 768],
            metadata: HashMap::new(),
        };

        // Cache the document
        self.metadata_cache
            .set(&metadata_key, document.metadata.clone())
            .await?;
        self.content_cache.set(&content_key, document.clone()).await?;

        Ok(document)
    }
}

// Application that uses all cached tools
struct SearchApplication {
    vector_navigator: VectorNavigator,
    embeddings_engine: EmbeddingsEngine,
    semantic_store: SemanticStore,
}

impl SearchApplication {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create caches for each component

        // Vector search cache (small, fast access)
        let vector_cache = MultiTierCache::new()
            .with_l1(MemoryCache::builder().capacity(100_000_000).build()?)
            .with_ttl(Duration::from_secs(3600))
            .build();

        // Embedding cache (persistent, as embeddings are expensive to compute)
        let embedding_cache = MultiTierCache::new()
            .with_l1(MemoryCache::builder().capacity(1_000_000_000).build()?)
            .with_ttl(Duration::from_secs(86400 * 7)) // 7 days
            .build();

        // Document metadata cache (small)
        let metadata_cache = MultiTierCache::new()
            .with_l1(MemoryCache::builder().capacity(50_000_000).build()?)
            .with_ttl(Duration::from_secs(1800))
            .build();

        // Document content cache (larger)
        let content_cache = MultiTierCache::new()
            .with_l1(MemoryCache::builder().capacity(500_000_000).build()?)
            .with_ttl(Duration::from_secs(3600))
            .build();

        Ok(Self {
            vector_navigator: VectorNavigator::new(vector_cache),
            embeddings_engine: EmbeddingsEngine::new(embedding_cache),
            semantic_store: SemanticStore::new(metadata_cache, content_cache),
        })
    }

    async fn semantic_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
        println!("\n--- Semantic Search: '{}' ---", query);

        // Step 1: Get query embedding (cached)
        println!("1. Computing query embedding...");
        let query_embedding = self.embeddings_engine.embed(query).await?;

        // Step 2: Search vectors (cached)
        println!("2. Searching similar documents...");
        let results = self
            .vector_navigator
            .search(&query_embedding, top_k)
            .await?;

        // Step 3: Fetch full documents (cached)
        println!("3. Fetching document details...");
        for result in &results {
            let _doc = self.semantic_store.get_document(&result.document_id).await?;
        }

        Ok(results)
    }

    fn print_metrics(&self) {
        println!("\n=== Cache Metrics ===");

        let v_metrics = self.vector_navigator.cache.metrics();
        println!("Vector Navigator:");
        println!("  Operations: {}", v_metrics.total_ops());
        println!("  Hit rate: {:.2}%", v_metrics.overall_hit_rate() * 100.0);

        let e_metrics = self.embeddings_engine.cache.metrics();
        println!("Embeddings Engine:");
        println!("  Operations: {}", e_metrics.total_ops());
        println!("  Hit rate: {:.2}%", e_metrics.overall_hit_rate() * 100.0);

        let m_metrics = self.semantic_store.metadata_cache.metrics();
        println!("Document Metadata:");
        println!("  Operations: {}", m_metrics.total_ops());
        println!("  Hit rate: {:.2}%", m_metrics.overall_hit_rate() * 100.0);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cache-Layer Ecosystem Integration ===\n");

    // Create application with cached components
    let app = SearchApplication::new()?;
    println!("✓ Created search application with multi-tier caching\n");

    // Perform first search (cache misses)
    println!("### First Search (Cache Misses) ###");
    let _results1 = app.semantic_search("machine learning algorithms", 5).await?;

    // Perform same search again (cache hits)
    println!("\n### Second Search (Cache Hits) ###");
    let _results2 = app.semantic_search("machine learning algorithms", 5).await?;

    // Perform different search (partial cache hits)
    println!("\n### Third Search (Partial Cache Hits) ###");
    let _results3 = app.semantic_search("deep learning models", 5).await?;

    // Print final metrics
    app.print_metrics();

    println!("\n=== Example Complete ===");
    println!("\nKey Benefits Demonstrated:");
    println!("  • Embedding computation cached (expensive operation)");
    println!("  • Vector search cached (repeated queries)");
    println!("  • Document retrieval cached (reduced I/O)");
    println!("  • Multi-tier architecture balances speed and capacity");

    Ok(())
}
