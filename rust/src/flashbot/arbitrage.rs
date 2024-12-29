use anyhow::{Result, anyhow};
use ethers::{
    types::{Address, U256, Transaction},
    providers::{Provider, Http},
    middleware::SignerMiddleware,
    signers::LocalWallet,
};
use std::{sync::Arc, collections::HashMap};
use tokio::sync::RwLock;
use crate::flashbot::types::*;
use crate::dex::{DexPool, DexManager};
use crate::security::SecurityManager;

pub struct ArbitrageManager {
    dex_manager: Arc<DexManager>,
    security_manager: Arc<SecurityManager>,
    flash_sources: Arc<RwLock<Vec<FlashLoanSource>>>,
    risk_config: Arc<RwLock<RiskConfig>>,
    execution_config: Arc<RwLock<ExecutionConfig>>,
    analytics: Arc<RwLock<Analytics>>,
}

impl ArbitrageManager {
    pub fn new(
        dex_manager: Arc<DexManager>,
        security_manager: Arc<SecurityManager>,
        risk_config: RiskConfig,
        execution_config: ExecutionConfig,
    ) -> Self {
        Self {
            dex_manager,
            security_manager,
            flash_sources: Arc::new(RwLock::new(Vec::new())),
            risk_config: Arc::new(RwLock::new(risk_config)),
            execution_config: Arc::new(RwLock::new(execution_config)),
            analytics: Arc::new(RwLock::new(Analytics::default())),
        }
    }

    /// Find arbitrage opportunities across DEXes
    pub async fn find_opportunities(&self, token: Address) -> Result<Vec<ArbitrageOpportunity>> {
        // Get all relevant pools
        let pools = self.dex_manager.get_pools_for_token(token).await?;
        
        // Group pools by protocol
        let mut opportunities = Vec::new();
        
        // Check V2 style pools
        self.find_v2_opportunities(&pools, &mut opportunities).await?;
        
        // Check V3 pools
        self.find_v3_opportunities(&pools, &mut opportunities).await?;
        
        // Check Curve pools
        self.find_curve_opportunities(&pools, &mut opportunities).await?;
        
        // Filter and validate opportunities
        let valid_ops = self.validate_opportunities(opportunities).await?;
        
        Ok(valid_ops)
    }

