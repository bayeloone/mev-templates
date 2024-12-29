use crate::protocols::aave::AaveProtocol;
use crate::protocols::routing::MultiChainRouter;
use crate::protocols::stargate::{StargateProtocol, StargateConfig, get_pool_config, is_supported_chain, is_supported_token};
use ethers::prelude::*;
use ethers::types::{Address, U256, Bytes};
use std::sync::Arc;
use anyhow::Result;
use super::types::*;
use tokio::time::{timeout, Duration};
use std::collections::HashMap;

const EXECUTION_TIMEOUT: u64 = 180; // 3 minutes timeout for full execution

pub struct CrossChainFlashloan<M: Middleware> {
    router: Arc<MultiChainRouter<M>>,
    aave_pools: HashMap<u64, Arc<AaveProtocol<M>>>,
    providers: HashMap<u64, Arc<M>>,
    stargate_protocols: HashMap<u64, Arc<StargateProtocol<M>>>,
}

impl<M: Middleware + 'static> CrossChainFlashloan<M> {
    pub fn new(
        router: Arc<MultiChainRouter<M>>,
        aave_pools: HashMap<u64, Arc<AaveProtocol<M>>>,
        providers: HashMap<u64, Arc<M>>,
        stargate_protocols: HashMap<u64, Arc<StargateProtocol<M>>>,
    ) -> Self {
        Self {
            router,
            aave_pools,
            providers,
            stargate_protocols,
        }
    }

    pub async fn execute_strategy(
        &self,
        strategy: FlashloanStrategy,
    ) -> Result<ExecutionResult> {
        // Validate strategy
        self.validate_strategy(&strategy)?;

        // Set timeout for full execution
        let result = timeout(
            Duration::from_secs(EXECUTION_TIMEOUT),
            self.execute_steps(strategy.clone())
        ).await??;

        Ok(result)
    }

    async fn execute_steps(
        &self,
        strategy: FlashloanStrategy,
    ) -> Result<ExecutionResult> {
        let mut completed_steps = Vec::new();
        let mut total_gas_used = U256::zero();
        let mut current_profit = U256::zero();

        for step in strategy.execution_steps {
            match step {
                ExecutionStep::FlashLoan { chain_id, token, amount, params } => {
                    let result = self.execute_flashloan(chain_id, token, amount, params).await;
                    self.handle_step_result("FlashLoan", chain_id, result, &mut completed_steps)?;
                }

                ExecutionStep::Bridge { from_chain, to_chain, token, amount, bridge_data } => {
                    let result = self.execute_bridge(from_chain, to_chain, token, amount, bridge_data).await;
                    self.handle_step_result("Bridge", from_chain, result, &mut completed_steps)?;
                }

                ExecutionStep::Swap { chain_id, token_in, token_out, amount_in, min_amount_out, dex } => {
                    let result = self.execute_swap(chain_id, token_in, token_out, amount_in, min_amount_out, dex).await;
                    self.handle_step_result("Swap", chain_id, result, &mut completed_steps)?;
                }

                ExecutionStep::AaveSupply { chain_id, token, amount } => {
                    let result = self.execute_aave_supply(chain_id, token, amount).await;
                    self.handle_step_result("AaveSupply", chain_id, result, &mut completed_steps)?;
                }

                ExecutionStep::AaveBorrow { chain_id, token, amount, interest_rate_mode } => {
                    let result = self.execute_aave_borrow(chain_id, token, amount, interest_rate_mode).await;
                    self.handle_step_result("AaveBorrow", chain_id, result, &mut completed_steps)?;
                }

                ExecutionStep::AaveRepay { chain_id, token, amount, interest_rate_mode } => {
                    let result = self.execute_aave_repay(chain_id, token, amount, interest_rate_mode).await;
                    self.handle_step_result("AaveRepay", chain_id, result, &mut completed_steps)?;
                }
            }
        }

        Ok(ExecutionResult {
            success: completed_steps.iter().all(|s| s.success),
            profit: current_profit,
            gas_used: total_gas_used,
            error: None,
            steps_completed: completed_steps,
        })
    }

    async fn execute_flashloan(
        &self,
        chain_id: u64,
        token: Address,
        amount: U256,
        params: Bytes,
    ) -> Result<TransactionReceipt> {
        let aave = self.aave_pools.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Aave pool not found for chain {}", chain_id))?;

        aave.execute_flashloan(
            aave.get_pool_address(),
            vec![token],
            vec![amount],
            vec![0], // Variable rate mode
            params,
        ).await
    }

    async fn execute_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        match bridge_data.protocol {
            BridgeProtocol::Stargate => {
                self.execute_stargate_bridge(from_chain, to_chain, token, amount, bridge_data).await
            }
            BridgeProtocol::Hop => {
                self.execute_hop_bridge(from_chain, to_chain, token, amount, bridge_data).await
            }
            BridgeProtocol::CCTP => {
                self.execute_cctp_bridge(from_chain, to_chain, token, amount, bridge_data).await
            }
            BridgeProtocol::LayerZero => {
                self.execute_layerzero_bridge(from_chain, to_chain, token, amount, bridge_data).await
            }
            BridgeProtocol::Across => {
                self.execute_across_bridge(from_chain, to_chain, token, amount, bridge_data).await
            }
        }
    }

    async fn execute_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
        dex: DexProtocol,
    ) -> Result<TransactionReceipt> {
        match dex {
            DexProtocol::UniswapV2 => {
                self.execute_uniswap_v2_swap(chain_id, token_in, token_out, amount_in, min_amount_out).await
            }
            DexProtocol::UniswapV3 => {
                self.execute_uniswap_v3_swap(chain_id, token_in, token_out, amount_in, min_amount_out).await
            }
            DexProtocol::Curve => {
                self.execute_curve_swap(chain_id, token_in, token_out, amount_in, min_amount_out).await
            }
            DexProtocol::Balancer => {
                self.execute_balancer_swap(chain_id, token_in, token_out, amount_in, min_amount_out).await
            }
            DexProtocol::OneInch => {
                self.execute_1inch_swap(chain_id, token_in, token_out, amount_in, min_amount_out).await
            }
        }
    }

    async fn execute_aave_supply(
        &self,
        chain_id: u64,
        token: Address,
        amount: U256,
    ) -> Result<TransactionReceipt> {
        let aave = self.aave_pools.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Aave pool not found for chain {}", chain_id))?;

        aave.supply(token, amount, aave.get_pool_address(), 0).await
    }

    async fn execute_aave_borrow(
        &self,
        chain_id: u64,
        token: Address,
        amount: U256,
        interest_rate_mode: u8,
    ) -> Result<TransactionReceipt> {
        let aave = self.aave_pools.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Aave pool not found for chain {}", chain_id))?;

        aave.borrow(token, amount, interest_rate_mode, 0, aave.get_pool_address()).await
    }

    async fn execute_aave_repay(
        &self,
        chain_id: u64,
        token: Address,
        amount: U256,
        interest_rate_mode: u8,
    ) -> Result<TransactionReceipt> {
        let aave = self.aave_pools.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Aave pool not found for chain {}", chain_id))?;

        aave.repay(token, amount, interest_rate_mode, aave.get_pool_address()).await
    }

    // Bridge protocol implementations
    async fn execute_stargate_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        // Get Stargate protocol for source chain
        let stargate = self.stargate_protocols.get(&from_chain)
            .ok_or_else(|| anyhow::anyhow!("Stargate protocol not found for chain {}", from_chain))?;

        // Get destination wallet address
        let dst_wallet = self.providers.get(&to_chain)
            .ok_or_else(|| anyhow::anyhow!("Provider not found for chain {}", to_chain))?
            .default_sender()
            .ok_or_else(|| anyhow::anyhow!("No wallet address found for chain {}", to_chain))?;

        // Get pool IDs for the token on both chains
        let (src_pool_id, dst_pool_id) = self.get_stargate_pool_ids(from_chain, to_chain, token)?;

        // Calculate minimum amount based on slippage
        let min_amount = amount.saturating_sub(
            amount.saturating_mul(U256::from(bridge_data.slippage * 100.0 as u64))
                .saturating_div(U256::from(10000))
        );

        // Execute bridge transaction
        let receipt = stargate.bridge_token(
            to_chain as u16,
            src_pool_id,
            dst_pool_id,
            amount,
            min_amount,
            dst_wallet,
            vec![], // No additional payload needed
        ).await?;

        Ok(receipt)
    }

    fn get_stargate_pool_ids(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
    ) -> Result<(U256, U256)> {
        // Verify chain support
        if !is_supported_chain(from_chain) {
            return Err(anyhow::anyhow!("Source chain {} not supported by Stargate", from_chain));
        }
        if !is_supported_chain(to_chain) {
            return Err(anyhow::anyhow!("Destination chain {} not supported by Stargate", to_chain));
        }

        // Verify token support on both chains
        if !is_supported_token(from_chain, token) {
            return Err(anyhow::anyhow!("Token {:?} not supported on source chain {}", token, from_chain));
        }
        if !is_supported_token(to_chain, token) {
            return Err(anyhow::anyhow!("Token {:?} not supported on destination chain {}", token, to_chain));
        }

        // Get pool configurations
        let src_pool = get_pool_config(from_chain, token)
            .ok_or_else(|| anyhow::anyhow!("Pool config not found for token {:?} on chain {}", token, from_chain))?;
        let dst_pool = get_pool_config(to_chain, token)
            .ok_or_else(|| anyhow::anyhow!("Pool config not found for token {:?} on chain {}", token, to_chain))?;

        // Verify pool versions are compatible
        if src_pool.version != dst_pool.version {
            return Err(anyhow::anyhow!(
                "Incompatible pool versions: source v{} != destination v{}", 
                src_pool.version, 
                dst_pool.version
            ));
        }

        Ok((src_pool.pool_id, dst_pool.pool_id))
    }

    async fn execute_hop_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        // Implement Hop bridge logic
        todo!("Implement Hop bridge")
    }

    async fn execute_cctp_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        // Implement CCTP bridge logic
        todo!("Implement CCTP bridge")
    }

    async fn execute_layerzero_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        // Implement LayerZero bridge logic
        todo!("Implement LayerZero bridge")
    }

    async fn execute_across_bridge(
        &self,
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    ) -> Result<TransactionReceipt> {
        // Implement Across bridge logic
        todo!("Implement Across bridge")
    }

    // DEX implementations
    async fn execute_uniswap_v2_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<TransactionReceipt> {
        // Implement Uniswap V2 swap
        todo!("Implement Uniswap V2 swap")
    }

    async fn execute_uniswap_v3_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<TransactionReceipt> {
        // Implement Uniswap V3 swap
        todo!("Implement Uniswap V3 swap")
    }

    async fn execute_curve_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<TransactionReceipt> {
        // Implement Curve swap
        todo!("Implement Curve swap")
    }

    async fn execute_balancer_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<TransactionReceipt> {
        // Implement Balancer swap
        todo!("Implement Balancer swap")
    }

    async fn execute_1inch_swap(
        &self,
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
    ) -> Result<TransactionReceipt> {
        // Implement 1inch swap
        todo!("Implement 1inch swap")
    }

    // Helper functions
    fn validate_strategy(&self, strategy: &FlashloanStrategy) -> Result<()> {
        // 1. Validate chains
        self.validate_chains(strategy)?;

        // 2. Validate token addresses
        self.validate_tokens(strategy)?;

        // 3. Validate amounts and profitability
        self.validate_amounts(strategy)?;

        // 4. Validate execution steps sequence
        self.validate_step_sequence(strategy)?;

        // 5. Validate bridge configurations
        self.validate_bridges(strategy)?;

        // 6. Validate DEX configurations
        self.validate_dexes(strategy)?;

        // 7. Validate gas requirements
        self.validate_gas_requirements(strategy)?;

        Ok(())
    }

    fn validate_chains(&self, strategy: &FlashloanStrategy) -> Result<()> {
        // Check if chains are supported
        if !self.aave_pools.contains_key(&strategy.source_chain) {
            return Err(anyhow::anyhow!(
                "Source chain {} not supported", 
                strategy.source_chain
            ));
        }
        if !self.aave_pools.contains_key(&strategy.target_chain) {
            return Err(anyhow::anyhow!(
                "Target chain {} not supported", 
                strategy.target_chain
            ));
        }

        // Verify chain connectivity (bridge availability)
        let has_bridge_step = strategy.execution_steps.iter().any(|step| {
            matches!(step, ExecutionStep::Bridge { .. })
        });
        
        if strategy.source_chain != strategy.target_chain && !has_bridge_step {
            return Err(anyhow::anyhow!(
                "Cross-chain strategy requires bridge steps between chains {} and {}", 
                strategy.source_chain, 
                strategy.target_chain
            ));
        }

        Ok(())
    }

    fn validate_tokens(&self, strategy: &FlashloanStrategy) -> Result<()> {
        // Check if flash token is supported on source chain
        let source_aave = self.aave_pools.get(&strategy.source_chain)
            .ok_or_else(|| anyhow::anyhow!("Source chain Aave pool not found"))?;

        if !source_aave.is_supported_asset(strategy.flash_token) {
            return Err(anyhow::anyhow!(
                "Flash token {:?} not supported on source chain {}", 
                strategy.flash_token, 
                strategy.source_chain
            ));
        }

        // Validate tokens in each step
        for step in &strategy.execution_steps {
            match step {
                ExecutionStep::FlashLoan { token, chain_id, .. } => {
                    let aave = self.aave_pools.get(chain_id)
                        .ok_or_else(|| anyhow::anyhow!("Aave pool not found"))?;
                    if !aave.is_supported_asset(*token) {
                        return Err(anyhow::anyhow!(
                            "Token {:?} not supported for flashloan on chain {}", 
                            token, 
                            chain_id
                        ));
                    }
                }
                ExecutionStep::Swap { chain_id, token_in, token_out, .. } => {
                    // Verify token pair is supported on the specified DEX
                    self.validate_token_pair(*chain_id, *token_in, *token_out)?;
                }
                ExecutionStep::AaveSupply { chain_id, token, .. } |
                ExecutionStep::AaveBorrow { chain_id, token, .. } |
                ExecutionStep::AaveRepay { chain_id, token, .. } => {
                    let aave = self.aave_pools.get(chain_id)
                        .ok_or_else(|| anyhow::anyhow!("Aave pool not found"))?;
                    if !aave.is_supported_asset(*token) {
                        return Err(anyhow::anyhow!(
                            "Token {:?} not supported on Aave chain {}", 
                            token, 
                            chain_id
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn validate_amounts(&self, strategy: &FlashloanStrategy) -> Result<()> {
        if strategy.flash_amount.is_zero() {
            return Err(anyhow::anyhow!("Flash amount cannot be zero"));
        }

        // Check if flash amount exceeds pool liquidity
        let source_aave = self.aave_pools.get(&strategy.source_chain)
            .ok_or_else(|| anyhow::anyhow!("Source chain Aave pool not found"))?;
        
        let reserve_data = source_aave.get_reserve_data(strategy.flash_token)
            .map_err(|_| anyhow::anyhow!("Failed to get reserve data"))?;

        if strategy.flash_amount >= reserve_data.liquidity_index {
            return Err(anyhow::anyhow!(
                "Flash amount exceeds available liquidity ({} > {})", 
                strategy.flash_amount, 
                reserve_data.liquidity_index
            ));
        }

        // Validate amounts in each step
        let mut balance_map: HashMap<(u64, Address), i128> = HashMap::new();
        
        for step in &strategy.execution_steps {
            match step {
                ExecutionStep::FlashLoan { chain_id, token, amount, .. } => {
                    *balance_map.entry((*chain_id, *token)).or_default() += amount.as_u128() as i128;
                }
                ExecutionStep::Bridge { from_chain, to_chain, token, amount, .. } => {
                    *balance_map.entry((*from_chain, *token)).or_default() -= amount.as_u128() as i128;
                    *balance_map.entry((*to_chain, *token)).or_default() += amount.as_u128() as i128;
                }
                ExecutionStep::Swap { chain_id, token_in, token_out, amount_in, min_amount_out, .. } => {
                    *balance_map.entry((*chain_id, *token_in)).or_default() -= amount_in.as_u128() as i128;
                    *balance_map.entry((*chain_id, *token_out)).or_default() += min_amount_out.as_u128() as i128;
                }
                ExecutionStep::AaveSupply { chain_id, token, amount, .. } => {
                    *balance_map.entry((*chain_id, *token)).or_default() -= amount.as_u128() as i128;
                }
                ExecutionStep::AaveBorrow { chain_id, token, amount, .. } => {
                    *balance_map.entry((*chain_id, *token)).or_default() += amount.as_u128() as i128;
                }
                ExecutionStep::AaveRepay { chain_id, token, amount, .. } => {
                    *balance_map.entry((*chain_id, *token)).or_default() -= amount.as_u128() as i128;
                }
            }
        }

        // Verify final balances
        for ((chain_id, token), balance) in balance_map {
            if balance < 0 {
                return Err(anyhow::anyhow!(
                    "Negative balance {} for token {:?} on chain {}", 
                    balance, 
                    token, 
                    chain_id
                ));
            }
        }

        Ok(())
    }

    fn validate_step_sequence(&self, strategy: &FlashloanStrategy) -> Result<()> {
        let mut current_chain = strategy.source_chain;
        let mut has_flashloan = false;
        let mut flashloan_repaid = false;
        let mut borrowed_amounts: HashMap<(u64, Address), U256> = HashMap::new();

        for step in &strategy.execution_steps {
            match step {
                ExecutionStep::FlashLoan { chain_id, .. } => {
                    if has_flashloan {
                        return Err(anyhow::anyhow!("Multiple flashloans not supported"));
                    }
                    if *chain_id != current_chain {
                        return Err(anyhow::anyhow!("Invalid chain sequence in flash loan step"));
                    }
                    has_flashloan = true;
                }
                ExecutionStep::Bridge { from_chain, to_chain, .. } => {
                    if *from_chain != current_chain {
                        return Err(anyhow::anyhow!("Invalid source chain in bridge step"));
                    }
                    current_chain = *to_chain;
                }
                ExecutionStep::AaveBorrow { chain_id, token, amount, .. } => {
                    if *chain_id != current_chain {
                        return Err(anyhow::anyhow!("Invalid chain sequence in borrow step"));
                    }
                    *borrowed_amounts.entry((*chain_id, *token)).or_default() += *amount;
                }
                ExecutionStep::AaveRepay { chain_id, token, amount, .. } => {
                    if *chain_id != current_chain {
                        return Err(anyhow::anyhow!("Invalid chain sequence in repay step"));
                    }
                    let borrowed = borrowed_amounts.get(&(*chain_id, *token))
                        .ok_or_else(|| anyhow::anyhow!("Repaying non-borrowed token"))?;
                    if amount > borrowed {
                        return Err(anyhow::anyhow!("Repay amount exceeds borrowed amount"));
                    }
                    if token == &strategy.flash_token && chain_id == &strategy.source_chain {
                        flashloan_repaid = true;
                    }
                }
                _ => {
                    // Validate other steps are on the current chain
                    let step_chain = match step {
                        ExecutionStep::Swap { chain_id, .. } => chain_id,
                        ExecutionStep::AaveSupply { chain_id, .. } => chain_id,
                        _ => continue,
                    };
                    if *step_chain != current_chain {
                        return Err(anyhow::anyhow!("Invalid chain sequence in step"));
                    }
                }
            }
        }

        // Verify flashloan was taken and repaid
        if !has_flashloan {
            return Err(anyhow::anyhow!("Strategy must include a flashloan"));
        }
        if !flashloan_repaid {
            return Err(anyhow::anyhow!("Flashloan must be repaid"));
        }

        Ok(())
    }

    fn validate_bridges(&self, strategy: &FlashloanStrategy) -> Result<()> {
        for step in &strategy.execution_steps {
            if let ExecutionStep::Bridge { from_chain, to_chain, bridge_data, .. } = step {
                // Verify bridge protocol supports the chain pair
                match bridge_data.protocol {
                    BridgeProtocol::Stargate => {
                        if !self.is_stargate_supported(*from_chain, *to_chain) {
                            return Err(anyhow::anyhow!(
                                "Stargate bridge not supported between chains {} and {}", 
                                from_chain, 
                                to_chain
                            ));
                        }
                    }
                    BridgeProtocol::Hop => {
                        if !self.is_hop_supported(*from_chain, *to_chain) {
                            return Err(anyhow::anyhow!(
                                "Hop bridge not supported between chains {} and {}", 
                                from_chain, 
                                to_chain
                            ));
                        }
                    }
                    // Add validation for other bridge protocols
                    _ => {}
                }

                // Verify bridge deadline
                if bridge_data.deadline < U256::from(block_timestamp().unwrap_or_default()) {
                    return Err(anyhow::anyhow!("Bridge deadline has expired"));
                }
            }
        }
        Ok(())
    }

    fn validate_dexes(&self, strategy: &FlashloanStrategy) -> Result<()> {
        for step in &strategy.execution_steps {
            if let ExecutionStep::Swap { chain_id, dex, token_in, token_out, .. } = step {
                match dex {
                    DexProtocol::UniswapV2 => {
                        if !self.is_uniswap_v2_supported(*chain_id) {
                            return Err(anyhow::anyhow!(
                                "Uniswap V2 not supported on chain {}", 
                                chain_id
                            ));
                        }
                    }
                    DexProtocol::UniswapV3 => {
                        if !self.is_uniswap_v3_supported(*chain_id) {
                            return Err(anyhow::anyhow!(
                                "Uniswap V3 not supported on chain {}", 
                                chain_id
                            ));
                        }
                    }
                    // Add validation for other DEX protocols
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn validate_gas_requirements(&self, strategy: &FlashloanStrategy) -> Result<()> {
        for (chain_id, provider) in &self.providers {
            // Get current gas price
            let gas_price = provider.get_gas_price()
                .map_err(|_| anyhow::anyhow!("Failed to get gas price"))?;

            // Estimate gas for all steps on this chain
            let mut total_gas = U256::zero();
            for step in &strategy.execution_steps {
                let step_gas = match step {
                    ExecutionStep::FlashLoan { chain_id: c, .. } if c == chain_id => {
                        U256::from(300_000) // Base gas for flashloan
                    }
                    ExecutionStep::Bridge { from_chain, .. } if from_chain == chain_id => {
                        U256::from(500_000) // Base gas for bridge
                    }
                    ExecutionStep::Swap { chain_id: c, .. } if c == chain_id => {
                        U256::from(200_000) // Base gas for swap
                    }
                    ExecutionStep::AaveSupply { chain_id: c, .. } |
                    ExecutionStep::AaveBorrow { chain_id: c, .. } |
                    ExecutionStep::AaveRepay { chain_id: c, .. } if c == chain_id => {
                        U256::from(250_000) // Base gas for Aave operations
                    }
                    _ => U256::zero()
                };
                total_gas += step_gas;
            }

            // Calculate total gas cost
            let gas_cost = total_gas * gas_price;

            // Get native token balance
            let wallet_address = provider.default_sender()
                .ok_or_else(|| anyhow::anyhow!("No wallet address found"))?;
            let balance = provider.get_balance(wallet_address, None)
                .map_err(|_| anyhow::anyhow!("Failed to get balance"))?;

            // Verify sufficient balance for gas
            if balance < gas_cost {
                return Err(anyhow::anyhow!(
                    "Insufficient gas balance on chain {}. Required: {}, Available: {}", 
                    chain_id, 
                    gas_cost, 
                    balance
                ));
            }
        }

        Ok(())
    }

    fn validate_token_pair(&self, chain_id: u64, token_in: Address, token_out: Address) -> Result<()> {
        // This should be implemented based on your DEX integration
        // For now, we'll just check if tokens are different
        if token_in == token_out {
            return Err(anyhow::anyhow!("Cannot swap same token"));
        }
        Ok(())
    }

    fn is_stargate_supported(&self, from_chain: u64, to_chain: u64) -> bool {
        // Check if we have Stargate protocol instances for both chains
        self.stargate_protocols.contains_key(&from_chain) && 
        self.stargate_protocols.contains_key(&to_chain)
    }

    fn is_hop_supported(&self, from_chain: u64, to_chain: u64) -> bool {
        // Implement actual Hop support check
        true // Placeholder
    }

    fn is_uniswap_v2_supported(&self, chain_id: u64) -> bool {
        // Implement actual Uniswap V2 support check
        matches!(chain_id, 1 | 137 | 42161 | 10) // Supported on mainnet, Polygon, Arbitrum, Optimism
    }

    fn is_uniswap_v3_supported(&self, chain_id: u64) -> bool {
        // Implement actual Uniswap V3 support check
        matches!(chain_id, 1 | 137 | 42161 | 10 | 8453) // Also supported on Base
    }

    fn block_timestamp() -> Result<u64> {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| anyhow::anyhow!("Failed to get timestamp: {}", e))
    }

    fn handle_step_result(
        &self,
        step_type: &str,
        chain_id: u64,
        result: Result<TransactionReceipt>,
        completed_steps: &mut Vec<CompletedStep>,
    ) -> Result<()> {
        match result {
            Ok(receipt) => {
                completed_steps.push(CompletedStep {
                    step_type: step_type.to_string(),
                    chain_id,
                    tx_hash: format!("{:?}", receipt.transaction_hash),
                    gas_used: receipt.gas_used.unwrap_or_default(),
                    success: receipt.status.unwrap_or_default().as_u64() == 1,
                    error: None,
                });
                Ok(())
            }
            Err(e) => {
                let error = format!("Failed to execute {}: {}", step_type, e);
                completed_steps.push(CompletedStep {
                    step_type: step_type.to_string(),
                    chain_id,
                    tx_hash: String::new(),
                    gas_used: U256::zero(),
                    success: false,
                    error: Some(error.clone()),
                });
                Err(anyhow::anyhow!(error))
            }
        }
    }
}
