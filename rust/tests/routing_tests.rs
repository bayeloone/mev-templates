use anyhow::Result;
use rust::{
    routing::PathFinder,
    pools::Pool,
};
use ethers::types::{Address, U256};
use test_log::test;

fn create_test_pools() -> Vec<Pool> {
    // Create a simple path: TokenA -> TokenB -> TokenC -> TokenA
    let token_a = Address::random();
    let token_b = Address::random();
    let token_c = Address::random();
    
    vec![
        Pool {
            address: Address::random(),
            token0: token_a,
            token1: token_b,
            // Add other required fields
        },
        Pool {
            address: Address::random(),
            token0: token_b,
            token1: token_c,
            // Add other required fields
        },
        Pool {
            address: Address::random(),
            token0: token_c,
            token1: token_a,
            // Add other required fields
        },
    ]
}

#[test]
async fn test_path_finding_basic() -> Result<()> {
    let mut finder = PathFinder::new();
    let pools = create_test_pools();
    let token = pools[0].token0;
    let amount = U256::from(1000000);
    
    let paths = finder.find_profitable_paths(token, amount, &pools).await?;
    
    // Should find at least one path
    assert!(!paths.is_empty());
    
    // Check path properties
    let path = &paths[0];
    assert!(!path.pools.is_empty());
    assert_eq!(path.tokens[0], token);
    assert_eq!(path.tokens.last(), Some(&token));
    
    Ok(())
}

#[test]
async fn test_path_validation() -> Result<()> {
    let mut finder = PathFinder::new();
    let pools = create_test_pools();
    let token = pools[0].token0;
    
    // Test with zero amount (should return no paths)
    let paths = finder.find_profitable_paths(token, U256::zero(), &pools).await?;
    assert!(paths.is_empty());
    
    // Test with small amount (might not be profitable)
    let paths = finder.find_profitable_paths(token, U256::from(1), &pools).await?;
    assert!(paths.is_empty());
    
    // Test with reasonable amount
    let paths = finder.find_profitable_paths(token, U256::from(1000000), &pools).await?;
    assert!(!paths.is_empty());
    
    Ok(())
}

#[test]
async fn test_gas_estimation() -> Result<()> {
    let finder = PathFinder::new();
    
    // Test single hop
    let tokens = vec![Address::random(), Address::random()];
    let gas = finder.estimate_gas_cost(&tokens)?;
    assert_eq!(gas, U256::from(121000)); // 21000 base + 100000 per hop
    
    // Test multi hop
    let tokens = vec![
        Address::random(),
        Address::random(),
        Address::random(),
        Address::random(),
    ];
    let gas = finder.estimate_gas_cost(&tokens)?;
    assert_eq!(gas, U256::from(321000)); // 21000 base + 3 * 100000
    
    Ok(())
}

#[test]
async fn test_path_profitability() -> Result<()> {
    let mut finder = PathFinder::new();
    let pools = create_test_pools();
    let token = pools[0].token0;
    let amount = U256::from(1000000);
    
    let paths = finder.find_profitable_paths(token, amount, &pools).await?;
    
    for path in paths {
        // Verify each path is profitable
        assert!(path.expected_profit > path.gas_estimate);
        
        // Verify impact score is acceptable
        assert!(path.impact_score <= 300); // 3% max impact
    }
    
    Ok(())
}
