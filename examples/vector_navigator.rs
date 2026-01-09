// Vector search caching example with cache-layer
//
// Demonstrates how cache-layer integrates with vector-navigator
// to cache similarity search results.

use cache_layer::{MultiTierCache, MemoryCache, EvictionPolicy};
use std::collections::HashMap;
use std::time::Duration;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct SearchResult {
    document_id: String,
    score: f32,
    metadata: HashMap<String, String>,
}

struct VectorStore {
    cache: MultiTierCache<String, Vec<SearchResult>>,
    vectors: HashMap<String, Vec<f32>>,
}

impl VectorStore {
    fn new(cache: MultiTierCache<String, Vec<SearchResult>>) -> Self {
        Self {
            cache,
            vectors: HashMap::new(),
        }
    }

    async fn search(
        &self,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
        // Create cache key from query parameters
        let cache_key = format!(
            "search:{}:{}",
            query.iter().map(|f| f.to_string()).collect::<String>(),
            top_k
        );

        // Try cache first
        if let Some(cached_results) = self.cache.get(&cache_key).await? {
            println!("Cache hit! Returning {} cached results", cached_results.len());
            return Ok(cached_results);
        }

        println!("Cache miss - computing similarity search...");

        // Simulate expensive vector search
        let results = self.compute_similarity(query, top_k);

        // Store in cache
        self.cache
            .set_with_ttl(&cache_key, results.clone(), Duration::from_secs(3600))
            .await?;

        Ok(results)
    }

    fn compute_similarity(&self, query: &[f32], top_k: usize) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // Simulate similarity computation
        for (doc_id, vector) in &self.vectors {
            let score = cosine_similarity(query, vector);
            results.push(SearchResult {
                document_id: doc_id.clone(),
                score,
                metadata: HashMap::new(),
            });
        }

        // Sort by score (descending)
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Return top-k
        results.truncate(top_k);
        results
    }

    fn add_vector(&mut self, doc_id: String, vector: Vec<f32>) {
        self.vectors.insert(doc_id, vector);
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Vector Search Caching Example ===\n");

    // Create cache optimized for vector search
    let cache = MultiTierCache::new()
        .with_l1(
            MemoryCache::builder()
                .capacity(500_000_000) // 500MB for vectors
                .eviction_policy(EvictionPolicy::LRU)
                .build(),
        )
        .with_ttl(Duration::from_secs(3600)) // 1 hour
        .build();

    println!("✓ Created multi-tier cache for vector search\n");

    // Create vector store
    let mut store = VectorStore::new(cache);

    // Add some sample vectors
    println!("Adding sample vectors...");
    store.add_vector("doc1".to_string(), vec![0.1, 0.2, 0.3, 0.4]);
    store.add_vector("doc2".to_string(), vec![0.5, 0.6, 0.7, 0.8]);
    store.add_vector("doc3".to_string(), vec![0.2, 0.3, 0.4, 0.5]);
    store.add_vector("doc4".to_string(), vec![0.9, 0.1, 0.2, 0.3]);
    store.add_vector("doc5".to_string(), vec![0.4, 0.5, 0.6, 0.7]);
    println!("Added 5 vectors\n");

    // Perform first search (cache miss)
    println!("--- First Search (Cache Miss) ---");
    let query = vec![0.15, 0.25, 0.35, 0.45];
    let results = store.search(&query, 3).await?;

    println!("Top 3 results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (score: {:.3})",
            i + 1,
            result.document_id,
            result.score
        );
    }
    println!();

    // Perform same search again (cache hit)
    println!("--- Second Search (Cache Hit) ---");
    let results = store.search(&query, 3).await?;

    println!("Top 3 results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (score: {:.3})",
            i + 1,
            result.document_id,
            result.score
        );
    }
    println!();

    // Show cache metrics
    println!("--- Cache Metrics ---");
    let metrics = store.cache.metrics();
    println!("Total operations: {}", metrics.total_ops());
    println!("L1 hit rate: {:.2}%", metrics.l1_hit_rate() * 100.0);
    println!(
        "L2 hit rate: {:.2}%",
        metrics.l2_hit_rate() * 100.0
    );
    println!("Overall hit rate: {:.2}%", metrics.overall_hit_rate() * 100.0);

    println!("\n=== Example Complete ===");
    Ok(())
}
