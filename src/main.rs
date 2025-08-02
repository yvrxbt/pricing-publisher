use anyhow::Result;
use chrono::Local;
use env_logger::Builder;
use log::{info, warn, LevelFilter};
use redis::AsyncCommands;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use tokio::{
    self,
    time::{sleep, Duration},
};

mod exchanges;
mod publisher;
mod types;

fn init_logger() {
    // Create the base logs directory if it doesn't exist
    let logs_dir = "logs";
    fs::create_dir_all(logs_dir).expect("Failed to create logs directory");

    // Create the date-specific directory
    let date_dir = format!("{}/{}", logs_dir, Local::now().format("%Y%m%d"));
    fs::create_dir_all(&date_dir).expect("Failed to create date directory");

    // Create the log file path
    let filename = format!("{}/price_publisher.out", date_dir,);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filename)
        .expect("Failed to open log file");

    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y%m%d %H:%M:%S%.6f"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .target(env_logger::Target::Pipe(Box::new(file)))
        .init();
}

async fn monitor_redis_updates(redis_client: redis::Client, symbols: Vec<String>) -> Result<()> {
    let mut conn = redis_client.get_async_connection().await?;
    let mut last_prices: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

    loop {
        for symbol in &symbols {
            let redis_key = format!("price:{}", symbol);
            let price: Option<String> = conn.get(&redis_key).await?;

            if let Some(price_str) = price {
                if let Ok(price) = price_str.parse::<f64>() {
                    if let Some(last_price) = last_prices.get(symbol) {
                        let change = ((price - last_price) / last_price * 100.0).abs();
                        if change > 0.1 {
                            // Log if price changed more than 0.1%
                            info!(
                                "{}: {:.8} (changed {:.2}% from {:.8})",
                                symbol, price, change, last_price
                            );
                        }
                    } else {
                        info!("Initial {} price: {:.8}", symbol, price);
                    }
                    last_prices.insert(symbol.clone(), price);
                }
            } else {
                warn!("No price available for {}", symbol);
            }

            // Also read and log the sources
            let sources_key = format!("price:{}:sources", symbol);
            let sources: Option<String> = conn.get(&sources_key).await?;
            if let Some(sources) = sources {
                info!("{} sources: {}", symbol, sources);
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn monitor_exchange_health(publisher: Arc<publisher::PricePublisher>) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        let health = publisher.clone().get_exchange_health().await;
        let prices = publisher.clone().get_latest_prices().await;

        info!("\n=== Exchange Health Report ===");
        for (exchange, metrics) in health {
            info!(
                "{}: Connected={}, Errors={}, Last Update={:?}",
                exchange,
                metrics.is_connected,
                metrics.error_count,
                metrics
                    .last_update
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
        }

        info!("\n=== Price Sources Report ===");
        for (symbol, sources) in prices {
            info!("{}:", symbol);
            for (source, (price, timestamp)) in sources {
                let age = std::time::SystemTime::now()
                    .duration_since(timestamp)
                    .unwrap()
                    .as_secs();
                info!("  {}: {:.8} ({}s old)", source, price, age);
            }
        }
        info!("===========================\n");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logger();

    info!("Starting price publisher test app...");

    // Create the publisher
    let publisher = Arc::new(publisher::PricePublisher::new().await?);

    // Get Redis client for monitoring
    let redis_url = "redis://127.0.0.1/";
    let redis_client = redis::Client::open(redis_url)?;

    // Define symbols to monitor
    let symbols = vec![
        "BTCUSDT".to_string(),
        "ETHUSDT".to_string(),
        "SOLUSDT".to_string(),
    ];

    // Spawn monitoring tasks
    let redis_monitor = tokio::spawn(monitor_redis_updates(redis_client, symbols));
    let publisher_clone = publisher.clone();
    let health_monitor = tokio::spawn(monitor_exchange_health(publisher_clone));

    // Run the publisher
    let publisher_handle = tokio::spawn(async move {
        if let Err(e) = publisher.run().await {
            warn!("Publisher exited with error: {}", e);
        }
    });

    info!("All tasks started. Press Ctrl+C to exit.");

    // Wait for Ctrl+C
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = redis_monitor => {
            warn!("Redis monitor exited unexpectedly");
        }
        _ = health_monitor => {
            warn!("Health monitor exited unexpectedly");
        }
        _ = publisher_handle => {
            warn!("Publisher exited unexpectedly");
        }
    }

    Ok(())
}
