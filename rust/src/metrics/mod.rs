use anyhow::Result;
use ethers::types::{Address, U256};
use log::{info, warn, error};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use metrics::{counter, gauge, histogram};

// Metrics for monitoring
const METRIC_POOL_UPDATES: &str = "pool_updates_total";
const METRIC_PROFITABLE_PATHS: &str = "profitable_paths_total";
const METRIC_EXECUTION_TIME: &str = "execution_time_seconds";
const METRIC_GAS_PRICE: &str = "gas_price_gwei";

#[derive(Debug, Clone)]
pub struct PoolState {
    pub reserve0: U256,
    pub reserve1: U256,
    pub last_update: Instant,
    pub update_count: u64,
}

pub struct StateMonitor {
    pool_states: Arc<RwLock<HashMap<Address, PoolState>>>,
    price_thresholds: HashMap<Address, (U256, U256)>, // (min, max) prices
    update_frequency: Duration,
}

impl StateMonitor {
    pub fn new(update_frequency: Duration) -> Self {
        Self {
            pool_states: Arc::new(RwLock::new(HashMap::new())),
            price_thresholds: HashMap::new(),
            update_frequency,
        }
    }

    pub async fn monitor_pools(&self, pools: Vec<Address>) -> Result<()> {
        info!("Starting pool monitoring for {} pools", pools.len());
        
        loop {
            let start = Instant::now();
            
            for pool in &pools {
                if let Err(e) = self.update_pool_state(*pool).await {
                    error!("Failed to update pool {}: {:?}", pool, e);
                    continue;
                }
                
                // Update metrics
                counter!(METRIC_POOL_UPDATES, 1);
                
                // Check for significant changes
                if let Some(changes) = self.check_significant_changes(*pool).await {
                    warn!("Significant changes in pool {}: {:?}", pool, changes);
                }
            }
            
            // Record execution time
            let duration = start.elapsed();
            histogram!(METRIC_EXECUTION_TIME, duration.as_secs_f64());
            
            // Wait for next update
            tokio::time::sleep(self.update_frequency).await;
        }
    }
    
    async fn update_pool_state(&self, pool: Address) -> Result<()> {
        let mut states = self.pool_states.write().await;
        
        // Update pool state
        let state = states.entry(pool).or_insert(PoolState {
            reserve0: U256::zero(),
            reserve1: U256::zero(),
            last_update: Instant::now(),
            update_count: 0,
        });
        
        state.update_count += 1;
        state.last_update = Instant::now();
        
        Ok(())
    }
    
    async fn check_significant_changes(&self, pool: Address) -> Option<Vec<String>> {
        let states = self.pool_states.read().await;
        let state = states.get(&pool)?;
        
        let mut changes = Vec::new();
        
        // Check reserve changes
        if let Some(thresholds) = self.price_thresholds.get(&pool) {
            let current_price = calculate_price(state.reserve0, state.reserve1);
            
            if current_price < thresholds.0 {
                changes.push(format!("Price below minimum threshold"));
            }
            if current_price > thresholds.1 {
                changes.push(format!("Price above maximum threshold"));
            }
        }
        
        if changes.is_empty() {
            None
        } else {
            Some(changes)
        }
    }
}

fn calculate_price(reserve0: U256, reserve1: U256) -> U256 {
    if reserve0.is_zero() {
        return U256::zero();
    }
    
    reserve1.checked_mul(U256::from(1e18 as u64))
        .and_then(|r| r.checked_div(reserve0))
        .unwrap_or(U256::zero())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_pool_monitoring() {
        let monitor = StateMonitor::new(Duration::from_secs(1));
        let pool = Address::random();
        
        // Test pool state updates
        monitor.update_pool_state(pool).await.unwrap();
        
        let states = monitor.pool_states.read().await;
        let state = states.get(&pool).unwrap();
        
        assert_eq!(state.update_count, 1);
    }
    
    #[tokio::test]
    async fn test_significant_changes() {
        let monitor = StateMonitor::new(Duration::from_secs(1));
        let pool = Address::random();
        
        // Add price thresholds
        monitor.price_thresholds.insert(
            pool,
            (U256::from(900), U256::from(1100)) // 10% threshold
        );
        
        // Test price changes
        monitor.update_pool_state(pool).await.unwrap();
        
        let changes = monitor.check_significant_changes(pool).await;
        assert!(changes.is_some());
    }
}
