use anyhow::{Result, anyhow};
use ethers::{
    providers::{Provider, Http},
    types::{U256, Address},
};
use std::{sync::Arc, time::SystemTime};
use crate::security::types::TWAPData;
use crate::dex::DexPool;

pub struct TWAPManager {
    /// Constants for TWAP calculations
    const MIN_TWAP_SAMPLES: usize = 3;
    const MIN_TWAP_CARDINALITY: u16 = 50;
    const MAX_TWAP_GAPS: usize = 2;
    const MAX_TICK_MOVEMENT: i64 = 1000; // About 10% price movement
}

impl TWAPManager {
    pub fn new() -> Self {
        Self {}
    }

    /// Get TWAP from Uniswap V3 pool with extensive validation
    pub async fn get_v3_twap(&self, pool: &DexPool, token: Address) -> Result<Option<TWAPData>> {
        let client = Arc::new(Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?);
        let pool_contract = UniswapV3Pool::new(pool.address, client.clone());

        // Get current state and validate pool health
        let (sqrt_price_x96, tick, _, _, _, fee_protocol, _) = pool_contract.slot0().call().await?;
        
        // Validate pool is active
        if sqrt_price_x96.is_zero() {
            return Ok(None); // Pool is not initialized
        }

        // Get pool tokens and validate
        let token0 = pool_contract.token0().call().await?;
        let token1 = pool_contract.token1().call().await?;
        let (base_token, quote_token) = if token == token0 {
            (token0, token1)
        } else if token == token1 {
            (token1, token0)
        } else {
            return Err(anyhow!("Token not found in pool"));
        };

        // Calculate time points for TWAP
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
        let seconds_ago = self.get_twap_observation_times(now)?;
        
        // Get observations
        let (ticks, initialized) = pool_contract.observe(seconds_ago.clone()).call().await?;
        
        // Validate observations
        if !self.validate_observations(&initialized)? {
            return Ok(None); // Not enough valid observations
        }

        // Calculate TWAP with cardinality checking
        let cardinality = pool_contract.observation_cardinality().call().await?;
        if cardinality < Self::MIN_TWAP_CARDINALITY {
            return Ok(None); // Not enough historical data
        }

        self.calculate_twap(token0, token, &ticks, &initialized, &seconds_ago, now)
    }

    /// Calculate TWAP from tick data
    fn calculate_twap(
        &self,
        token0: Address,
        token: Address,
        ticks: &[i64],
        initialized: &[bool],
        seconds_ago: &[u32],
        now: u64,
    ) -> Result<Option<TWAPData>> {
        let mut twap_sum = U256::zero();
        let mut valid_samples = 0;
        
        for i in 0..ticks.len()-1 {
            if !initialized[i] || !initialized[i+1] {
                continue;
            }

            let tick_diff = ticks[i+1] - ticks[i];
            let time_diff = seconds_ago[i+1] - seconds_ago[i];
            
            // Validate tick movement
            if !self.validate_tick_movement(tick_diff)? {
                continue;
            }

            let tick_avg = tick_diff.checked_div(time_diff as i64)
                .ok_or_else(|| anyhow!("TWAP calculation error"))?;
                
            // Convert tick to price
            let price = self.tick_to_sqrt_price(tick_avg)?;
            
            // Adjust price based on token order
            let adjusted_price = if token == token0 {
                price
            } else {
                U256::from(1u128 << 192).checked_div(price)
                    .ok_or_else(|| anyhow!("Price inversion overflow"))?
            };

            twap_sum = twap_sum.saturating_add(adjusted_price);
            valid_samples += 1;
        }

        if valid_samples < Self::MIN_TWAP_SAMPLES {
            return Ok(None);
        }

        let twap_price = twap_sum.checked_div(U256::from(valid_samples))
            .ok_or_else(|| anyhow!("TWAP average calculation error"))?;

        Ok(Some(TWAPData {
            price: twap_price,
            timestamp: now,
            samples: valid_samples as u32,
        }))
    }

    /// Get observation times for TWAP calculation
    fn get_twap_observation_times(&self, now: u64) -> Result<Vec<u32>> {
        // Default to 5-minute intervals over 30 minutes
        Ok(vec![
            30 * 60,  // 30 minutes
            25 * 60,  // 25 minutes
            20 * 60,  // 20 minutes
            15 * 60,  // 15 minutes
            10 * 60,  // 10 minutes
            5 * 60,   // 5 minutes
            0,        // Current
        ])
    }

    /// Validate TWAP observations
    fn validate_observations(&self, initialized: &[bool]) -> Result<bool> {
        let valid_count = initialized.iter().filter(|&&x| x).count();
        
        // Require at least MIN_TWAP_SAMPLES valid observations
        if valid_count < Self::MIN_TWAP_SAMPLES {
            return Ok(false);
        }
        
        // Check for gaps
        let max_gap = initialized.windows(2)
            .filter(|w| !w[0] || !w[1])
            .count();
            
        if max_gap > Self::MAX_TWAP_GAPS {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Validate tick movement is within bounds
    fn validate_tick_movement(&self, tick_diff: i64) -> Result<bool> {
        if tick_diff.abs() > Self::MAX_TICK_MOVEMENT {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Convert tick to sqrt price
    fn tick_to_sqrt_price(&self, tick: i64) -> Result<U256> {
        // Price = 1.0001^tick
        let sqrt_price = (1.0001f64.powi(tick as i32) * (1u128 << 96) as f64) as u128;
        Ok(U256::from(sqrt_price))
    }
}