    /// Find arbitrage in Uniswap V2 style pools
    async fn find_v2_opportunities(
        &self,
        pools: &[DexPool],
        opportunities: &mut Vec<ArbitrageOpportunity>
    ) -> Result<()> {
        let v2_pools: Vec<_> = pools.iter()
            .filter(|p| matches!(p.protocol, DexProtocol::UniswapV2))
            .collect();
            
        for i in 0..v2_pools.len() {
            for j in i+1..v2_pools.len() {
                let pool1 = &v2_pools[i];
                let pool2 = &v2_pools[j];
                
                // Check if pools share tokens
                if !self.pools_share_tokens(pool1, pool2) {
                    continue;
                }
                
                // Calculate optimal amount and profit
                if let Some((amount, profit)) = self.calculate_v2_arbitrage(pool1, pool2).await? {
                    if self.is_profitable(profit).await? {
                        opportunities.push(ArbitrageOpportunity {
                            path: vec![pool1.token0, pool1.token1],
                            expected_profit: profit,
                            required_flash_amount: amount,
                            risk_score: self.calculate_risk_score(pool1, pool2).await?,
                            gas_cost: self.estimate_gas_cost(pool1, pool2).await?,
                            execution_time_ms: 1000, // Estimated 1s execution
                            pools: vec![pool1.clone(), pool2.clone()],
                            profit_token: pool1.token0,
                        });
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Find arbitrage in Uniswap V3 pools
    async fn find_v3_opportunities(
        &self,
        pools: &[DexPool],
        opportunities: &mut Vec<ArbitrageOpportunity>
    ) -> Result<()> {
        let v3_pools: Vec<_> = pools.iter()
            .filter(|p| matches!(p.protocol, DexProtocol::UniswapV3))
            .collect();
            
        for i in 0..v3_pools.len() {
            for j in i+1..v3_pools.len() {
                let pool1 = &v3_pools[i];
                let pool2 = &v3_pools[j];
                
                // Check if pools share tokens and have enough liquidity
                if !self.validate_v3_pools(pool1, pool2).await? {
                    continue;
                }
                
                // Calculate optimal amount and profit considering concentrated liquidity
                if let Some((amount, profit)) = self.calculate_v3_arbitrage(pool1, pool2).await? {
                    if self.is_profitable(profit).await? {
                        opportunities.push(ArbitrageOpportunity {
                            path: vec![pool1.token0, pool1.token1],
                            expected_profit: profit,
                            required_flash_amount: amount,
                            risk_score: self.calculate_risk_score(pool1, pool2).await?,
                            gas_cost: self.estimate_gas_cost(pool1, pool2).await?,
                            execution_time_ms: 1000,
                            pools: vec![pool1.clone(), pool2.clone()],
                            profit_token: pool1.token0,
                        });
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Execute arbitrage opportunity
    pub async fn execute_arbitrage(
        &self,
        opportunity: &ArbitrageOpportunity,
        wallet: LocalWallet,
    ) -> Result<TradeResult> {
        // Final validation before execution
        self.validate_execution(opportunity).await?;
        
        // Prepare flash loan
        let flash_params = self.prepare_flash_loan(opportunity).await?;
        
        // Build transaction
        let tx = self.build_arbitrage_transaction(opportunity, flash_params).await?;
        
        // Execute with MEV protection
        let result = self.execute_with_protection(tx, wallet).await;
        
        // Record result
        self.record_trade_result(opportunity, &result).await?;
        
        Ok(result)
    }

    /// Calculate risk score for pools
    async fn calculate_risk_score(&self, pool1: &DexPool, pool2: &DexPool) -> Result<u8> {
        let mut score = 0u8;
        
        // Check pool liquidity (0-25 points)
        let min_liquidity = min(pool1.liquidity, pool2.liquidity);
        score += self.score_liquidity(min_liquidity);
        
        // Check price impact (0-25 points)
        let impact = self.calculate_price_impact(pool1, pool2).await?;
        score += self.score_price_impact(impact);
        
        // Check pool age and reliability (0-25 points)
        score += self.score_pool_reliability(pool1, pool2).await?;
        
        // Check token security (0-25 points)
        score += self.score_token_security(&[pool1.token0, pool1.token1]).await?;
        
        Ok(score)
    }

    /// Validate if opportunity is still profitable
    async fn validate_execution(&self, op: &ArbitrageOpportunity) -> Result<()> {
        // Check if pools still have sufficient liquidity
        for pool in &op.pools {
            let current_liquidity = self.dex_manager.get_pool_liquidity(&pool.address).await?;
            if current_liquidity < pool.liquidity.saturating_mul(95) / 100 {
                return Err(anyhow!("Pool liquidity decreased"));
            }
        }
        
        // Verify price hasn't moved significantly
        let current_profit = self.simulate_arbitrage(op).await?;
        if current_profit < op.expected_profit.saturating_mul(90) / 100 {
            return Err(anyhow!("Profit decreased significantly"));
        }
        
        // Check gas price is still acceptable
        let gas_price = self.get_current_gas_price().await?;
        let config = self.execution_config.read().await;
        if gas_price > config.max_gas_price {
            return Err(anyhow!("Gas price too high"));
        }
        
        Ok(())
    }

    /// Record trade result and update analytics
    async fn record_trade_result(
        &self,
        opportunity: &ArbitrageOpportunity,
        result: &TradeResult,
    ) -> Result<()> {
        let mut analytics = self.analytics.write().await;
        
        // Update metrics
        if result.success {
            analytics.successful_trades += 1;
            analytics.total_profit = analytics.total_profit.saturating_add(result.actual_profit);
        } else {
            analytics.failed_trades += 1;
            if let Some(ref error) = result.error {
                analytics.errors.push(error.clone());
            }
        }
        
        // Update averages
        analytics.avg_profit_per_trade = analytics.total_profit
            .checked_div(U256::from(analytics.successful_trades))
            .unwrap_or_default();
            
        analytics.avg_execution_time = Duration::from_micros(
            (analytics.avg_execution_time.as_micros() as u64 + result.execution_time.as_micros() as u64) / 2
        );
        
        // Update gas stats
        analytics.gas_spent = analytics.gas_spent.saturating_add(result.gas_used);
        
        // Add to history
        analytics.trade_history.push(result.clone());
        
        // Trim history if too long
        if analytics.trade_history.len() > 1000 {
            analytics.trade_history.remove(0);
        }
        
        Ok(())
    }
}
