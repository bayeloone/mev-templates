use anyhow::{anyhow, Result};
use ethers::{
    types::{Address, U256, H256, BlockNumber},
    providers::{Provider, Http},
    contract::{Contract, abigen},
};
use std::{sync::Arc, time::{Duration, SystemTime}, collections::HashMap};
use tokio::sync::RwLock;
use log::{info, warn, error};
use serde::{Serialize, Deserialize};
use std::cmp::min;

/// Maximum allowed slippage (3%)
pub const MAX_SLIPPAGE: u64 = 300;

/// Maximum impact on pool reserves (1%)
pub const MAX_POOL_IMPACT: u64 = 100;

/// Minimum liquidity requirement ($10,000)
pub const MIN_LIQUIDITY_USD: u64 = 10_000;

/// Maximum gas price (500 gwei)
pub const MAX_GAS_PRICE: u64 = 500_000_000_000;

/// Minimum token age in days for whitelisting
pub const MIN_TOKEN_AGE_DAYS: u64 = 30;

/// Minimum holder count for whitelisting
pub const MIN_HOLDERS: u64 = 1000;

/// Minimum trading volume in USD
pub const MIN_VOLUME_USD: u64 = 100_000;

/// Price oracle interface for common stablecoins
const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const USDT_ADDRESS: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

/// Common ERC20 tokens and their metadata
const TOKEN_METADATA: &[(&str, &str, u8)] = &[
    // Stablecoins
    ("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "USDC", 6),
    ("0xdAC17F958D2ee523a2206206994597C13D831ec7", "USDT", 6),
    ("0x6B175474E89094C44Da98b954EedeAC495271d0F", "DAI", 18),
    ("0x4Fabb145d64652a948d72533023f6E7A623C7C53", "BUSD", 18),
    ("0x8E870D67F660D95d5be530380D0eC0bd388289E1", "PAX", 18),
    ("0x956F47F50A910163D8BF957Cf5846D573E7f87CA", "FEI", 18),
    
    // Major DEX tokens
    ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "WETH", 18),
    ("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", "WBTC", 8),
    ("0x7D1AfA7B718fb893dB30A3aBc0Cfc608AaCfeBB0", "MATIC", 18),
    
    // DeFi tokens
    ("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984", "UNI", 18),
    ("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9", "AAVE", 18),
    ("0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2", "MKR", 18),
    ("0x0bc529c00C6401aEF6D220BE8C6Ea1667F6Ad93e", "YFI", 18),
    ("0x6B3595068778DD592e39A122f4f5a5cF09C90fE2", "SUSHI", 18),
    ("0xba100000625a3754423978a60c9317c58a424e3D", "BAL", 18),
    
    // Liquid staking tokens
    ("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84", "stETH", 18),
    ("0xBe9895146f7AF43049ca1c1AE358B0541Ea49704", "cbETH", 18),
    ("0x9559Aaa82d9649C7A7b220E7c461d2E74c9a3593", "rETH", 18),
];

/// Chainlink price feed addresses
const CHAINLINK_FEEDS: &[(&str, &str)] = &[
    ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"), // ETH/USD
    ("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", "0xF4030086522a5bEEa4988F8cA5B36dbC97BeE88c"), // BTC/USD
    ("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6"), // USDC/USD
];

/// TWAP configuration
const TWAP_PERIOD: u64 = 1800; // 30 minutes
const MIN_TWAP_SAMPLES: u32 = 3;

/// Token validation constants
const MIN_TOTAL_SUPPLY: U256 = U256([1_000_000, 0, 0, 0]); // 1M tokens
const MAX_TOTAL_SUPPLY: U256 = U256([1_000_000_000_000, 0, 0, 0]); // 1T tokens
const MIN_HOLDERS_FOR_VALID: u32 = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Stablecoin,
    WrappedNative,
    WrappedBTC,
    LiquidStaking,
    DeFi,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub decimals: u8,
    pub token_type: TokenType,
}

