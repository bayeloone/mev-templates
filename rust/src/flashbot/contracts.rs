use anyhow::Result;
use ethers::{
    types::{Address, U256, Bytes},
    contract::{Contract, ContractFactory},
    providers::{Provider, Http},
    middleware::SignerMiddleware,
    signers::LocalWallet,
};
use std::sync::Arc;

pub struct ContractManager {
    // Core contracts
    pub executor: Address,
    pub vault: Address,
    
    // Protocol adapters
    pub dex_adapters: HashMap<String, Address>,
    pub lending_adapters: HashMap<String, Address>,
    
    // Safety modules
    pub emergency_stop: Address,
    pub access_control: Address,
    
    // Contract interfaces
    executor_contract: Contract<Provider<Http>>,
    vault_contract: Contract<Provider<Http>>,
}

impl ContractManager {
    pub async fn new(
        provider: Arc<Provider<Http>>,
        executor: Address,
        vault: Address,
    ) -> Result<Self> {
        // Load contract ABIs
        let executor_contract = Contract::new(executor, EXECUTOR_ABI.parse()?, provider.clone());
        let vault_contract = Contract::new(vault, VAULT_ABI.parse()?, provider.clone());
        
        Ok(Self {
            executor,
            vault,
            dex_adapters: HashMap::new(),
            lending_adapters: HashMap::new(),
            emergency_stop: Address::zero(),
            access_control: Address::zero(),
            executor_contract,
            vault_contract,
        })
    }

    /// Execute flashloan arbitrage
    pub async fn execute_flashloan(
        &self,
        token: Address,
        amount: U256,
        pools: Vec<Address>,
        data: Bytes,
    ) -> Result<()> {
        self.executor_contract
            .method("executeFlashloan", (token, amount, pools, data))?
            .send()
            .await?
            .await?;
        Ok(())
    }

    /// Deploy new protocol adapter
    pub async fn deploy_adapter(
        &mut self,
        protocol: &str,
        implementation: Address,
    ) -> Result<Address> {
        // Deploy adapter proxy
        let adapter = self.deploy_proxy(implementation).await?;
        
        // Initialize adapter
        self.initialize_adapter(adapter, protocol).await?;
        
        // Register adapter
        self.register_adapter(protocol, adapter).await?;
        
        // Store adapter address
        self.dex_adapters.insert(protocol.to_string(), adapter);
        
        Ok(adapter)
    }

    /// Emergency stop all operations
    pub async fn emergency_stop(&self) -> Result<()> {
        self.executor_contract
            .method("emergencyStop", ())?
            .send()
            .await?
            .await?;
        Ok(())
    }

    /// Withdraw funds from vault
    pub async fn withdraw(
        &self,
        token: Address,
        amount: U256,
        recipient: Address,
    ) -> Result<()> {
        self.vault_contract
            .method("withdraw", (token, amount, recipient))?
            .send()
            .await?
            .await?;
        Ok(())
    }

    /// Update protocol fee
    pub async fn update_fee(&self, new_fee: U256) -> Result<()> {
        self.executor_contract
            .method("updateFee", new_fee)?
            .send()
            .await?
            .await?;
        Ok(())
    }

    /// Add new operator
    pub async fn add_operator(&self, operator: Address) -> Result<()> {
        self.access_control
            .method("addOperator", operator)?
            .send()
            .await?
            .await?;
        Ok(())
    }

    /// Check if address is operator
    pub async fn is_operator(&self, address: Address) -> Result<bool> {
        Ok(self.access_control
            .method("isOperator", address)?
            .call()
            .await?)
    }

    /// Get protocol fee
    pub async fn get_fee(&self) -> Result<U256> {
        Ok(self.executor_contract
            .method("fee", ())?
            .call()
            .await?)
    }

    /// Get vault balance
    pub async fn get_balance(&self, token: Address) -> Result<U256> {
        Ok(self.vault_contract
            .method("getBalance", token)?
            .call()
            .await?)
    }
}
