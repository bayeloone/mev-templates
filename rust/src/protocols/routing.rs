use super::aave::AaveProtocol;
use ethers::prelude::*;
use ethers::types::{Address, U256};
use futures::future::join_all;
use std::sync::Arc;
use anyhow::Result;
use std::collections::HashMap;
use tokio::time::{timeout, Duration};
use serde::{Serialize, Deserialize};

const TIMEOUT_DURATION: u64 = 5; // 5 seconds timeout for RPC calls

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub rpc_url: String,
    pub bridge_address: Option<Address>,
    pub gas_token: Address,
    pub stable_tokens: Vec<Address>,
    pub native_wrapped: Address,
}

#[derive(Debug, Clone)]
pub struct RateInfo {
    pub chain_id: u64,
    pub asset: Address,
    pub supply_apy: f64,
    pub borrow_apy: f64,
    pub liquidity: U256,
    pub utilization: f64,
    pub gas_token_price: f64,
    pub estimated_gas_cost: U256,
}

#[derive(Debug)]
pub struct CrossChainRoute {
    pub source_chain: u64,
    pub target_chain: u64,
    pub asset: Address,
    pub amount: U256,
    pub estimated_profit: U256,
    pub steps: Vec<RouteStep>,
}

#[derive(Debug)]
pub enum RouteStep {
    Bridge {
        from_chain: u64,
        to_chain: u64,
        bridge_address: Address,
        asset: Address,
        amount: U256,
    },
    Supply {
        chain_id: u64,
        asset: Address,
        amount: U256,
        apy: f64,
    },
    Borrow {
        chain_id: u64,
        asset: Address,
        amount: U256,
        apy: f64,
    },
    Swap {
        chain_id: u64,
        asset_in: Address,
        asset_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    },
}

pub struct MultiChainRouter<M: Middleware> {
    chains: HashMap<u64, ChainConfig>,
    providers: HashMap<u64, Arc<M>>,
    aave_pools: HashMap<u64, Arc<AaveProtocol<M>>>,
}