impl TokenInfo {
    fn new(address: Address, symbol: &str, decimals: u8) -> Self {
        let token_type = match symbol {
            "USDC" | "USDT" | "DAI" | "BUSD" | "PAX" | "FEI" => TokenType::Stablecoin,
            "WETH" => TokenType::WrappedNative,
            "WBTC" => TokenType::WrappedBTC,
            "stETH" | "cbETH" | "rETH" => TokenType::LiquidStaking,
            "UNI" | "AAVE" | "MKR" | "YFI" | "SUSHI" | "BAL" => TokenType::DeFi,
            _ => TokenType::Unknown,
        };

        Self {
            address,
            symbol: symbol.to_string(),
            decimals,
            token_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    /// Token creation timestamp
    pub created_at: u64,
    /// Number of holders
    pub holder_count: u64,
    /// 24h trading volume in USD
    pub volume_24h: u64,
    /// Last price in USD
    pub price_usd: f64,
    /// Last update timestamp
    pub last_updated: u64,
    /// Blacklist reason if any
    pub blacklist_reason: Option<String>,
    /// Whether token is verified on blockchain explorer
    pub is_verified: bool,
    /// Token contract source code hash if verified
    pub source_hash: Option<String>,
}

#[derive(Debug)]
pub struct SecurityConfig {
    /// Maximum slippage tolerance in basis points (1 = 0.01%)
    pub max_slippage: u64,
    /// Maximum pool impact in basis points
    pub max_pool_impact: u64,
    /// Minimum liquidity required in USD
    pub min_liquidity_usd: u64,
    /// Maximum gas price in wei
    pub max_gas_price: U256,
    /// Blacklisted tokens
    pub blacklisted_tokens: Vec<Address>,
    /// Blacklisted contracts
    pub blacklisted_contracts: Vec<Address>,
    /// Token metadata cache
    pub token_metadata: Arc<RwLock<HashMap<Address, TokenMetadata>>>,
    /// Known malicious code patterns
    pub malicious_patterns: Vec<String>,
    /// Trusted token creators
    pub trusted_creators: Vec<Address>,
    /// Etherscan API key
    pub etherscan_api_key: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_slippage: MAX_SLIPPAGE,
            max_pool_impact: MAX_POOL_IMPACT,
            min_liquidity_usd: MIN_LIQUIDITY_USD,
            max_gas_price: U256::from(MAX_GAS_PRICE),
            blacklisted_tokens: vec![],
            blacklisted_contracts: vec![],
            token_metadata: Arc::new(RwLock::new(HashMap::new())),
            malicious_patterns: vec![
                // Known honeypot patterns
                "function transfer(address,uint256) external returns (bool) { revert(); }".to_string(),
                "function approve(address,uint256) external returns (bool) { revert(); }".to_string(),
                // Fee manipulation patterns
                "uint256 private _fee = 0;".to_string(),
                "if(sender != owner) { fee = 99; }".to_string(),
            ],
            trusted_creators: vec![
                // Add known legitimate token creators
                Address::from_slice(&hex::decode("1111111111111111111111111111111111111111").unwrap()),
            ],
            etherscan_api_key: "YOUR_API_KEY".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenPrice {
    pub price_usd: U256,    // Price in USD with 18 decimals
    pub decimals: u8,       // Token decimals
    pub last_updated: u64,  // Last update timestamp
}

#[derive(Debug)]
pub struct Pool {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee: u64,
}

mod price;
mod token;
mod twap;
mod types;

pub use price::PriceManager;
pub use token::TokenManager;
pub use twap::TWAPManager;
pub use types::*;

use anyhow::Result;
use ethers::types::Address;
use std::sync::Arc;
use crate::dex::DexPool;

pub struct SecurityManager {
    price_manager: Arc<PriceManager>,
    token_manager: Arc<TokenManager>,
    twap_manager: Arc<TWAPManager>,
}

impl SecurityManager {
    pub fn new() -> Self {
        Self {
            price_manager: Arc::new(PriceManager::new()),
            token_manager: Arc::new(TokenManager::new()),
            twap_manager: Arc::new(TWAPManager::new()),
        }
    }

    /// Validate token and get its metadata
    pub async fn validate_token(&self, token: Address) -> Result<TokenValidation> {
        self.token_manager.validate_token(token).await
    }

    /// Get TWAP price for a token
    pub async fn get_twap(&self, pool: &DexPool, token: Address) -> Result<Option<TWAPData>> {
        self.twap_manager.get_v3_twap(pool, token).await
    }

    /// Get spot price from various sources
    pub async fn get_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        // Try Uniswap V3 first
        if let Some(price) = self.price_manager.get_uniswap_v3_price(pool, token).await? {
            return Ok(Some(price));
        }

        // Fallback to Balancer
        self.price_manager.get_balancer_price(pool, token).await
    }

    /// Check if token is USD-based
    pub fn is_usd_token(&self, token: Address) -> bool {
        self.price_manager.is_usd_token(token)
    }
}

/// Normalize token amount to 18 decimals
fn normalize_to_18_decimals(amount: U256, token_decimals: u8) -> U256 {
    if token_decimals == 18 {
        amount
    } else if token_decimals < 18 {
        amount.saturating_mul(U256::exp10(18 - token_decimals))
    } else {
        amount.saturating_div(U256::exp10(token_decimals - 18))
    }
}

// Generate type-safe contract bindings
abigen!(
    ChainlinkOracle,
    r#"[
        function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound)
    ]"#,
);

