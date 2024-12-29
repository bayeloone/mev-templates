use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http},
    types::{U256, Address},
};
use std::sync::Arc;
use crate::security::types::PriceSource;
use crate::dex::DexPool;

pub struct PriceManager {
    usd_tokens: Vec<Address>,
}

impl PriceManager {
    pub fn new() -> Self {
        // Initialize with known USD-based tokens
        let usd_tokens = vec![
            "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            "dAC17F958D2ee523a2206206994597C13D831ec7", // USDT
            "6B175474E89094C44Da98b954EedeAC495271d0F", // DAI
        ].into_iter()
         .map(|addr| Address::from_slice(&hex::decode(addr).unwrap()))
         .collect();

        Self { usd_tokens }
    }

    /// Get price from Uniswap V3 pool
    pub async fn get_uniswap_v3_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = UniswapV3Pool::new(pool.address, client.clone());

        // Get current price from slot0
        let (sqrt_price_x96, _, _, _, _, _, _) = pool_contract.slot0().call().await?;
        
        if sqrt_price_x96.is_zero() {
            return Ok(None);
        }

        // Get tokens
        let token0 = pool_contract.token0().call().await?;
        
        // Calculate price based on token order
        let price = if token == token0 {
            sqrt_price_x96
        } else {
            U256::from(1u128 << 96)
                .checked_div(sqrt_price_x96)
                .ok_or_else(|| anyhow!("Price inversion overflow"))?
        };

        Ok(Some(PriceSource {
            price,
            weight: 1.0,
            source: "UniswapV3".to_string(),
        }))
    }

    /// Get price from Balancer pool
    pub async fn get_balancer_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = BalancerPool::new(pool.address, client.clone());
        let vault_address = pool_contract.get_vault().call().await?;
        let vault = BalancerVault::new(vault_address, client);

        // Get pool tokens and balances
        let pool_id = pool_contract.get_pool_id().call().await?;
        let (tokens, balances, _) = vault.get_pool_tokens(pool_id).call().await?;

        // Find token index
        let token_index = tokens.iter().position(|&t| t == token)
            .ok_or_else(|| anyhow!("Token not found in pool"))?;

        // Get spot price
        let spot_price = pool_contract.get_spot_price(
            tokens[token_index],
            tokens[1 - token_index],
            balances[token_index],
            balances[1 - token_index],
        ).call().await?;

        Ok(Some(PriceSource {
            price: spot_price,
            weight: 0.8, // Lower weight due to potential manipulation
            source: "Balancer".to_string(),
        }))
    }

    /// Check if token is USD-based
    pub fn is_usd_token(&self, token: Address) -> bool {
        self.usd_tokens.contains(&token)
    }
}
