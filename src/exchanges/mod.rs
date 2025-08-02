use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::types::{PriceUpdate, TradingPair};

pub mod binance;
pub mod bybit;
pub mod coinbase;
pub mod hyperliquid;
pub mod ws_stream;

#[derive(Clone)]
pub enum ExchangeImpl {
    Binance(binance::BinanceExchange),
    Bybit(bybit::BybitExchange),
    Coinbase(coinbase::CoinbaseExchange),
    Hyperliquid(hyperliquid::HyperliquidExchange),
}

#[async_trait]
impl Exchange for ExchangeImpl {
    async fn init(&mut self) -> Result<()> {
        match self {
            ExchangeImpl::Binance(e) => e.init().await,
            ExchangeImpl::Bybit(e) => e.init().await,
            ExchangeImpl::Coinbase(e) => e.init().await,
            ExchangeImpl::Hyperliquid(e) => e.init().await,
        }
    }

    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()> {
        match self {
            ExchangeImpl::Binance(e) => e.listen(price_sender).await,
            ExchangeImpl::Bybit(e) => e.listen(price_sender).await,
            ExchangeImpl::Coinbase(e) => e.listen(price_sender).await,
            ExchangeImpl::Hyperliquid(e) => e.listen(price_sender).await,
        }
    }

    fn get_trading_pairs(&self) -> &[TradingPair] {
        match self {
            ExchangeImpl::Binance(e) => e.get_trading_pairs(),
            ExchangeImpl::Bybit(e) => e.get_trading_pairs(),
            ExchangeImpl::Coinbase(e) => e.get_trading_pairs(),
            ExchangeImpl::Hyperliquid(e) => e.get_trading_pairs(),
        }
    }

    fn get_name(&self) -> &'static str {
        match self {
            ExchangeImpl::Binance(e) => e.get_name(),
            ExchangeImpl::Bybit(e) => e.get_name(),
            ExchangeImpl::Coinbase(e) => e.get_name(),
            ExchangeImpl::Hyperliquid(e) => e.get_name(),
        }
    }

    async fn is_healthy(&self) -> bool {
        match self {
            ExchangeImpl::Binance(e) => e.is_healthy().await,
            ExchangeImpl::Bybit(e) => e.is_healthy().await,
            ExchangeImpl::Coinbase(e) => e.is_healthy().await,
            ExchangeImpl::Hyperliquid(e) => e.is_healthy().await,
        }
    }
}

#[async_trait]
pub trait Exchange: Send + Sync + Clone {
    async fn init(&mut self) -> Result<()>;
    async fn listen(&self, price_sender: Sender<PriceUpdate>) -> Result<()>;
    fn get_trading_pairs(&self) -> &[TradingPair];
    fn get_name(&self) -> &'static str;
    async fn is_healthy(&self) -> bool;
}

pub async fn create_exchange(
    exchange_type: crate::types::Exchange,
    trading_pairs: Vec<TradingPair>,
) -> Result<ExchangeImpl> {
    match exchange_type {
        crate::types::Exchange::Binance => Ok(ExchangeImpl::Binance(
            binance::BinanceExchange::new(trading_pairs),
        )),
        crate::types::Exchange::Bybit => Ok(ExchangeImpl::Bybit(bybit::BybitExchange::new(
            trading_pairs,
        ))),
        crate::types::Exchange::Coinbase => Ok(ExchangeImpl::Coinbase(
            coinbase::CoinbaseExchange::new(trading_pairs),
        )),
        crate::types::Exchange::Hyperliquid => Ok(ExchangeImpl::Hyperliquid(
            hyperliquid::HyperliquidExchange::new(trading_pairs),
        )),
        crate::types::Exchange::UniswapV2 => Err(anyhow!("UniswapV2 exchange not implemented yet")),
    }
}