abigen!(
    ERC20,
    r#"[
        function name() external view returns (string)
        function symbol() external view returns (string)
        function decimals() external view returns (uint8)
        function totalSupply() external view returns (uint256)
        function balanceOf(address account) external view returns (uint256)
        function transfer(address recipient, uint256 amount) external returns (bool)
        function allowance(address owner, address spender) external view returns (uint256)
        function approve(address spender, uint256 amount) external returns (bool)
        function transferFrom(address sender, address recipient, uint256 amount) external returns (bool)
    ]"#,
);

abigen!(
    UniswapV2Pair,
    r#"[
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        function price0CumulativeLast() external view returns (uint256)
        function price1CumulativeLast() external view returns (uint256)
        function token0() external view returns (address)
        function token1() external view returns (address)
    ]"#,
);

abigen!(
    UniswapV3Pool,
    r#"[
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
        function token0() external view returns (address)
        function token1() external view returns (address)
        function liquidity() external view returns (uint128)
        function observe(uint32[] secondsAgos) external view returns (int56[] tickCumulatives, uint160[] secondsPerLiquidityCumulativeX128s)
    ]"#,
);

abigen!(
    BalancerPool,
    r#"[
        function getPoolId() external view returns (bytes32)
        function getVault() external view returns (address)
        function getTotalLiquidity() external view returns (uint256)
        function getLatest(uint8 variable) external view returns (uint256)
    ]"#,
);

abigen!(
    BalancerVault,
    r#"[
        function getPoolTokens(bytes32 poolId) external view returns (address[] tokens, uint256[] balances, uint256 lastChangeBlock)
    ]"#,
);

abigen!(
    CurvePool,
    r#"[
        function coins(uint256 i) external view returns (address)
        function balances(uint256 i) external view returns (uint256)
        function get_dy(int128 i, int128 j, uint256 dx) external view returns (uint256)
        function get_virtual_price() external view returns (uint256)
    ]"#,
);

abigen!(
    CurveRegistry,
    r#"[
        function pool_count() external view returns (uint256)
        function pool_list(uint256 id) external view returns (address)
        function get_pool_coins(address pool) external view returns (address[8] coins, uint256[8] balances, uint256[8] decimals)
        function get_pool_info(address pool) external view returns (uint256[8] balances, uint256[8] decimals, uint256 A, uint256 fee)
        function get_virtual_price_from_lp_token(address lpToken) external view returns (uint256)
    ]"#,
);

