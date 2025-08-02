use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use log::{error, info};
use serde::Deserialize;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::mpsc::Sender;

use super::{ws_stream::WsStream, Exchange};
use crate::types::{PriceUpdate, TradingPair};

pub struct BinanceExchange {
    trading_pairs: Vec<TradingPair>,
    last_heartbeat: AtomicI64,
}

impl Clone for BinanceExchange {
    fn clone(&self) -> Self {
        Self {
            trading_pairs: self.trading_pairs.clone(),
            last_heartbeat: AtomicI64::new(self.last_heartbeat.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BinanceBookTicker {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    best_bid: String,
    #[serde(rename = "a")]
    best_ask: String,
}

impl BinanceExchange {
    pub fn new(trading_pairs: Vec<TradingPair>) -> Self {
        Self {
            trading_pairs,
            last_heartbeat: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    fn get_websocket_url(&self) -> String {
        let symbols = self
            .trading_pairs
            .iter()
            .map(|pair| pair.to_binance_symbol().to_lowercase())
            .collect::<Vec<_>>()
            .join("/");
        format!("wss://stream.binance.com:9443/ws/{}@bookTicker", symbols)
    }

    fn create_subscription_message(&self) -> String {
        serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [format!("{}@bookTicker", self.trading_pairs.iter().map(|pair| pair.to_binance_symbol().to_lowercase()).collect::<Vec<_>>().join("/"))],
            "id": 1
        }).to_string()
    }

    fn update_heartbeat(&self) {
        self.last_heartbeat
            .store(Utc::now().timestamp(), Ordering::SeqCst);
    }
}

#[async_trait]
impl Exchange for BinanceExchange {
    async fn init(&mut self) -> Result<()> {
        // Binance doesn't require initialization
        Ok(())
    }

    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()> {
        let mut ws = WsStream::connect(&self.get_websocket_url()).await?;
        info!("Connected to Binance WebSocket");

        // Send subscription message
        let subscription_msg = self.create_subscription_message();
        ws.send_text(subscription_msg.clone()).await?;
        info!("Sent subscription message to Binance: {}", subscription_msg);

        self.update_heartbeat();

        while let Some(text) = ws.read_text().await? {
            if let Ok(ticker) = serde_json::from_str::<BinanceBookTicker>(&text) {
                let best_bid = ticker.best_bid.parse::<f64>()?;
                let best_ask = ticker.best_ask.parse::<f64>()?;
                let mid_price = (best_bid + best_ask) / 2.0;

                let update = PriceUpdate {
                    symbol: ticker.symbol,
                    price: mid_price,
                    timestamp: Utc::now().into(),
                    source: "binance".to_string(),
                };

                if let Err(e) = price_sender.send(update).await {
                    error!("Failed to send price update: {}", e);
                    return Err(anyhow!("Channel closed"));
                }

                self.update_heartbeat();
            }
        }

        Err(anyhow!("WebSocket stream ended"))
    }

    fn get_trading_pairs(&self) -> &[TradingPair] {
        &self.trading_pairs
    }

    fn get_name(&self) -> &'static str {
        "binance"
    }

    async fn is_healthy(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::SeqCst);
        let age = Utc::now().timestamp() - last;
        age < 10
    }
}
