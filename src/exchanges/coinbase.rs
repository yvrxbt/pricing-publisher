use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use log::{error, info};
use serde::Deserialize;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::mpsc::Sender;

use super::{ws_stream::WsStream, Exchange};
use crate::types::{PriceUpdate, TradingPair};

pub struct CoinbaseExchange {
    trading_pairs: Vec<TradingPair>,
    last_heartbeat: AtomicI64,
}

impl Clone for CoinbaseExchange {
    fn clone(&self) -> Self {
        Self {
            trading_pairs: self.trading_pairs.clone(),
            last_heartbeat: AtomicI64::new(self.last_heartbeat.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CoinbaseTicker {
    product_id: String,
    best_bid: String,
    best_ask: String,
}

impl CoinbaseExchange {
    pub fn new(trading_pairs: Vec<TradingPair>) -> Self {
        Self {
            trading_pairs,
            last_heartbeat: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    fn get_websocket_url(&self) -> String {
        "wss://ws-feed.exchange.coinbase.com/ws".to_string()
    }

    fn create_subscription_message(&self) -> String {
        let product_ids = self
            .trading_pairs
            .iter()
            .map(|pair| pair.to_coinbase_symbol())
            .collect::<Vec<_>>();

        serde_json::json!({
            "type": "subscribe",
            "product_ids": product_ids,
            "channels": ["ticker"]
        })
        .to_string()
    }

    fn update_heartbeat(&self) {
        self.last_heartbeat
            .store(Utc::now().timestamp(), Ordering::SeqCst);
    }

    fn handle_usdc_usdt(&self, price_sender: &Sender<PriceUpdate>) -> Result<()> {
        // Special case: USDC/USDT is always 1:1
        if self.trading_pairs.iter().any(|pair| {
            pair.base.eq_ignore_ascii_case("USDC") && pair.quote.eq_ignore_ascii_case("USDT")
        }) {
            let update = PriceUpdate {
                symbol: "USDCUSDT".to_string(),
                price: 1.0,
                timestamp: Utc::now().into(),
                source: "coinbase".to_string(),
            };

            price_sender.try_send(update)?;
        }
        Ok(())
    }
}

#[async_trait]
impl Exchange for CoinbaseExchange {
    async fn init(&mut self) -> Result<()> {
        // Coinbase doesn't require initialization
        Ok(())
    }

    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()> {
        // Handle special case for USDC/USDT
        self.handle_usdc_usdt(&price_sender)?;

        let mut ws = WsStream::connect(&self.get_websocket_url()).await?;
        info!("Connected to Coinbase WebSocket");

        // Send subscription message
        let subscription_msg = self.create_subscription_message();
        ws.send_text(subscription_msg.clone()).await?;
        info!(
            "Sent subscription message to Coinbase: {}",
            subscription_msg
        );

        self.update_heartbeat();

        while let Some(text) = ws.read_text().await? {
            if let Ok(ticker) = serde_json::from_str::<CoinbaseTicker>(&text) {
                if let (Ok(best_bid), Ok(best_ask)) = (
                    ticker.best_bid.parse::<f64>(),
                    ticker.best_ask.parse::<f64>(),
                ) {
                    let mid_price = (best_bid + best_ask) / 2.0;
                    let symbol = ticker.product_id.replace("-", "");

                    let update = PriceUpdate {
                        symbol,
                        price: mid_price,
                        timestamp: Utc::now().into(),
                        source: "coinbase".to_string(),
                    };

                    if let Err(e) = price_sender.send(update).await {
                        error!("Failed to send price update: {}", e);
                        return Err(anyhow!("Channel closed"));
                    }

                    self.update_heartbeat();
                }
            }
        }

        Err(anyhow!("WebSocket stream ended"))
    }

    fn get_trading_pairs(&self) -> &[TradingPair] {
        &self.trading_pairs
    }

    fn get_name(&self) -> &'static str {
        "coinbase"
    }

    async fn is_healthy(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::SeqCst);
        let age = Utc::now().timestamp() - last;
        age < 10
    }
}