abigen!(
    CurveMetaRegistry,
    r#"[
        function get_registry() external view returns (address)
        function get_base_registry() external view returns (address)
        function get_gauges_registry() external view returns (address)
    ]"#,
);

abigen!(
    UniswapV3Factory,
    r#"[
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
        function feeAmountTickSpacing(uint24 fee) external view returns (int24)
        function owner() external view returns (address)
    ]"#,
);

#[derive(Debug, Clone)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
    Balancer,
    Curve,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DexPool {
    pub address: Address,
    pub dex_type: DexType,
    pub tokens: Vec<Address>,
    pub liquidity_usd: U256,
    pub volume_24h: U256,
}

impl SecurityManager {
    // ... existing methods ...

    /// Get prices from multiple DEXes with volume weighting
    async fn get_dex_prices(&self, token: Address) -> Result<Vec<PriceSource>> {
        let mut prices = Vec::new();
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;

        // 1. Find pools across different DEXes
        let pools = self.find_all_pools(token).await?;
        
        for pool in pools {
            match pool.dex_type {
                DexType::UniswapV2 => {
                    if let Some(price) = self.get_uniswap_v2_price(&pool, token).await? {
                        prices.push(price);
                    }
                },
                DexType::UniswapV3 => {
                    if let Some(price) = self.get_uniswap_v3_price(&pool, token).await? {
                        prices.push(price);
                    }
                },
                DexType::Balancer => {
                    if let Some(price) = self.get_balancer_price(&pool, token).await? {
                        prices.push(price);
                    }
                },
                DexType::Curve => {
                    if let Some(price) = self.get_curve_price(&pool, token).await? {
                        prices.push(price);
                    }
                },
                DexType::Unknown => continue,
            }
        }

        Ok(prices)
    }

    /// Find all pools across different DEXes
    async fn find_all_pools(&self, token: Address) -> Result<Vec<DexPool>> {
        let mut pools = Vec::new();
        let client = Arc::new(Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?);
        
        // 1. Find Uniswap V2 & fork pools
        pools.extend(self.find_uniswap_v2_pools(token).await?);

        // 2. Find Uniswap V3 pools
        pools.extend(self.find_uniswap_v3_pools(token).await?);

        // 3. Find Balancer pools
        pools.extend(self.find_balancer_pools(token).await?);

        // 4. Find Curve pools
        pools.extend(self.find_curve_pools(token).await?);

        // Sort pools by liquidity
        pools.sort_by(|a, b| b.liquidity_usd.cmp(&a.liquidity_usd));

        Ok(pools)
    }

    /// Get price from Uniswap V3 pool
    async fn get_uniswap_v3_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = UniswapV3Pool::new(pool.address, client.clone());

        // Get current price from slot0
        let (sqrt_price_x96, _, _, _, _, _, _) = pool_contract.slot0().call().await?;
        
        // Convert sqrtPriceX96 to price
        let price_x96 = U256::from(sqrt_price_x96)
            .saturating_mul(U256::from(sqrt_price_x96))
            .checked_div(U256::from(1u128 << 96))
            .ok_or_else(|| anyhow!("Price calculation overflow"))?;

        let is_token0 = token == pool_contract.token0().call().await?;
        let price = if is_token0 {
            price_x96
        } else {
            U256::from(1u128 << 96)
                .checked_div(price_x96)
                .ok_or_else(|| anyhow!("Price inversion overflow"))?
        };

