use anyhow::Result;
use rust::metrics::StateMonitor;
use ethers::types::{Address, U256};
use std::time::Duration;
use test_log::test;

#[test]
async fn test_pool_state_updates() -> Result<()> {
    let monitor = StateMonitor::new(Duration::from_secs(1));
    let pool = Address::random();
    
    // Test initial state
    monitor.update_pool_state(pool).await?;
    let states = monitor.pool_states.read().await;
    let state = states.get(&pool).unwrap();
    assert_eq!(state.update_count, 1);
    
    // Test multiple updates
    monitor.update_pool_state(pool).await?;
    let states = monitor.pool_states.read().await;
    let state = states.get(&pool).unwrap();
    assert_eq!(state.update_count, 2);
    
    Ok(())
}

#[test]
async fn test_price_thresholds() -> Result<()> {
    let monitor = StateMonitor::new(Duration::from_secs(1));
    let pool = Address::random();
    
    // Add price thresholds (±10% from base price)
    monitor.price_thresholds.insert(
        pool,
        (U256::from(900), U256::from(1100))
    );
    
    // Test price within threshold
    let changes = monitor.check_significant_changes(pool).await;
    assert!(changes.is_none());
    
    // Test price outside threshold
    // This would require mocking the price calculation
    // or setting up a test pool with specific reserves
    
    Ok(())
}

#[test]
async fn test_monitoring_multiple_pools() -> Result<()> {
    let monitor = StateMonitor::new(Duration::from_secs(1));
    let pools = vec![Address::random(), Address::random(), Address::random()];
    
    // Update all pools
    for pool in &pools {
        monitor.update_pool_state(*pool).await?;
    }
    
    // Verify all pools were updated
    let states = monitor.pool_states.read().await;
    assert_eq!(states.len(), pools.len());
    
    for pool in &pools {
        let state = states.get(pool).unwrap();
        assert_eq!(state.update_count, 1);
    }
    
    Ok(())
}
