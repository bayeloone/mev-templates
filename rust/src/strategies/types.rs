use ethers::types::{Address, U256, Bytes};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashloanStrategy {
    pub source_chain: u64,
    pub target_chain: u64,
    pub flash_token: Address,
    pub flash_amount: U256,
    pub min_profit: U256,
    pub max_slippage: f64,
    pub execution_steps: Vec<ExecutionStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStep {
    FlashLoan {
        chain_id: u64,
        token: Address,
        amount: U256,
        params: Bytes,
    },
    Bridge {
        from_chain: u64,
        to_chain: u64,
        token: Address,
        amount: U256,
        bridge_data: BridgeData,
    },
    Swap {
        chain_id: u64,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        min_amount_out: U256,
        dex: DexProtocol,
    },
    AaveSupply {
        chain_id: u64,
        token: Address,
        amount: U256,
    },
    AaveBorrow {
        chain_id: u64,
        token: Address,
        amount: U256,
        interest_rate_mode: u8,
    },
    AaveRepay {
        chain_id: u64,
        token: Address,
        amount: U256,
        interest_rate_mode: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeData {
    pub protocol: BridgeProtocol,
    pub gas_limit: U256,
    pub deadline: U256,
    pub signature: Option<Bytes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeProtocol {
    Stargate,
    Hop,
    CCTP,
    LayerZero,
    Across,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DexProtocol {
    UniswapV2,
    UniswapV3,
    Curve,
    Balancer,
    OneInch,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub profit: U256,
    pub gas_used: U256,
    pub error: Option<String>,
    pub steps_completed: Vec<CompletedStep>,
}

#[derive(Debug, Clone)]
pub struct CompletedStep {
    pub step_type: String,
    pub chain_id: u64,
    pub tx_hash: String,
    pub gas_used: U256,
    pub success: bool,
    pub error: Option<String>,
}