        Ok(Some(PriceSource {
            price,
            weight: 2, // Higher weight for V3 due to concentrated liquidity
            timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
            source: format!("uniswap_v3_{:?}", pool.address),
        }))
    }

    /// Get price from Balancer pool
    async fn get_balancer_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = BalancerPool::new(pool.address, client.clone());
        let vault_address = pool_contract.get_vault().call().await?;
        let vault = BalancerVault::new(vault_address, client);

        // Get pool tokens and balances
        let pool_id = pool_contract.get_pool_id().call().await?;
        let (tokens, balances, _) = vault.get_pool_tokens(pool_id).call().await?;

        // Find token index and stable index
        let token_idx = tokens.iter().position(|&t| t == token)
            .ok_or_else(|| anyhow!("Token not found in pool"))?;
        let stable_idx = tokens.iter().position(|&t| {
            ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7", "0x6B175474E89094C44Da98b954EedeAC495271d0F"].iter().any(|&s| {
                t == Address::from_slice(&hex::decode(s.trim_start_matches("0x")).unwrap())
            })
        }).ok_or_else(|| anyhow!("No stablecoin found in pool"))?;

        // Calculate price based on balances
        let price = U256::from(balances[stable_idx])
            .saturating_mul(U256::exp10(18))
            .checked_div(U256::from(balances[token_idx]))
            .ok_or_else(|| anyhow!("Price calculation overflow"))?;

        Ok(Some(PriceSource {
            price,
            weight: 1,
            timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
            source: format!("balancer_{:?}", pool.address),
        }))
    }

    /// Get price from Curve pool
    async fn get_curve_price(&self, pool: &DexPool, token: Address) -> Result<Option<PriceSource>> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = CurvePool::new(pool.address, client.clone());

        // Find token indices
        let mut token_idx = None;
        let mut stable_idx = None;
        for i in 0..8 { // Curve pools can have up to 8 tokens
            if let Ok(coin) = pool_contract.coins(U256::from(i)).call().await {
                if coin == token {
                    token_idx = Some(i);
                }
                if ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7", "0x6B175474E89094C44Da98b954EedeAC495271d0F"].iter().any(|&s| {
                    coin == Address::from_slice(&hex::decode(s.trim_start_matches("0x")).unwrap())
                }) {
                    stable_idx = Some(i);
                }
            } else {
                break;
            }
        }

        if let (Some(token_i), Some(stable_i)) = (token_idx, stable_idx) {
            // Get price using get_dy
            let virtual_price = pool_contract.get_virtual_price().call().await?;
            let dy = pool_contract
                .get_dy(
                    token_i as i128,
                    stable_i as i128,
                    U256::exp10(18)
                )
                .call()
                .await?;

            let price = dy.saturating_mul(virtual_price)
                .checked_div(U256::exp10(18))
                .ok_or_else(|| anyhow!("Price calculation overflow"))?;

            Ok(Some(PriceSource {
                price,
                weight: 2, // Higher weight for Curve due to stable math
                timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
                source: format!("curve_{:?}", pool.address),
            }))
        } else {
            Ok(None)
        }
    }

    /// Find Uniswap V3 pools
    async fn find_uniswap_v3_pools(&self, token: Address) -> Result<Vec<DexPool>> {
        let mut pools = Vec::new();
        let client = Arc::new(Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?);
        
        // Initialize Uniswap V3 factory
        let factory = UniswapV3Factory::new(
            Address::from_slice(&hex::decode("1F98431c8aD98523631AE4a59f267346ea31F984").unwrap()),
            client.clone()
        );

        // Common paired tokens to check
        let paired_tokens = [
            // Stablecoins
            ("USDC", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            ("USDT", "0xdAC17F958D2ee523a2206206994597C13D831ec7"),
            ("DAI", "0x6B175474E89094C44Da98b954EedeAC495271d0F"),
            // Major tokens
            ("WETH", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            ("WBTC", "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"),
        ];

        // Fee tiers to check (0.01%, 0.05%, 0.3%, 1%)
        let fee_tiers = [100, 500, 3000, 10000];

        // Batch pool queries for efficiency
        let mut pool_promises = Vec::new();

        for (_, paired_token) in paired_tokens.iter() {
            let paired_addr = Address::from_slice(&hex::decode(paired_token.trim_start_matches("0x")).unwrap());
            
            for &fee in fee_tiers.iter() {
                let factory_clone = factory.clone();
                let token_a = std::cmp::min(token, paired_addr);
                let token_b = std::cmp::max(token, paired_addr);
                
                pool_promises.push(tokio::spawn(async move {
                    let pool_addr = factory_clone.get_pool(token_a, token_b, fee).call().await?;
                    if pool_addr != Address::zero() {
                        // Initialize pool contract
                        let pool = UniswapV3Pool::new(pool_addr, Arc::new(client.clone()));
                        
                        // Get pool data
                        let liquidity = pool.liquidity().call().await?;
                        let (sqrt_price_x96, _, _, _, _, _, _) = pool.slot0().call().await?;
                        
                        Ok::<_, Error>((pool_addr, liquidity, sqrt_price_x96, fee))
                    } else {
                        Err(anyhow!("Pool does not exist"))
                    }
                }));
            }
        }

        // Process pool results
        for promise in pool_promises {
            if let Ok(Ok((pool_addr, liquidity, sqrt_price_x96, fee))) = promise.await {
                // Calculate pool liquidity in USD
                let total_liquidity = self.calculate_v3_liquidity(
                    pool_addr,
                    U256::from(liquidity),
                    sqrt_price_x96,
                    fee
                ).await?;

                // Get 24h volume from subgraph
                let volume_24h = self.get_v3_volume(pool_addr).await?;

                // Only add pools with sufficient liquidity
                if total_liquidity > U256::from(50_000) * U256::exp10(18) { // $50k min liquidity
                    pools.push(DexPool {
                        address: pool_addr,
                        dex_type: DexType::UniswapV3,
                        tokens: vec![token], // Add paired token
                        liquidity_usd: total_liquidity,
                        volume_24h,
                    });
                }
            }
        }

        // Sort pools by liquidity
        pools.sort_by(|a, b| b.liquidity_usd.cmp(&a.liquidity_usd));

        Ok(pools)
    }

    /// Calculate Uniswap V3 pool liquidity in USD
    async fn calculate_v3_liquidity(
        &self,
        pool: Address,
        liquidity: U256,
        sqrt_price_x96: U256,
        fee: u32,
    ) -> Result<U256> {
        let client = Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?;
        let pool_contract = UniswapV3Pool::new(pool, client.clone());

        // Get tokens
        let token0 = pool_contract.token0().call().await?;
        let token1 = pool_contract.token1().call().await?;

        // Get token prices
        let price0 = self.get_token_price(token0).await?;
        let price1 = self.get_token_price(token1).await?;

        // Calculate amounts based on current tick
        let price = sqrt_price_x96
            .saturating_mul(sqrt_price_x96)
            .checked_div(U256::from(1u128 << 96))
            .ok_or_else(|| anyhow!("Price calculation overflow"))?;

        // Calculate token amounts from liquidity
        let amount0 = liquidity
            .saturating_mul(U256::exp10(18))
            .checked_div(sqrt_price_x96)
            .ok_or_else(|| anyhow!("Amount0 calculation overflow"))?;

        let amount1 = liquidity
            .saturating_mul(sqrt_price_x96)
            .checked_div(U256::from(1u128 << 96))
            .ok_or_else(|| anyhow!("Amount1 calculation overflow"))?;

        // Calculate total value in USD
        let value0 = amount0
            .saturating_mul(price0.price_usd)
            .checked_div(U256::exp10(price0.decimals as u32))
            .ok_or_else(|| anyhow!("Value0 calculation overflow"))?;

        let value1 = amount1
            .saturating_mul(price1.price_usd)
            .checked_div(U256::exp10(price1.decimals as u32))
            .ok_or_else(|| anyhow!("Value1 calculation overflow"))?;

        Ok(value0.saturating_add(value1))
    }

    /// Get 24h volume for Uniswap V3 pool from subgraph
    async fn get_v3_volume(&self, pool: Address) -> Result<U256> {
        // Query the Uniswap V3 subgraph
        let query = format!(
            r#"{{
                pool(id: "{:?}") {{
                    volumeUSD
                }}
            }}"#,
            pool
        );

        let client = reqwest::Client::new();
        let res = client
            .post("https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v3")
            .json(&json!({
                "query": query
            }))
            .send()
            .await?
            .json::<Value>()
            .await?;

        // Parse volume from response
        let volume = res
            .get("data")
            .and_then(|d| d.get("pool"))
            .and_then(|p| p.get("volumeUSD"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Failed to get volume from subgraph"))?;

        // Convert volume string to U256
        let volume_float: f64 = volume.parse()?;
        Ok(U256::from((volume_float * 1e18) as u64))
    }
}

#[derive(Debug, Clone)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
    Balancer,
    Curve,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DexPool {
    pub address: Address,
    pub dex_type: DexType,
    pub tokens: Vec<Address>,
    pub liquidity_usd: U256,
    pub volume_24h: U256,
}

impl SecurityManager {
    // ... existing methods ...

    /// Get TWAP from Uniswap V3 pool with extensive validation
    async fn get_v3_twap(&self, pool: &DexPool, token: Address) -> Result<Option<TWAPData>> {
        let client = Arc::new(Provider::<Http>::try_from("https://eth-mainnet.alchemyapi.io/v2/your-api-key")?);
        let pool_contract = UniswapV3Pool::new(pool.address, client.clone());

        // Get current state and validate pool health
        let (sqrt_price_x96, tick, _, _, _, fee_protocol, _) = pool_contract.slot0().call().await?;
        
        // Validate pool is active
        if sqrt_price_x96.is_zero() {
            return Ok(None); // Pool is not initialized
        }

        // Get pool tokens and validate
        let token0 = pool_contract.token0().call().await?;
        let token1 = pool_contract.token1().call().await?;
        let (base_token, quote_token) = if token == token0 {
            (token0, token1)
        } else if token == token1 {
            (token1, token0)
        } else {
            return Err(anyhow!("Token not found in pool"));
        };

        // Calculate time points for TWAP
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
        let seconds_ago: Vec<u32> = self.get_twap_observation_times(now)?;
        
        // Get observations
        let (ticks, initialized) = pool_contract.observe(seconds_ago.clone()).call().await?;
        
        // Validate observations
        if !self.validate_observations(&initialized)? {
            return Ok(None); // Not enough valid observations
        }

        // Calculate TWAP with cardinality checking
        let cardinality = pool_contract.observation_cardinality().call().await?;
        if cardinality < MIN_TWAP_CARDINALITY {
            return Ok(None); // Not enough historical data
        }

        // Process tick data
        let mut twap_sum = U256::zero();
        let mut valid_samples = 0;
        
        for i in 0..ticks.len()-1 {
            if !initialized[i] || !initialized[i+1] {
                continue;
            }

            let tick_diff = ticks[i+1] - ticks[i];
            let time_diff = seconds_ago[i+1] - seconds_ago[i];
            
            // Validate tick movement
            if !self.validate_tick_movement(tick_diff)? {
                continue;
            }

            let tick_avg = tick_diff.checked_div(time_diff as i64)
                .ok_or_else(|| anyhow!("TWAP calculation error"))?;
            
            // Convert tick to price
            let price = self.tick_to_sqrt_price(tick_avg)?;
            
            // Adjust price based on token order
            let adjusted_price = if token == token0 {
                price
            } else {
                U256::from(1u128 << 192).checked_div(price)
                    .ok_or_else(|| anyhow!("Price inversion overflow"))?
            };

            twap_sum = twap_sum.saturating_add(adjusted_price);
            valid_samples += 1;
        }

        if valid_samples < MIN_TWAP_SAMPLES {
            return Ok(None);
        }

        // Calculate final TWAP price
        let twap_price = twap_sum.checked_div(U256::from(valid_samples))
            .ok_or_else(|| anyhow!("TWAP average calculation error"))?;

        // Convert to USD if quote token is not USD-based
        let final_price = if self.is_usd_token(quote_token)? {
            twap_price
        } else {
            let quote_price = self.get_token_price(quote_token).await?;
            twap_price.saturating_mul(quote_price.price_usd)
                .checked_div(U256::exp10(18))
                .ok_or_else(|| anyhow!("USD conversion overflow"))?
        };

        Ok(Some(TWAPData {
            price: final_price,
            timestamp: now,
            samples: valid_samples as u32,
        }))
    }

    /// Get observation times for TWAP calculation
    fn get_twap_observation_times(&self, now: u64) -> Result<Vec<u32>> {
        // Default to 5-minute intervals over 30 minutes
        let intervals = vec![
            30 * 60,  // 30 minutes
            25 * 60,  // 25 minutes
            20 * 60,  // 20 minutes
            15 * 60,  // 15 minutes
            10 * 60,  // 10 minutes
            5 * 60,   // 5 minutes
            0,        // Current
        ];
        
        Ok(intervals)
    }

    /// Validate TWAP observations
    fn validate_observations(&self, initialized: &[bool]) -> Result<bool> {
        let valid_count = initialized.iter().filter(|&&x| x).count();
        
        // Require at least MIN_TWAP_SAMPLES valid observations
        if valid_count < MIN_TWAP_SAMPLES {
            return Ok(false);
        }
        
        // Check for gaps
        let max_gap = initialized.windows(2)
            .filter(|w| !w[0] || !w[1])
            .count();
        
        if max_gap > MAX_TWAP_GAPS {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Validate tick movement is within bounds
    fn validate_tick_movement(&self, tick_diff: i64) -> Result<bool> {
        const MAX_TICK_MOVEMENT: i64 = 1000; // About 10% price movement
        
        if tick_diff.abs() > MAX_TICK_MOVEMENT {
            return Ok(false);
        }
        
        Ok(true)
    }

    /// Convert tick to sqrt price
    fn tick_to_sqrt_price(&self, tick: i64) -> Result<U256> {
        // Price = 1.0001^tick
        let sqrt_price = (1.0001f64.powi(tick as i32) * (1u128 << 96) as f64) as u128;
        Ok(U256::from(sqrt_price))
    }

    /// Check if token is USD-based
    fn is_usd_token(&self, token: Address) -> Result<bool> {
        const USD_TOKENS: [&str; 3] = [
            "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", // USDC
            "dAC17F958D2ee523a2206206994597C13D831ec7", // USDT
            "6B175474E89094C44Da98b954EedeAC495271d0F", // DAI
        ];
        
        Ok(USD_TOKENS.iter().any(|&addr| {
            token == Address::from_slice(&hex::decode(addr).unwrap())
        }))
    }

    /// Constants for TWAP calculations
    const MIN_TWAP_SAMPLES: usize = 3;
    const MIN_TWAP_CARDINALITY: u16 = 50;
    const MAX_TWAP_GAPS: usize = 2;
}

/// Price source with weight and timestamp
#[derive(Debug, Clone)]
pub struct PriceSource {
    pub price: U256,
    pub weight: u32,
    pub timestamp: u64,
    pub source: String,
}

/// Token validation result
#[derive(Debug, Clone)]
pub struct TokenValidation {
    pub is_valid: bool,
    pub has_transfer_fee: bool,
    pub has_transfer_restrictions: bool,
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
struct VolumeData {
    volume_24h: U256,
    sources: Vec<(&'static str, U256)>,
    last_updated: u64,
}

/// Holder data with unique holders, top holders, and concentration
#[derive(Debug)]
struct HolderData {
    unique_holders: usize,
    top_holders: Vec<(Address, U256)>,
    concentration: U256, // Percentage held by top 10 holders
    last_updated: u64,
}

/// Contract data with creation timestamp, verification status, and source code hash
#[derive(Debug)]
struct ContractData {
    created_at: u64,
    is_verified: bool,
    source_code_hash: String,
    malicious_patterns: Vec<String>,
    last_updated: u64,
}
