use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub symbol: String,
    pub price: f64,
    pub timestamp: SystemTime,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exchange {
    Binance,
    Bybit,
    Coinbase,
    Hyperliquid,
    UniswapV2,
}

impl Exchange {
    pub fn as_str(&self) -> &'static str {
        match self {
            Exchange::Binance => "binance",
            Exchange::Bybit => "bybit",
            Exchange::Coinbase => "coinbase",
            Exchange::Hyperliquid => "hyperliquid",
            Exchange::UniswapV2 => "univ2",
        }
    }
}

// Represents a trading pair (e.g., BTC/USD)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TradingPair {
    pub base: String,  // e.g., "BTC"
    pub quote: String, // e.g., "USD"
}

impl TradingPair {
    pub fn new(base: &str, quote: &str) -> Self {
        Self {
            base: base.to_uppercase(),
            quote: quote.to_uppercase(),
        }
    }

    pub fn to_binance_symbol(&self) -> String {
        format!("{}{}", self.base, self.quote)
    }

    pub fn to_bybit_symbol(&self) -> String {
        format!("{}{}", self.base, self.quote)
    }

    pub fn to_coinbase_symbol(&self) -> String {
        format!("{}-{}", self.base, self.quote)
    }

    pub fn to_redis_key(&self) -> String {
        format!("price:{}:{}", self.base, self.quote)
    }
}
