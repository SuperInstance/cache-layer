// Monitoring and metrics example for cache-layer
//
// Demonstrates how to monitor cache performance and collect metrics.

use cache_layer::{MultiTierCache, MemoryCache};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cache Monitoring Example ===\n");

    // Create cache with metrics enabled
    let cache = MultiTierCache::new()
        .with_l1(MemoryCache::new(10_000_000)?) // 10MB
        .with_metrics(true)
        .build();

    println!("✓ Created cache with metrics enabled\n");

    // Example 1: Real-time metrics
    println!("--- Example 1: Real-Time Metrics ---");

    // Perform some operations
    for i in 0..100 {
        cache.set(&format!("key_{}", i), format!("value_{}", i)).await?;
    }

    for i in 0..100 {
        cache.get(&format!("key_{}", i)).await?;
    }

    // Get metrics snapshot
    let metrics = cache.metrics();

    println!("After 100 sets and 100 gets:");
    println!("  Total operations: {}", metrics.total_ops());
    println!("  L1 hit rate: {:.2}%", metrics.l1_hit_rate() * 100.0);
    println!("  L1 hits: {}", metrics.l1_hits());
    println!("  L1 misses: {}", metrics.l1_misses());
    println!();

    // Example 2: Performance profiling
    println!("--- Example 2: Performance Profiling ---");

    let start = Instant::now();
    let iterations = 10_000;

    for i in 0..iterations {
        cache.set(&format!("perf_key_{}", i), i).await?;
    }

    let set_duration = start.elapsed();
    let set_throughput = iterations as f64 / set_duration.as_secs_f64();

    println!("Set operations:");
    println!("  Total time: {:?}", set_duration);
    println!("  Throughput: {:.2} ops/sec", set_throughput);
    println!("  Avg latency: {:?}", set_duration / iterations);

    let start = Instant::now();

    for i in 0..iterations {
        cache.get(&format!("perf_key_{}", i)).await?;
    }

    let get_duration = start.elapsed();
    let get_throughput = iterations as f64 / get_duration.as_secs_f64();

    println!("Get operations:");
    println!("  Total time: {:?}", get_duration);
    println!("  Throughput: {:.2} ops/sec", get_throughput);
    println!("  Avg latency: {:?}", get_duration / iterations);
    println!();

    // Example 3: Hit rate monitoring over time
    println!("--- Example 3: Hit Rate Over Time ---");

    // Initial load (misses)
    println!("Initial load (1000 keys)...");
    for i in 0..1000 {
        cache.set(&format!("hit_test_{}", i), i).await?;
        cache.get(&format!("hit_test_{}", i)).await?;
    }

    let initial_metrics = cache.metrics();
    println!(
        "Initial hit rate: {:.2}%",
        initial_metrics.l1_hit_rate() * 100.0
    );

    // Repeated access (should be mostly hits)
    println!("Repeated access (same 1000 keys)...");
    for _ in 0..10 {
        for i in 0..1000 {
            cache.get(&format!("hit_test_{}", i)).await?;
        }
    }

    let final_metrics = cache.metrics();
    println!(
        "Final hit rate: {:.2}%",
        final_metrics.l1_hit_rate() * 100.0
    );
    println!(
        "Hit rate improvement: +{:.2}%",
        (final_metrics.l1_hit_rate() - initial_metrics.l1_hit_rate()) * 100.0
    );
    println!();

    // Example 4: Capacity monitoring
    println!("--- Example 4: Capacity Monitoring ---");

    if let Some(l1) = cache.l1_tier() {
        println!("L1 Cache:");
        println!("  Current size: {} bytes", l1.size());
        println!("  Capacity: {} bytes", l1.capacity());
        println!(
            "  Usage: {:.2}%",
            (l1.size() as f64 / l1.capacity() as f64) * 100.0
        );
        println!(
            "  Available: {} bytes",
            l1.capacity() - l1.size()
        );
    }
    println!();

    // Example 5: Continuous monitoring
    println!("--- Example 5: Continuous Monitoring ---");
    println!("Monitoring cache for 5 seconds...\n");

    let monitor_cache = cache.clone();
    let monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut last_ops = 0;

        for i in 1..=5 {
            interval.tick().await;

            let metrics = monitor_cache.metrics();
            let total_ops = metrics.total_ops();
            let ops_since_last = total_ops - last_ops;
            last_ops = total_ops;

            println!(
                "[{}] Ops/sec: {} | Hit rate: {:.2}% | Total: {}",
                i,
                ops_since_last,
                metrics.overall_hit_rate() * 100.0,
                total_ops
            );
        }
    });

    // Simulate workload
    let workload_task = tokio::spawn(async move {
        for i in 0..500 {
            cache.set(&format!("monitor_key_{}", i), i).await?;
            cache.get(&format!("monitor_key_{}", i)).await?;
            cache.get(&format!("monitor_key_{}", i % 100)).await?; // Some hits
            sleep(Duration::from_micros(100)).await;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    });

    // Wait for both tasks
    tokio::select! {
        _ = monitor_task => {},
        _ = workload_task => {},
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