impl<M: Middleware + 'static> MultiChainRouter<M> {
    pub fn new(chains: Vec<ChainConfig>, providers: HashMap<u64, Arc<M>>) -> Result<Self> {
        let mut aave_pools = HashMap::new();
        
        for (chain_id, provider) in providers.iter() {
            let aave = Arc::new(AaveProtocol::new(*chain_id, provider.clone())?);
            aave_pools.insert(*chain_id, aave);
        }

        Ok(Self {
            chains: chains.into_iter().map(|c| (c.chain_id, c)).collect(),
            providers,
            aave_pools,
        })
    }

    pub async fn find_best_rates(&self, 
        asset: Address,
        amount: U256,
        source_chain: u64,
    ) -> Result<Vec<RateInfo>> {
        let mut rates = Vec::new();
        let mut futures = Vec::new();

        // Query rates on all chains in parallel
        for (chain_id, aave) in self.aave_pools.iter() {
            let asset = asset;
            let amount = amount;
            
            futures.push(async move {
                let result = timeout(
                    Duration::from_secs(TIMEOUT_DURATION),
                    self.get_chain_rates(*chain_id, asset, amount)
                ).await;
                
                match result {
                    Ok(Ok(rate)) => Some(rate),
                    _ => None,
                }
            });
        }

        // Collect results
        let results = join_all(futures).await;
        for result in results.into_iter().flatten() {
            rates.push(result);
        }

        // Sort by supply APY descending
        rates.sort_by(|a, b| b.supply_apy.partial_cmp(&a.supply_apy).unwrap());
        
        Ok(rates)
    }

    async fn get_chain_rates(&self, 
        chain_id: u64,
        asset: Address,
        amount: U256,
    ) -> Result<RateInfo> {
        let aave = self.aave_pools.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Chain not supported"))?;

        let reserve_data = aave.get_reserve_data(asset).await?;
        let asset_price = aave.get_asset_price(asset).await?;
        
        // Calculate APYs
        let supply_apy = self.calculate_apy(reserve_data.current_liquidity_rate)?;
        let borrow_apy = self.calculate_apy(reserve_data.current_variable_borrow_rate)?;
        
        // Get gas token price
        let chain_config = self.chains.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Chain config not found"))?;
        let gas_price = aave.get_asset_price(chain_config.gas_token).await?;
        let gas_token_price = ethers::utils::format_units(gas_price, "ether")
            .parse::<f64>()?;

        // Estimate gas costs for common operations
        let estimated_gas = self.estimate_gas_cost(chain_id).await?;

        // Calculate utilization rate
        let total_supply = reserve_data.liquidity_index;
        let total_borrow = reserve_data.variable_borrow_index;
        let utilization = if !total_supply.is_zero() {
            total_borrow.as_u128() as f64 / total_supply.as_u128() as f64
        } else {
            0.0
        };

        Ok(RateInfo {
            chain_id,
            asset,
            supply_apy,
            borrow_apy,
            liquidity: total_supply,
            utilization,
            gas_token_price,
            estimated_gas_cost: estimated_gas,
        })
    }

    pub async fn find_arbitrage_routes(&self,
        asset: Address,
        amount: U256,
        source_chain: u64,
        min_profit: U256,
    ) -> Result<Vec<CrossChainRoute>> {
        let mut routes = Vec::new();
        let rates = self.find_best_rates(asset, amount, source_chain).await?;

        // Find profitable routes between chains
        for source_rate in &rates {
            for target_rate in &rates {
                if source_rate.chain_id == target_rate.chain_id {
                    continue;
                }

                let profit = self.calculate_route_profit(
                    source_rate,
                    target_rate,
                    amount
                )?;

                if profit > min_profit {
                    let route = self.build_route(
                        source_rate,
                        target_rate,
                        asset,
                        amount,
                        profit
                    )?;
                    routes.push(route);
                }
            }
        }

        // Sort routes by profit
        routes.sort_by(|a, b| b.estimated_profit.cmp(&a.estimated_profit));
        
        Ok(routes)
    }

    pub async fn execute_route(&self, route: CrossChainRoute) -> Result<Vec<TransactionReceipt>> {
        let mut receipts = Vec::new();

        for step in route.steps {
            match step {
                RouteStep::Bridge { from_chain, to_chain, bridge_address, asset, amount } => {
                    let aave = self.aave_pools.get(&from_chain)
                        .ok_or_else(|| anyhow::anyhow!("Source chain not supported"))?;
                    
                    // Execute bridge transaction
                    // Implementation depends on specific bridge protocol
                }
                
                RouteStep::Supply { chain_id, asset, amount, apy: _ } => {
                    let aave = self.aave_pools.get(&chain_id)
                        .ok_or_else(|| anyhow::anyhow!("Chain not supported"))?;
                    let receipt = aave.supply(asset, amount, aave.get_pool_address(), 0).await?;
                    receipts.push(receipt);
                }
                
                RouteStep::Borrow { chain_id, asset, amount, apy: _ } => {
                    let aave = self.aave_pools.get(&chain_id)
                        .ok_or_else(|| anyhow::anyhow!("Chain not supported"))?;
                    let receipt = aave.borrow(
                        asset,
                        amount,
                        2, // Variable rate
                        0,
                        aave.get_pool_address()
                    ).await?;
                    receipts.push(receipt);
                }
                
                RouteStep::Swap { chain_id, asset_in, asset_out, amount_in, min_amount_out } => {
                    // Execute swap using DEX aggregator
                    // Implementation depends on specific DEX
                }
            }
        }

        Ok(receipts)
    }

    // Helper functions
    fn calculate_apy(&self, rate: U256) -> Result<f64> {
        let ray = U256::from(10).pow(U256::from(27));
        let rate_f64 = rate.as_u128() as f64 / ray.as_u128() as f64;
        Ok(((1.0 + rate_f64 / 31536000.0).powf(31536000.0) - 1.0) * 100.0)
    }

    async fn estimate_gas_cost(&self, chain_id: u64) -> Result<U256> {
        let provider = self.providers.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;
            
        let gas_price = provider.get_gas_price().await?;
        
        // Estimate gas units for common operations
        let supply_gas = U256::from(200_000);
        let borrow_gas = U256::from(300_000);
        let bridge_gas = U256::from(500_000);
        
        Ok(gas_price.checked_mul(supply_gas + borrow_gas + bridge_gas)
            .ok_or_else(|| anyhow::anyhow!("Gas calculation overflow"))?)
    }

    fn calculate_route_profit(
        &self,
        source_rate: &RateInfo,
        target_rate: &RateInfo,
        amount: U256,
    ) -> Result<U256> {
        let source_yield = (source_rate.supply_apy / 100.0) * amount.as_u128() as f64;
        let target_yield = (target_rate.supply_apy / 100.0) * amount.as_u128() as f64;
        
        let bridge_cost = source_rate.estimated_gas_cost
            .checked_add(target_rate.estimated_gas_cost)
            .ok_or_else(|| anyhow::anyhow!("Gas cost overflow"))?;

        let total_yield = target_yield - source_yield;
        let profit = total_yield - (bridge_cost.as_u128() as f64 * source_rate.gas_token_price);
        
        Ok(U256::from((profit.max(0.0)) as u128))
    }

    fn build_route(
        &self,
        source_rate: &RateInfo,
        target_rate: &RateInfo,
        asset: Address,
        amount: U256,
        profit: U256,
    ) -> Result<CrossChainRoute> {
        let mut steps = Vec::new();
        
        // 1. Bridge step
        if let Some(bridge_address) = self.chains.get(&source_rate.chain_id)
            .and_then(|c| c.bridge_address) {
            steps.push(RouteStep::Bridge {
                from_chain: source_rate.chain_id,
                to_chain: target_rate.chain_id,
                bridge_address,
                asset,
                amount,
            });
        }
        
        // 2. Supply on target chain
        steps.push(RouteStep::Supply {
            chain_id: target_rate.chain_id,
            asset,
            amount,
            apy: target_rate.supply_apy,
        });

        Ok(CrossChainRoute {
            source_chain: source_rate.chain_id,
            target_chain: target_rate.chain_id,
            asset,
            amount,
            estimated_profit: profit,
            steps,
        })
    }
}
