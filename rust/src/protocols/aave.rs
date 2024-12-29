use ethers::types::{Address, U256, Bytes};
use ethers::prelude::*;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use anyhow::Result;
use crate::protocols::routing::{MultiChainRouter, ChainConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AaveConfig {
    pub pool_address: Address,
    pub pool_data_provider: Address,
    pub price_oracle: Address,
    pub incentives_controller: Address,
    pub supported_assets: Vec<Address>,
}

lazy_static! {
    pub static ref AAVE_V3_DEPLOYMENTS: HashMap<u64, AaveConfig> = {
        let mut m = HashMap::new();
        
        // Ethereum Mainnet (ChainID: 1)
        m.insert(1, AaveConfig {
            pool_address: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".parse().unwrap(),
            pool_data_provider: "0x7B4EB56E7CD4b454BA8ff71E4518426369a138a3".parse().unwrap(),
            price_oracle: "0x54586bE62E3c3580375aE3723C145253060Ca0C0C2".parse().unwrap(),
            incentives_controller: "0x8164Cc65827dcFe994AB23944CBC90e0aa80bFcb".parse().unwrap(),
            supported_assets: vec![
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(), // WETH
                "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap(), // WBTC
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(), // USDC
                "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(), // DAI
            ],
        });
        
        // Polygon (ChainID: 137)
        m.insert(137, AaveConfig {
            pool_address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD".parse().unwrap(),
            pool_data_provider: "0x69FA688f1Dc47d4B5d8029D5a35FB7a548310654".parse().unwrap(),
            price_oracle: "0xb023e699F5a33916Ea823A16485e259257cA8Bd1".parse().unwrap(),
            incentives_controller: "0x929EC64c34a17401F460460D4B9390518E5B473e".parse().unwrap(),
            supported_assets: vec![
                "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".parse().unwrap(), // WETH
                "0x1BFD67037B42Cf73acF2047067bd4F2C47D9BfD6".parse().unwrap(), // WBTC
                "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap(), // USDC
                "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063".parse().unwrap(), // DAI
            ],
        });
        
        // Arbitrum (ChainID: 42161)
        m.insert(42161, AaveConfig {
            pool_address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD".parse().unwrap(),
            pool_data_provider: "0x69FA688f1Dc47d4B5d8029D5a35FB7a548310654".parse().unwrap(),
            price_oracle: "0xb023e699F5a33916Ea823A16485e259257cA8Bd1".parse().unwrap(),
            incentives_controller: "0x929EC64c34a17401F460460D4B9390518E5B473e".parse().unwrap(),
            supported_assets: vec![
                "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".parse().unwrap(), // WETH
                "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f".parse().unwrap(), // WBTC
                "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".parse().unwrap(), // USDC
                "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".parse().unwrap(), // DAI
            ],
        });
        
        // Optimism (ChainID: 10)
        m.insert(10, AaveConfig {
            pool_address: "0x794a61358D6845594F94dc1DB02A252b5b4814aD".parse().unwrap(),
            pool_data_provider: "0x69FA688f1Dc47d4B5d8029D5a35FB7a548310654".parse().unwrap(),
            price_oracle: "0xb023e699F5a33916Ea823A16485e259257cA8Bd1".parse().unwrap(),
            incentives_controller: "0x929EC64c34a17401F460460D4B9390518E5B473e".parse().unwrap(),
            supported_assets: vec![
                "0x4200000000000000000000000000000000000006".parse().unwrap(), // WETH
                "0x68f180fcCe6836688e9084f035309E29Bf0A2095".parse().unwrap(), // WBTC
                "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".parse().unwrap(), // USDC
                "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".parse().unwrap(), // DAI
            ],
        });

        // Base (ChainID: 8453)
        m.insert(8453, AaveConfig {
            pool_address: "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5".parse().unwrap(),
            pool_data_provider: "0x2d8A3C5677189723C4cB8873CfC9C8976FDF38Ac".parse().unwrap(),
            price_oracle: "0x2Da88497588d63c4B1c1462bEb5eE6B8e08130B9".parse().unwrap(),
            incentives_controller: "0x4ea8314b91236e14eD267e30cA830A56bB5c5D1B".parse().unwrap(),
            supported_assets: vec![
                "0x4200000000000000000000000000000000000006".parse().unwrap(), // WETH
                "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".parse().unwrap(), // USDC
                "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb".parse().unwrap(), // DAI
            ],
        });

        m
    };
}

#[derive(Debug)]
pub struct AaveProtocol<M: Middleware> {
    chain_id: u64,
    config: AaveConfig,
    pool_contract: IPool<M>,
    oracle_contract: IPriceOracle<M>,
    data_provider: IPoolDataProvider<M>,
}

#[derive(Debug, Clone)]
pub struct ReserveData {
    pub configuration: ReserveConfigurationMap,
    pub liquidity_index: U256,
    pub current_liquidity_rate: U256,
    pub variable_borrow_index: U256,
    pub current_variable_borrow_rate: U256,
    pub current_stable_borrow_rate: U256,
    pub last_update_timestamp: U256,
    pub id: u16,
    pub a_token_address: Address,
    pub stable_debt_token_address: Address,
    pub variable_debt_token_address: Address,
    pub interest_rate_strategy_address: Address,
    pub acc_stable_borrow_index: U256,
    pub supply_cap: U256,
    pub borrow_cap: U256,
    pub debt_ceiling: U256,
    pub debt_ceiling_decimals: u8,
    pub emode_category: u8,
}

#[derive(Debug, Clone)]
pub struct UserAccountData {
    pub total_collateral_base: U256,
    pub total_debt_base: U256,
    pub available_borrows_base: U256,
    pub current_liquidation_threshold: U256,
    pub ltv: U256,
    pub health_factor: U256,
}

impl<M: Middleware> AaveProtocol<M> {
    pub fn new(chain_id: u64, client: Arc<M>) -> Result<Self> {
        let config = AAVE_V3_DEPLOYMENTS.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Unsupported chain ID"))?;

        let pool_contract = IPool::new(config.pool_address, client.clone());
        let oracle_contract = IPriceOracle::new(config.price_oracle, client.clone());
        let data_provider = IPoolDataProvider::new(config.pool_data_provider, client.clone());

        Ok(Self {
            chain_id,
            config: config.clone(),
            pool_contract,
            oracle_contract,
            data_provider,
        })
    }

    // Flashloan Operations
    pub async fn execute_flashloan(
        &self,
        receiver: Address,
        assets: Vec<Address>,
        amounts: Vec<U256>,
        interest_rate_modes: Vec<u8>,
        params: Bytes,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .flashloan(receiver, assets, amounts, interest_rate_modes, receiver, params, 0)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Transaction failed"))?)
    }

    // Supply/Borrow Operations
    pub async fn supply(
        &self,
        asset: Address,
        amount: U256,
        on_behalf_of: Address,
        referral_code: u16,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .supply(asset, amount, on_behalf_of, referral_code)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Supply failed"))?)
    }

    pub async fn borrow(
        &self,
        asset: Address,
        amount: U256,
        interest_rate_mode: u8,
        referral_code: u16,
        on_behalf_of: Address,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .borrow(asset, amount, interest_rate_mode, referral_code, on_behalf_of)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Borrow failed"))?)
    }

    pub async fn repay(
        &self,
        asset: Address,
        amount: U256,
        interest_rate_mode: u8,
        on_behalf_of: Address,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .repay(asset, amount, interest_rate_mode, on_behalf_of)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Repay failed"))?)
    }

    // Position Management
    pub async fn get_user_account_data(&self, user: Address) -> Result<UserAccountData> {
        let data = self.pool_contract
            .get_user_account_data(user)
            .call()
            .await?;

        Ok(UserAccountData {
            total_collateral_base: data.0,
            total_debt_base: data.1,
            available_borrows_base: data.2,
            current_liquidation_threshold: data.3,
            ltv: data.4,
            health_factor: data.5,
        })
    }

    pub async fn get_reserve_data(&self, asset: Address) -> Result<ReserveData> {
        let data = self.pool_contract
            .get_reserve_data(asset)
            .call()
            .await?;

        Ok(ReserveData {
            configuration: data.0,
            liquidity_index: data.1,
            current_liquidity_rate: data.2,
            variable_borrow_index: data.3,
            current_variable_borrow_rate: data.4,
            current_stable_borrow_rate: data.5,
            last_update_timestamp: data.6,
            id: data.7,
            a_token_address: data.8,
            stable_debt_token_address: data.9,
            variable_debt_token_address: data.10,
            interest_rate_strategy_address: data.11,
            acc_stable_borrow_index: data.12,
            supply_cap: data.13,
            borrow_cap: data.14,
            debt_ceiling: data.15,
            debt_ceiling_decimals: data.16,
            emode_category: data.17,
        })
    }

    // Price and Rate Queries
    pub async fn get_asset_price(&self, asset: Address) -> Result<U256> {
        Ok(self.oracle_contract.get_asset_price(asset).call().await?)
    }

    pub async fn get_reserve_normalized_income(&self, asset: Address) -> Result<U256> {
        Ok(self.pool_contract.get_reserve_normalized_income(asset).call().await?)
    }

    pub async fn get_reserve_normalized_variable_debt(&self, asset: Address) -> Result<U256> {
        Ok(self.pool_contract.get_reserve_normalized_variable_debt(asset).call().await?)
    }

    // Risk Management
    pub async fn set_user_use_reserve_as_collateral(
        &self,
        asset: Address,
        use_as_collateral: bool,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .set_user_use_reserve_as_collateral(asset, use_as_collateral)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Failed to set collateral usage"))?)
    }

    pub async fn swap_borrow_rate_mode(
        &self,
        asset: Address,
        interest_rate_mode: u8,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .swap_borrow_rate_mode(asset, interest_rate_mode)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Failed to swap borrow rate"))?)
    }

    // Liquidation
    pub async fn liquidation_call(
        &self,
        collateral_asset: Address,
        debt_asset: Address,
        user: Address,
        debt_to_cover: U256,
        receive_a_token: bool,
    ) -> Result<TransactionReceipt> {
        let tx = self.pool_contract
            .liquidation_call(collateral_asset, debt_asset, user, debt_to_cover, receive_a_token)
            .send()
            .await?
            .await?;
        Ok(tx.ok_or_else(|| anyhow::anyhow!("Liquidation failed"))?)
    }

    // Helper Functions
    pub async fn calculate_health_factor_from_balances(
        &self,
        total_collateral_in_base_currency: U256,
        total_debt_in_base_currency: U256,
        liquidation_threshold: U256,
    ) -> Result<U256> {
        if total_debt_in_base_currency.is_zero() {
            return Ok(U256::MAX);
        }

        Ok((total_collateral_in_base_currency
            .checked_mul(liquidation_threshold)?
            .checked_div(10000)?)
            .checked_div(total_debt_in_base_currency)?)
    }

    pub async fn calculate_user_debt_position(
        &self,
        user: Address,
        asset: Address,
    ) -> Result<(U256, U256)> {
        let stable_debt = self.pool_contract
            .get_stable_debt(asset, user)
            .call()
            .await?;

        let variable_debt = self.pool_contract
            .get_variable_debt(asset, user)
            .call()
            .await?;

        Ok((stable_debt, variable_debt))
    }

    // Multi-chain routing methods
    pub async fn find_best_lending_rates(
        &self,
        router: &MultiChainRouter<M>,
        asset: Address,
        amount: U256,
    ) -> Result<Vec<(u64, f64)>> {
        let rates = router.find_best_rates(asset, amount, self.chain_id).await?;
        Ok(rates.into_iter()
            .map(|r| (r.chain_id, r.supply_apy))
            .collect())
    }

    pub async fn find_best_borrowing_rates(
        &self,
        router: &MultiChainRouter<M>,
        asset: Address,
        amount: U256,
    ) -> Result<Vec<(u64, f64)>> {
        let rates = router.find_best_rates(asset, amount, self.chain_id).await?;
        Ok(rates.into_iter()
            .map(|r| (r.chain_id, r.borrow_apy))
            .collect())
    }

    pub async fn execute_cross_chain_supply(
        &self,
        router: &MultiChainRouter<M>,
        asset: Address,
        amount: U256,
        target_chain: u64,
    ) -> Result<Vec<TransactionReceipt>> {
        let rates = router.find_best_rates(asset, amount, self.chain_id).await?;
        
        // Find target chain rate
        let target_rate = rates.iter()
            .find(|r| r.chain_id == target_chain)
            .ok_or_else(|| anyhow::anyhow!("Target chain not found"))?;
            
        // Find arbitrage routes
        let routes = router.find_arbitrage_routes(
            asset,
            amount,
            self.chain_id,
            U256::zero() // Include all routes
        ).await?;
        
        // Execute best route
        if let Some(best_route) = routes.first() {
            router.execute_route(best_route.clone()).await
        } else {
            Err(anyhow::anyhow!("No viable routes found"))
        }
    }

    pub async fn monitor_cross_chain_opportunities(
        &self,
        router: &MultiChainRouter<M>,
        asset: Address,
        amount: U256,
        min_profit: U256,
        update_interval: u64,
    ) -> Result<()> {
        use tokio::time::{interval, Duration};
        
        let mut interval = interval(Duration::from_secs(update_interval));
        
        loop {
            interval.tick().await;
            
            match router.find_arbitrage_routes(
                asset,
                amount,
                self.chain_id,
                min_profit
            ).await {
                Ok(routes) => {
                    for route in routes {
                        println!(
                            "Found opportunity: {} -> {} | Profit: {} wei",
                            route.source_chain,
                            route.target_chain,
                            route.estimated_profit
                        );
                    }
                }
                Err(e) => eprintln!("Error finding routes: {}", e),
            }
        }
    }
}

// Contract interfaces
abigen!(
    IPool,
    r#"[
        function flashLoan(address receiverAddress, address[] calldata assets, uint256[] calldata amounts, uint256[] calldata modes, address onBehalfOf, bytes calldata params, uint16 referralCode) external
        function supply(address asset, uint256 amount, address onBehalfOf, uint16 referralCode) external
        function borrow(address asset, uint256 amount, uint256 interestRateMode, uint16 referralCode, address onBehalfOf) external
        function repay(address asset, uint256 amount, uint256 rateMode, address onBehalfOf) external
        function getUserAccountData(address user) external view returns (uint256, uint256, uint256, uint256, uint256, uint256)
        function getReserveData(address asset) external view returns (tuple(uint256,uint256,uint256,uint256,uint256,uint256,uint40,uint16,address,address,address,address,uint128,uint128,uint128,uint128,uint8,uint8))
        function getReserveNormalizedIncome(address asset) external view returns (uint256)
        function getReserveNormalizedVariableDebt(address asset) external view returns (uint256)
        function setUserUseReserveAsCollateral(address asset, bool useAsCollateral) external
        function swapBorrowRateMode(address asset, uint256 rateMode) external
        function liquidationCall(address collateralAsset, address debtAsset, address user, uint256 debtToCover, bool receiveAToken) external
        function getStableDebt(address asset, address user) external view returns (uint256)
        function getVariableDebt(address asset, address user) external view returns (uint256)
    ]"#
);

abigen!(
    IPriceOracle,
    r#"[
        function getAssetPrice(address asset) external view returns (uint256)
    ]"#
);

abigen!(
    IPoolDataProvider,
    r#"[
        function getReserveData(address asset) external view returns (uint256, uint256, uint256, uint256, uint256, uint256, uint256, uint256, uint256, uint256)
    ]"#
);
