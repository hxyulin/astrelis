//! Example demonstrating async task execution with the TaskPool.
//!
//! This example shows how to:
//! - Use AsyncRuntimePlugin to add async support
//! - Spawn parallel async tasks
//! - Wait for task completion
//! - Use async for background work
//!
//! Run with: cargo run -p astrelis --example async_tasks

use astrelis::prelude::*;
use std::time::{Duration, Instant};

fn main() {
    println!("=== Example 1: Basic async task execution ===\n");

    let engine = Engine::builder()
        .add_plugin(AsyncRuntimePlugin::default())
        .build();

    let pool = engine.get::<TaskPool>().unwrap();

    // Spawn a simple async task
    let task = pool.spawn(async {
        println!("Task running on thread: {:?}", std::thread::current().id());
        42
    });

    // Wait for the task to complete
    let result = pollster::block_on(task);
    println!("Task completed with result: {}\n", result);

    println!("=== Example 2: Parallel task execution ===\n");

    let start = Instant::now();

    // Spawn multiple tasks that run in parallel
    let tasks: Vec<_> = (0..8)
        .map(|i| {
            pool.spawn(async move {
                println!("Task {} starting on thread {:?}", i, std::thread::current().id());

                // Simulate some work
                std::thread::sleep(Duration::from_millis(100));

                println!("Task {} completed", i);
                i * 2
            })
        })
        .collect();

    // Wait for all tasks to complete
    let results: Vec<_> = tasks
        .into_iter()
        .map(|t| pollster::block_on(t))
        .collect();

    let elapsed = start.elapsed();
    println!("\nAll tasks completed in {:?}", elapsed);
    println!("Results: {:?}\n", results);

    // Note: With 8 tasks running in parallel on multiple threads,
    // this should take ~100ms instead of 800ms sequential execution

    println!("=== Example 3: Async/await composition ===\n");

    let task1 = pool.spawn(async {
        println!("Subtask 1 running");
        std::thread::sleep(Duration::from_millis(50));
        10
    });

    let task2 = pool.spawn(async {
        println!("Subtask 2 running");
        std::thread::sleep(Duration::from_millis(50));
        20
    });

    // Compose multiple async tasks
    let combined = pool.spawn(async move {
        let a = task1.await;
        let b = task2.await;
        println!("Combined results: {} + {} = {}", a, b, a + b);
        a + b
    });

    let final_result = pollster::block_on(combined);
    println!("Final result: {}\n", final_result);

    println!("=== Example 4: Error handling ===\n");

    let fallible_task = pool.spawn(async {
        // Simulate a task that might fail
        if std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            % 2
            == 0
        {
            Ok::<i32, &'static str>(100)
        } else {
            Err("Something went wrong")
        }
    });

    match pollster::block_on(fallible_task) {
        Ok(value) => println!("Task succeeded with value: {}", value),
        Err(e) => println!("Task failed with error: {}", e),
    }

    println!("\n=== Example 5: Custom thread count ===\n");

    let custom_engine = Engine::builder()
        .add_plugin(AsyncRuntimePlugin::new().with_threads(2))
        .build();

    let custom_pool = custom_engine.get::<TaskPool>().unwrap();
    println!("Created TaskPool with {} threads", custom_pool.thread_count());

    // Spawn tasks on the custom pool
    let task = custom_pool.spawn(async {
        println!("Running on custom thread pool");
        "done"
    });

    let result = pollster::block_on(task);
    println!("Result: {}\n", result);

    println!("All examples completed successfully!");
}
