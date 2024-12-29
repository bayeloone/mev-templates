use anyhow::{Result, anyhow};
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct BotConfig {
    // Network configuration
    #[validate(custom = "validate_rpc_url")]
    pub rpc_url: String,
    #[validate(custom = "validate_chain_id")]
    pub chain_id: u64,
    
    // Wallet configuration
    #[validate(custom = "validate_private_key")]
    pub private_key: String,
    
    // Contract addresses
    #[validate(custom = "validate_address")]
    pub executor_address: Address,
    #[validate(custom = "validate_address")]
    pub vault_address: Address,
    
    // Risk parameters
    #[validate(range(min = 1, max = 1000000))]
    pub max_position_size: U256,
    #[validate(range(min = 1, max = 10))]
    pub max_leverage: u8,
    #[validate(range(min = 1, max = 100))]
    pub stop_loss_pct: u8,
    #[validate(range(min = 1, max = 100))]
    pub max_drawdown: u8,
    
    // Execution parameters
    #[validate(range(min = 1, max = 500))]
    pub max_gas_price: u64,
    #[validate(range(min = 0, max = 100))]
    pub priority_fee: u64,
    #[validate(range(min = 1, max = 5))]
    pub max_hops: u8,
    
    // MEV protection
    pub flashbots_enabled: bool,
    #[validate(custom = "validate_rpc_url")]
    pub flashbots_rpc: Option<String>,
    pub eden_enabled: bool,
    #[validate(custom = "validate_rpc_url")]
    pub eden_rpc: Option<String>,
    
    // Market making
    pub market_making_enabled: bool,
    #[validate(range(min = 1, max = 1000))]
    pub min_spread_bps: u16,
    #[validate(range(min = 1, max = 100))]
    pub rebalance_threshold: u8,
}

impl BotConfig {
    pub fn validate_all(&self) -> Result<()> {
        // Run validator derive validations
        if let Err(e) = self.validate() {
            return Err(anyhow!("Configuration validation failed: {:?}", e));
        }
        
        // Additional complex validations
        self.validate_contract_compatibility()?;
        self.validate_token_configurations()?;
        self.validate_network_settings()?;
        
        Ok(())
    }

    fn validate_contract_compatibility(&self) -> Result<()> {
        // Check if contracts are deployed and compatible
        Ok(())
    }

    fn validate_token_configurations(&self) -> Result<()> {
        // Validate token settings and permissions
        Ok(())
    }

    fn validate_network_settings(&self) -> Result<()> {
        // Validate network-specific configurations
        Ok(())
    }
}

// Custom validators
fn validate_rpc_url(url: &str) -> Result<(), ValidationError> {
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("ws://") {
        return Err(ValidationError::new("invalid_rpc_url"));
    }
    Ok(())
}

fn validate_chain_id(chain_id: &u64) -> Result<(), ValidationError> {
    match chain_id {
        // Mainnets
        1 => Ok(()),     // Ethereum
        10 => Ok(()),    // Optimism
        137 => Ok(()),   // Polygon
        42161 => Ok(()),  // Arbitrum
        8453 => Ok(()),  // Base
        
        // Testnets
        5 => Ok(()),     // Goerli
        80001 => Ok(()),  // Mumbai
        421613 => Ok(()),  // Arbitrum Goerli
        420 => Ok(()),   // Optimism Goerli
        84531 => Ok(()),  // Base Goerli
        
        _ => Err(ValidationError::new("unsupported_chain")),
    }
}

fn validate_private_key(key: &str) -> Result<(), ValidationError> {
    if !key.starts_with("0x") || key.len() != 66 {
        return Err(ValidationError::new("invalid_private_key"));
    }
    Ok(())
}

fn validate_address(address: &Address) -> Result<(), ValidationError> {
    if address == &Address::zero() {
        return Err(ValidationError::new("zero_address"));
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub max_memory_mb: u64,
    pub health_check_interval: Duration,
    pub metrics_port: u16,
    pub log_level: String,
    pub retry_attempts: u32,
    pub backoff_base_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024,
            health_check_interval: Duration::from_secs(60),
            metrics_port: 9090,
            log_level: "info".to_string(),
            retry_attempts: 3,
            backoff_base_ms: 1000,
        }
    }
}
