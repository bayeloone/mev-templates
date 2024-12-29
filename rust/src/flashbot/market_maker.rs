use anyhow::Result;
use ethers::{
    types::{Address, U256},
    providers::{Provider, Http},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct MarketMaker {
    // Liquidity config
    max_pool_exposure: U256,
    rebalance_threshold: u8,
    
    // Spread management
    min_spread_bps: u16,
    dynamic_spread: bool,
    
    // Inventory management
    target_inventory: HashMap<Address, U256>,
    inventory_range: HashMap<Address, (U256, U256)>,
    
    // Current state
    current_positions: Arc<RwLock<HashMap<Address, U256>>>,
    current_spreads: Arc<RwLock<HashMap<Address, u16>>>,
}

impl MarketMaker {
    pub fn new(
        max_pool_exposure: U256,
        rebalance_threshold: u8,
        min_spread_bps: u16,
    ) -> Self {
        Self {
            max_pool_exposure,
            rebalance_threshold,
            min_spread_bps,
            dynamic_spread: true,
            target_inventory: HashMap::new(),
            inventory_range: HashMap::new(),
            current_positions: Arc::new(RwLock::new(HashMap::new())),
            current_spreads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update position for token
    pub async fn update_position(&self, token: Address, amount: U256) -> Result<()> {
        let mut positions = self.current_positions.write().await;
        positions.insert(token, amount);
        
        // Check if rebalance needed
        if self.needs_rebalance(token, amount).await? {
            self.rebalance_position(token).await?;
        }
        
        Ok(())
    }

    /// Calculate optimal spread
    pub async fn calculate_spread(&self, token: Address) -> Result<u16> {
        let mut spread = self.min_spread_bps;
        
        if self.dynamic_spread {
            // Adjust spread based on volatility
            let volatility = self.calculate_volatility(token).await?;
            spread = spread.saturating_add((volatility * 100.0) as u16);
            
            // Adjust spread based on inventory
            let inventory_ratio = self.get_inventory_ratio(token).await?;
            spread = spread.saturating_add((inventory_ratio * 50.0) as u16);
            
            // Adjust spread based on market conditions
            let market_impact = self.estimate_market_impact(token).await?;
            spread = spread.saturating_add((market_impact * 200.0) as u16);
        }
        
        // Update current spread
        self.current_spreads.write().await.insert(token, spread);
        
        Ok(spread)
    }

    /// Check if position needs rebalancing
    async fn needs_rebalance(&self, token: Address, amount: U256) -> Result<bool> {
        if let Some(&target) = self.target_inventory.get(&token) {
            let diff = if amount > target {
                amount - target
            } else {
                target - amount
            };
            
            let threshold = target.saturating_mul(U256::from(self.rebalance_threshold))
                .checked_div(U256::from(100))
                .unwrap_or_default();
                
            return Ok(diff > threshold);
        }
        Ok(false)
    }

    /// Rebalance position to target
    async fn rebalance_position(&self, token: Address) -> Result<()> {
        let current = self.current_positions.read().await.get(&token).copied()
            .unwrap_or_default();
            
        if let Some(&target) = self.target_inventory.get(&token) {
            if current > target {
                // Reduce position
                let amount = current - target;
                self.reduce_exposure(token, amount).await?;
            } else {
                // Increase position
                let amount = target - current;
                self.increase_exposure(token, amount).await?;
            }
        }
        
        Ok(())
    }

    /// Calculate position volatility
    async fn calculate_volatility(&self, token: Address) -> Result<f64> {
        // Get price history
        let prices = self.get_price_history(token).await?;
        
        // Calculate standard deviation
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
            
        Ok(variance.sqrt())
    }

    /// Get inventory ratio relative to target
    async fn get_inventory_ratio(&self, token: Address) -> Result<f64> {
        let current = self.current_positions.read().await.get(&token)
            .copied()
            .unwrap_or_default();
            
        if let Some(&target) = self.target_inventory.get(&token) {
            if target.is_zero() {
                return Ok(0.0);
            }
            
            let current_f = current.as_u128() as f64;
            let target_f = target.as_u128() as f64;
            
            return Ok((current_f - target_f) / target_f);
        }
        
        Ok(0.0)
    }

    /// Estimate market impact of trade
    async fn estimate_market_impact(&self, token: Address) -> Result<f64> {
        // Get pool depth
        let depth = self.get_pool_depth(token).await?;
        
        // Get recent trade volume
        let volume = self.get_recent_volume(token).await?;
        
        // Calculate impact
        let impact = volume.as_u128() as f64 / depth.as_u128() as f64;
        
        Ok(impact.min(1.0))
    }
}
