use anyhow::Result;
use redis::AsyncCommands;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // Simple Redis connection without auth
    let redis_url = "redis://127.0.0.1/";

    println!("Connecting to Redis...");
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_async_connection().await?;

    // Define symbols to monitor
    let symbols = vec!["BTCUSDT", "ETHUSDT", "SOLUSDT"];

    println!("Successfully connected to Redis!");
    println!("Press Ctrl+C to exit\n");

    loop {
        println!("\n=== Current Prices ===");
        for symbol in &symbols {
            // Get latest price
            let price_key = format!("price:{}", symbol);
            let price: Option<String> = conn.get(&price_key).await?;

            // Get sources information
            let sources_key = format!("price:{}:sources", symbol);
            let sources: Option<String> = conn.get(&sources_key).await?;

            match (price, sources) {
                (Some(price), Some(sources)) => {
                    println!("{}: {}", symbol, price);
                    let parts: Vec<&str> = sources.split(':').collect();
                    if parts.len() >= 3 {
                        let source = parts[0];
                        let timestamp = parts[2].parse::<u64>()?;
                        let age = SystemTime::now()
                            .duration_since(UNIX_EPOCH)?
                            .as_secs()
                            .saturating_sub(timestamp);
                        println!("  Source: {} ({}s ago)", source, age);
                    }
                }
                _ => println!("{}: No data available", symbol),
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
