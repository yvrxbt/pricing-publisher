use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use log::{error, info};
use serde::Deserialize;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::mpsc::Sender;

use super::{ws_stream::WsStream, Exchange};
use crate::types::{PriceUpdate, TradingPair};

pub struct BybitExchange {
    trading_pairs: Vec<TradingPair>,
    last_heartbeat: AtomicI64,
}

impl Clone for BybitExchange {
    fn clone(&self) -> Self {
        Self {
            trading_pairs: self.trading_pairs.clone(),
            last_heartbeat: AtomicI64::new(self.last_heartbeat.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BybitOrderbook {
    topic: String,
    data: BybitOrderbookData,
}

#[derive(Debug, Deserialize)]
struct BybitOrderbookData {
    #[serde(rename = "b")]
    bids: Vec<Vec<String>>,
    #[serde(rename = "a")]
    asks: Vec<Vec<String>>,
}

impl BybitExchange {
    pub fn new(trading_pairs: Vec<TradingPair>) -> Self {
        Self {
            trading_pairs,
            last_heartbeat: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    fn get_websocket_url(&self) -> String {
        "wss://stream.bybit.com/v5/public/spot".to_string()
    }

    fn create_subscription_message(&self) -> String {
        let args = self
            .trading_pairs
            .iter()
            .map(|pair| format!("orderbook.1.{}", pair.to_bybit_symbol()))
            .collect::<Vec<_>>();

        serde_json::json!({
            "op": "subscribe",
            "args": args
        })
        .to_string()
    }

    fn update_heartbeat(&self) {
        self.last_heartbeat
            .store(Utc::now().timestamp(), Ordering::SeqCst);
    }
}

#[async_trait]
impl Exchange for BybitExchange {
    async fn init(&mut self) -> Result<()> {
        // Bybit doesn't require initialization
        Ok(())
    }

    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()> {
        let mut ws = WsStream::connect(&self.get_websocket_url()).await?;
        info!("Connected to Bybit WebSocket");

        // Send subscription message
        let subscription_msg = self.create_subscription_message();
        ws.send_text(subscription_msg.clone()).await?;
        info!("Sent subscription message to Bybit: {}", subscription_msg);

        self.update_heartbeat();

        while let Some(text) = ws.read_text().await? {
            if let Ok(orderbook) = serde_json::from_str::<BybitOrderbook>(&text) {
                if let (Some(best_bid), Some(best_ask)) = (
                    orderbook
                        .data
                        .bids
                        .first()
                        .and_then(|bid| bid[0].parse::<f64>().ok()),
                    orderbook
                        .data
                        .asks
                        .first()
                        .and_then(|ask| ask[0].parse::<f64>().ok()),
                ) {
                    let mid_price = (best_bid + best_ask) / 2.0;
                    let symbol = orderbook
                        .topic
                        .strip_prefix("orderbook.1.")
                        .unwrap_or(&orderbook.topic)
                        .to_string();

                    let update = PriceUpdate {
                        symbol,
                        price: mid_price,
                        timestamp: Utc::now().into(),
                        source: "bybit".to_string(),
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
        "bybit"
    }

    async fn is_healthy(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::SeqCst);
        let age = Utc::now().timestamp() - last;
        age < 10
    }
}
