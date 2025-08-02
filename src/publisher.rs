use anyhow::{anyhow, Result};
use log::{error, info, warn};
use redis::AsyncCommands;
use tokio::sync::mpsc;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::exchanges::{self, Exchange, ExchangeImpl};
use crate::types::{self, PriceUpdate, TradingPair};

const CHANNEL_SIZE: usize = 1000;
const REDIS_PRICE_EXPIRY: usize = 60; // 60 seconds
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const STALE_PRICE_THRESHOLD: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct ExchangeHealth {
    pub last_update: SystemTime,
    pub is_connected: bool,
    pub error_count: u32,
}

pub struct PricePublisher {
    exchanges: Vec<Arc<ExchangeImpl>>,
    redis_client: redis::Client,
    health_metrics: Arc<RwLock<HashMap<String, ExchangeHealth>>>,
    latest_prices: Arc<RwLock<HashMap<String, HashMap<String, (f64, SystemTime)>>>>,
}

impl PricePublisher {
    pub async fn new() -> Result<Self> {
        // Initialize Redis client without authentication
        let redis_url = "redis://127.0.0.1/";
        let redis_client = redis::Client::open(redis_url)?;

        // Test the connection
        let mut conn = redis_client.get_async_connection().await?;
        redis::cmd("PING").query_async(&mut conn).await?;
        info!("Successfully connected to Redis");

        // Define trading pairs to track
        let trading_pairs = vec![
            TradingPair::new("BTC", "USDT"),
            TradingPair::new("ETH", "USDT"),
            TradingPair::new("SOL", "USDT"),
            TradingPair::new("USDC", "USDT"), // For Coinbase special case
        ];
        info!("Initializing with trading pairs: {:?}", trading_pairs);

        // Initialize exchanges
        let mut exchanges: Vec<Arc<ExchangeImpl>> = Vec::new();
        let mut health_metrics = HashMap::new();

        // Create exchange instances
        let exchange_types = [
            types::Exchange::Binance,
            types::Exchange::Bybit,
            types::Exchange::Coinbase,
            types::Exchange::Hyperliquid,
        ];

        for exchange_type in exchange_types.iter() {
            match exchanges::create_exchange(*exchange_type, trading_pairs.clone()).await {
                Ok(mut exchange) => {
                    let exchange_name = exchange_type.as_str().to_string();
                    if let Err(e) = exchange.init().await {
                        error!("Failed to initialize {}: {}", exchange_name, e);
                        health_metrics.insert(
                            exchange_name,
                            ExchangeHealth {
                                last_update: SystemTime::now(),
                                is_connected: false,
                                error_count: 1,
                            },
                        );
                        continue;
                    }
                    health_metrics.insert(
                        exchange_name,
                        ExchangeHealth {
                            last_update: SystemTime::now(),
                            is_connected: true,
                            error_count: 0,
                        },
                    );
                    exchanges.push(Arc::new(exchange));
                }
                Err(e) => {
                    error!("Failed to create {}: {}", exchange_type.as_str(), e);
                    health_metrics.insert(
                        exchange_type.as_str().to_string(),
                        ExchangeHealth {
                            last_update: SystemTime::now(),
                            is_connected: false,
                            error_count: 1,
                        },
                    );
                }
            }
        }

        if exchanges.is_empty() {
            return Err(anyhow!("No exchanges were successfully initialized"));
        }

        Ok(Self {
            exchanges,
            redis_client,
            health_metrics: Arc::new(RwLock::new(health_metrics)),
            latest_prices: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn update_health_metrics(&self, exchange: &str, is_healthy: bool, had_error: bool) {
        let mut health_metrics = self.health_metrics.write().await;
        if let Some(metrics) = health_metrics.get_mut(exchange) {
            metrics.last_update = SystemTime::now();
            metrics.is_connected = is_healthy;
            if had_error {
                metrics.error_count += 1;
            } else {
                metrics.error_count = 0;
            }
        }
    }

    async fn run_health_checks(&self) {
        let mut interval = interval(HEALTH_CHECK_INTERVAL);

        loop {
            interval.tick().await;
            let health_metrics = self.health_metrics.read().await;
            let latest_prices = self.latest_prices.read().await;

            for (exchange, metrics) in health_metrics.iter() {
                // Check connection status
                if !metrics.is_connected {
                    warn!("{} is disconnected", exchange);
                }

                // Check error count
                if metrics.error_count > 5 {
                    error!("{} has high error count: {}", exchange, metrics.error_count);
                }

                // Check last update time
                if let Ok(elapsed) = SystemTime::now().duration_since(metrics.last_update) {
                    if elapsed > STALE_PRICE_THRESHOLD {
                        warn!(
                            "{} hasn't updated in {} seconds",
                            exchange,
                            elapsed.as_secs()
                        );
                    }
                }
            }

            // Check for stale prices
            for (symbol, sources) in latest_prices.iter() {
                for (source, (_, timestamp)) in sources.iter() {
                    if let Ok(elapsed) = SystemTime::now().duration_since(*timestamp) {
                        if elapsed > STALE_PRICE_THRESHOLD {
                            warn!(
                                "Stale price for {}/{}: {} seconds old",
                                symbol,
                                source,
                                elapsed.as_secs()
                            );
                        }
                    }
                }
            }
        }
    }

    async fn write_to_redis(&self, update: &PriceUpdate) -> Result<()> {
        let mut conn = self.redis_client.get_async_connection().await?;

        // Write the latest price
        let price_key = format!("price:{}", update.symbol);
        conn.set_ex(&price_key, update.price.to_string(), REDIS_PRICE_EXPIRY)
            .await?;

        // Write source information
        let sources_key = format!("price:{}:sources", update.symbol);
        let timestamp = update
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let source_info = format!("{}:{:.8}:{}", update.source, update.price, timestamp);
        conn.set_ex(&sources_key, source_info, REDIS_PRICE_EXPIRY)
            .await?;

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        let (price_sender, mut price_receiver) = mpsc::channel(CHANNEL_SIZE);

        // Spawn health check monitoring
        // let health_check_handle = {
        //     let publisher = self.clone();
        //     tokio::spawn(async move {
        //         publisher.run_health_checks().await;
        //     })
        // };

        // Spawn exchange listeners
        for exchange in &self.exchanges {
            let price_sender = price_sender.clone();
            let exchange_name = exchange.get_name().to_string();
            let health_metrics = self.health_metrics.clone();
            let exchange = Arc::new(exchange.as_ref().clone());

            tokio::spawn(async move {
                loop {
                    info!("Starting {} price feed", exchange_name);
                    match exchange.listen(price_sender.clone()).await {
                        Ok(_) => {
                            let mut metrics = health_metrics.write().await;
                            if let Some(m) = metrics.get_mut(&exchange_name) {
                                m.is_connected = true;
                                m.error_count = 0;
                            }
                        }
                        Err(e) => {
                            error!("{} price feed error: {}", exchange_name, e);
                            let mut metrics = health_metrics.write().await;
                            if let Some(m) = metrics.get_mut(&exchange_name) {
                                m.is_connected = false;
                                m.error_count += 1;
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            });
        }

        // Process price updates
        while let Some(update) = price_receiver.recv().await {
            // Update latest prices
            {
                let mut latest_prices = self.latest_prices.write().await;
                latest_prices
                    .entry(update.symbol.clone())
                    .or_default()
                    .insert(update.source.clone(), (update.price, update.timestamp));
            }

            // Write to Redis
            if let Err(e) = self.write_to_redis(&update).await {
                error!("Failed to write to Redis: {}", e);
            }

            info!(
                "Received price update from {}: {} = {}",
                update.source, update.symbol, update.price
            );
        }

        // Keep the main task alive
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn get_exchange_health(&self) -> HashMap<String, ExchangeHealth> {
        self.health_metrics.read().await.clone()
    }

    pub async fn get_latest_prices(&self) -> HashMap<String, HashMap<String, (f64, SystemTime)>> {
        self.latest_prices.read().await.clone()
    }
}
