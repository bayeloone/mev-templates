use ethers::types::{U256, Address};

/// Price source with weight and timestamp
#[derive(Debug, Clone)]
pub struct PriceSource {
    pub price: U256,
    pub weight: f64,
    pub source: String,
}

/// Token validation result
#[derive(Debug, Clone)]
pub struct TokenValidation {
    pub is_valid: bool,
    pub reason: String,
    pub error: Option<String>,
}

/// TWAP data with timestamp and sample count
#[derive(Debug, Clone)]
pub struct TWAPData {
    pub price: U256,
    pub timestamp: u64,
    pub samples: u32,
}

/// Volume data with sources and timestamp
#[derive(Debug)]
pub struct VolumeData {
    pub volume_24h: U256,
    pub sources: Vec<String>,
    pub last_updated: u64,
}

/// Holder data with unique holders, top holders, and concentration
#[derive(Debug)]
pub struct HolderData {
    pub unique_holders: usize,
    pub top_holders: Vec<(Address, U256)>,
    pub last_updated: u64,
}

/// Contract data with creation timestamp, verification status, and source code hash
#[derive(Debug)]
pub struct ContractData {
    pub created_at: u64,
    pub is_verified: bool,
    pub source_hash: Option<String>,
    pub last_updated: u64,
}
