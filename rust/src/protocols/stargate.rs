use ethers::prelude::*;
use std::sync::Arc;
use anyhow::Result;
use serde::{Serialize, Deserialize};

// Stargate Router ABI functions we need
abigen!(
    StargateRouter,
    r#"[
        function swap(
            uint16 _dstChainId,
            uint256 _srcPoolId,
            uint256 _dstPoolId,
            address payable _refundAddress,
            uint256 _amountLD,
            uint256 _minAmountLD,
            lzTxObj memory _lzTxParams,
            bytes calldata _to,
            bytes calldata _payload
        ) external payable
        
        struct lzTxObj {
            uint256 dstGasForCall;
            uint256 dstNativeAmount;
            bytes dstNativeAddr;
        }
    ]"#
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StargateConfig {
    pub router_address: Address,
    pub pool_id: u256,
    pub chain_id: u16,
    pub native_gas_amount: U256,
    pub gas_for_call: U256,
}

pub struct StargateProtocol<M: Middleware> {
    config: StargateConfig,
    router: StargateRouter<M>,
    provider: Arc<M>,
}

impl<M: Middleware> StargateProtocol<M> {
    pub fn new(
        config: StargateConfig,
        provider: Arc<M>,
    ) -> Self {
        let router = StargateRouter::new(config.router_address, provider.clone());
        
        Self {
            config,
            router,
            provider,
        }
    }

    pub async fn bridge_token(
        &self,
        dst_chain_id: u16,
        src_pool_id: U256,
        dst_pool_id: U256,
        amount: U256,
        min_amount: U256,
        dst_wallet_addr: Address,
        payload: Vec<u8>,
    ) -> Result<TransactionReceipt> {
        // Construct the LayerZero transaction object
        let lz_tx_params = LzTxObj {
            dst_gas_for_call: self.config.gas_for_call,
            dst_native_amount: self.config.native_gas_amount,
            dst_native_addr: dst_wallet_addr.as_bytes().to_vec(),
        };

        // Get refund address (use sender's address)
        let refund_address = self.provider.default_sender()
            .ok_or_else(|| anyhow::anyhow!("No wallet address found"))?;

        // Encode destination address
        let dst_address = ethers::abi::encode(&[
            ethers::abi::Token::Address(dst_wallet_addr)
        ]);

        // Call Stargate Router swap function
        let tx = self.router.swap(
            dst_chain_id,
            src_pool_id,
            dst_pool_id,
            refund_address,
            amount,
            min_amount,
            lz_tx_params,
            dst_address,
            payload,
        );

        // Send transaction and wait for receipt
        let receipt = tx
            .send()
            .await?
            .await?
            .ok_or_else(|| anyhow::anyhow!("Transaction failed"))?;

        Ok(receipt)
    }

    // Helper functions
    pub fn get_router_address(&self) -> Address {
        self.config.router_address
    }

    pub fn get_pool_id(&self) -> U256 {
        self.config.pool_id
    }

    pub fn get_chain_id(&self) -> u16 {
        self.config.chain_id
    }
}
