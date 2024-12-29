use ethers::types::{Address, U256};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub path: Vec<Address>,           // Path of tokens/pools
    pub expected_profit: U256,        // Expected profit in USD
    pub required_flash_amount: U256,  // Required flashloan amount
    pub risk_score: u8,              // Risk assessment (0-100)
    pub gas_cost: U256,              // Estimated gas cost
    pub execution_time_ms: u64,      // Expected execution time
    pub pools: Vec<PoolInfo>,        // Pools involved in arbitrage
    pub profit_token: Address,       // Token to receive profit in
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    pub address: Address,
    pub protocol: DexProtocol,
    pub token0: Address,
    pub token1: Address,
    pub reserves: (U256, U256),
    pub fee: u32,
    pub liquidity: U256,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DexProtocol {
    UniswapV2,
    UniswapV3,
    Curve,
    Balancer,
    Custom(Address),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanSource {
    pub protocol: LendingProtocol,
    pub token: Address,
    pub max_amount: U256,
    pub fee_bps: u16,
    pub gas_overhead: U256,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LendingProtocol {
    Aave,
    Compound,
    DyDx,
    MakerDAO,
    Custom(Address),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size: U256,
    pub max_leverage: u8,
    pub stop_loss_pct: u8,
    pub max_drawdown: u8,
    pub min_pool_liquidity: U256,
    pub max_price_impact_bps: u16,
    pub blacklisted_tokens: Vec<Address>,
    pub min_profit_threshold: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub max_gas_price: U256,
    pub priority_fee: U256,
    pub max_hops: u8,
    pub block_delay: u8,
    pub max_execution_time: Duration,
    pub min_profit_threshold: U256,
}

#[derive(Debug, Clone, Default)]
pub struct Analytics {
    // Performance metrics
    pub total_profit: U256,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub avg_profit_per_trade: U256,
    
    // Risk metrics
    pub max_drawdown: U256,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    
    // System metrics
    pub avg_execution_time: Duration,
    pub gas_spent: U256,
    pub errors: Vec<String>,
    
    // Historical data
    pub trade_history: Vec<TradeResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub opportunity: ArbitrageOpportunity,
    pub actual_profit: U256,
    pub gas_used: U256,
    pub execution_time: Duration,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: u64,
}
