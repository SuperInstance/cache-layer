// Basic usage example for cache-layer
//
// Run with: cargo run --example basic_usage

use cache_layer::{MultiTierCache, MemoryCache};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== cache-layer Basic Usage Example ===\n");

    // Create a simple memory cache
    let cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(1_000_000)?)  // 1MB
        .build();

    println!("✓ Created memory cache (1MB capacity)\n");

    // Example 1: Basic set and get
    println!("--- Example 1: Basic Set/Get ---");
    cache.set("greeting", "Hello, World!").await?;
    println!("Set: greeting = 'Hello, World!'");

    if let Some(value) = cache.get(&"greeting".to_string()).await? {
        println!("Get: greeting = '{}'", value);
    }
    println!();

    // Example 2: Complex types
    println!("--- Example 2: Complex Types ---");
    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct User {
        id: u64,
        name: String,
        email: String,
    }

    let user = User {
        id: 123,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    cache.set("user:123", user.clone()).await?;
    println!("Set: user:123 = {:?}", user);

    if let Some(cached_user) = cache.get::<User>(&"user:123".to_string()).await? {
        println!("Get: user:123 = {:?}", cached_user);
    }
    println!();

    // Example 3: TTL (Time-To-Live)
    println!("--- Example 3: TTL Expiration ---");
    cache
        .set_with_ttl("temporary", "I'll expire soon", Duration::from_secs(2))
        .await?;
    println!("Set: temporary = 'I'll expire soon' (2 second TTL)");

    if let Some(value) = cache.get(&"temporary".to_string()).await? {
        println!("Get (immediate): temporary = '{}'", value);
    }

    println!("Waiting 2.5 seconds...");
    sleep(Duration::from_secs(2)).await;

    if let Some(value) = cache.get(&"temporary".to_string()).await? {
        println!("Get (after TTL): temporary = '{}'", value);
    } else {
        println!("Get (after TTL): temporary = None (expired)");
    }
    println!();

    // Example 4: Delete
    println!("--- Example 4: Delete ---");
    cache.set("to_delete", "Goodbye").await?;
    println!("Set: to_delete = 'Goodbye'");

    if let Some(value) = cache.get(&"to_delete".to_string()).await? {
        println!("Get (before delete): to_delete = '{}'", value);
    }

    cache.delete(&"to_delete".to_string()).await?;
    println!("Delete: to_delete");

    if let Some(value) = cache.get(&"to_delete".to_string()).await? {
        println!("Get (after delete): to_delete = '{}'", value);
    } else {
        println!("Get (after delete): to_delete = None (deleted)");
    }
    println!();

    // Example 5: Check existence
    println!("--- Example 5: Check Existence ---");
    cache.set("exists", "I exist").await?;

    let exists = cache.exists(&"exists".to_string()).await?;
    println!("Exists('exists'): {}", exists);

    let not_exists = cache.exists(&"not_exists".to_string()).await?;
    println!("Exists('not_exists'): {}", not_exists);
    println!();

    // Example 6: Cache metrics
    println!("--- Example 6: Cache Metrics ---");
    let metrics = cache.metrics();
    println!("Total operations: {}", metrics.total_ops());
    println!("L1 hit rate: {:.2}%", metrics.l1_hit_rate() * 100.0);
    println!("Average latency: {:?}", metrics.avg_latency());

    println!("\n=== Example Complete ===");
    Ok(())
}
