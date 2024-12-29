use anyhow::Result;
use rust::core::{FlashloanManager, FlashloanParams, FlashloanProvider};
use ethers::types::{Address, U256};
use test_log::test;

#[test]
async fn test_flashloan_validation() -> Result<()> {
    let manager = FlashloanManager::new();
    
    // Test with zero amount (should fail)
    let params = FlashloanParams {
        provider: FlashloanProvider::AAVE,
        token: Address::random(),
        amount: U256::zero(),
        data: vec![],
        callback: Address::random(),
    };
    assert!(manager.validate_params(&params).is_err());
    
    // Test with valid amount (should pass)
    let params = FlashloanParams {
        provider: FlashloanProvider::AAVE,
        token: Address::random(),
        amount: U256::from(1000000),
        data: vec![],
        callback: Address::random(),
    };
    assert!(manager.validate_params(&params).is_ok());
    
    Ok(())
}

#[test]
async fn test_fee_calculations() -> Result<()> {
    let manager = FlashloanManager::new();
    
    // Test AAVE fee (0.09%)
    let params = FlashloanParams {
        provider: FlashloanProvider::AAVE,
        token: Address::random(),
        amount: U256::from(1000000),
        data: vec![],
        callback: Address::random(),
    };
    let fee = manager.calculate_fee(&params)?;
    assert_eq!(fee, U256::from(900)); // 0.09% of 1000000
    
    // Test Balancer fee (should be different)
    let params = FlashloanParams {
        provider: FlashloanProvider::Balancer,
        token: Address::random(),
        amount: U256::from(1000000),
        data: vec![],
        callback: Address::random(),
    };
    let fee = manager.calculate_fee(&params)?;
    assert!(fee > U256::zero());
    
    Ok(())
}

#[test]
async fn test_profitability_check() -> Result<()> {
    let manager = FlashloanManager::new();
    
    // Test unprofitable trade
    let amount = U256::from(1000);
    let fee = U256::from(100);
    assert!(!manager.is_profitable_after_fees(amount, fee));
    
    // Test profitable trade
    let amount = U256::from(1000000);
    let fee = U256::from(900);
    assert!(manager.is_profitable_after_fees(amount, fee));
    
    Ok(())
}
