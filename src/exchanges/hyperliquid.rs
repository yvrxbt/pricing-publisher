use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::{error, info};
use serde::Deserialize;
use tokio::sync::mpsc::Sender;

use chrono::Utc;
use std::sync::atomic::{AtomicI64, Ordering};

use super::{ws_stream::WsStream, Exchange};
use crate::types::{PriceUpdate, TradingPair};

pub struct HyperliquidExchange {
    trading_pairs: Vec<TradingPair>,
    last_heartbeat: AtomicI64,
}

impl Clone for HyperliquidExchange {
    fn clone(&self) -> Self {
        Self {
            trading_pairs: self.trading_pairs.clone(),
            last_heartbeat: AtomicI64::new(self.last_heartbeat.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct HyperliquidMessage {
    channel: String,
    data: HyperliquidData,
}

#[derive(Debug, Deserialize)]
struct HyperliquidData {
    mids: std::collections::HashMap<String, String>,
}

impl HyperliquidExchange {
    pub fn new(trading_pairs: Vec<TradingPair>) -> Self {
        Self {
            trading_pairs,
            last_heartbeat: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    fn get_websocket_url(&self) -> String {
        "wss://api.hyperliquid.xyz/ws".to_string()
    }

    fn create_subscription_message(&self) -> String {
        serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "allMids",
            }
        })
        .to_string()
    }

    fn update_heartbeat(&self) {
        self.last_heartbeat
            .store(Utc::now().timestamp(), Ordering::SeqCst);
    }
}

#[async_trait]
impl Exchange for HyperliquidExchange {
    async fn init(&mut self) -> Result<()> {
        // Hyperliquid doesn't require initialization
        Ok(())
    }

    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()> {
        let mut ws = WsStream::connect(&self.get_websocket_url()).await?;
        info!("Connected to Hyperliquid WebSocket");

        // Send subscription message
        let subscription_msg = self.create_subscription_message();
        ws.send_text(subscription_msg.clone()).await?;
        info!(
            "Sent subscription message to Hyperliquid: {}",
            subscription_msg
        );

        self.update_heartbeat();

        while let Some(text) = ws.read_text().await? {
            if let Ok(message) = serde_json::from_str::<HyperliquidMessage>(&text) {
                if message.channel == "allMids" {
                    for (symbol, price_str) in message.data.mids {
                        if let Ok(price) = price_str.parse::<f64>() {
                            let update = PriceUpdate {
                                symbol,
                                price,
                                timestamp: Utc::now().into(),
                                source: "hyperliquid".to_string(),
                            };

                            if let Err(e) = price_sender.send(update).await {
                                error!("Failed to send price update: {}", e);
                                return Err(anyhow!("Channel closed"));
                            }

                            self.update_heartbeat();
                        }
                    }
                }
            }
        }

        Err(anyhow!("WebSocket stream ended"))
    }

    fn get_trading_pairs(&self) -> &[TradingPair] {
        &self.trading_pairs
    }

    fn get_name(&self) -> &'static str {
        "hyperliquid"
    }

    async fn is_healthy(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::SeqCst);
        let age = Utc::now().timestamp() - last;
        age < 10
    }
}
