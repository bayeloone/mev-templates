use anyhow::{Ok, Result};
use cfmms::{
    dex::{Dex, DexVariant as CfmmsDexVariant},
    pool::Pool as CfmmsPool,
    sync::sync_pairs,
};
use csv::StringRecord;
use ethers::{
    providers::{Provider, Ws},
    types::{H160, U256},
};
use log::info;
use std::{path::Path, str::FromStr, sync::Arc};

#[derive(Debug, Clone)]
pub enum DexVariant {
    UniswapV2,
    UniswapV3,
}

#[derive(Debug, Clone)]
pub struct Pool {
    pub address: H160,
    pub version: DexVariant,
    pub token0: H160,
    pub token1: H160,
    pub decimals0: u8,
    pub decimals1: u8,
    pub fee: u32,
    pub reserve0: U256,
    pub reserve1: U256,
}

impl From<StringRecord> for Pool {
    fn from(record: StringRecord) -> Self {
        let version = if record.get(1).unwrap() == "2" {
            DexVariant::UniswapV2
        } else {
            DexVariant::UniswapV3
        };
        Self {
            address: H160::from_str(record.get(0).unwrap()).unwrap(),
            version,
            token0: H160::from_str(record.get(2).unwrap()).unwrap(),
            token1: H160::from_str(record.get(3).unwrap()).unwrap(),
            decimals0: record.get(4).unwrap().parse().unwrap(),
            decimals1: record.get(5).unwrap().parse().unwrap(),
            fee: record.get(6).unwrap().parse().unwrap(),
            reserve0: U256::zero(),
            reserve1: U256::zero(),
        }
    }
}

impl Pool {
    pub fn cache_row(&self) -> (String, i32, String, String, u8, u8, u32) {
        (
            format!("{:?}", self.address),
            match self.version {
                DexVariant::UniswapV2 => 2,
                DexVariant::UniswapV3 => 3,
            },
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.decimals0,
            self.decimals1,
            self.fee,
        )
    }

    pub fn get_liquidity_usd(&self) -> U256 {
        // USDC address on Ethereum mainnet
        let usdc = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        
        // Case 1: Direct USDC pair
        // If one of the tokens is USDC, we can directly use its reserve
        // USDC has 6 decimals, so reserve0 or reserve1 * 10^6 = USD value
        if self.token0 == usdc {
            self.reserve0 * U256::from(10).pow(U256::from(6))
        } else if self.token1 == usdc {
            self.reserve1 * U256::from(10).pow(U256::from(6))
        } else {
            // Case 2: ETH pair
            // For ETH pairs, we use ETH price to calculate USD value
            let weth = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
            let eth_price = U256::from(1500); // ETH price in USD
            
            if self.token0 == weth {
                // Convert ETH amount to USD: reserve0 * ETH_price
                // Adjust for 18 decimals of ETH
                self.reserve0 * eth_price
            } else if self.token1 == weth {
                self.reserve1 * eth_price
            } else {
                // Case 3: Other token pairs
                // For other pairs, we need price oracle data
                // Currently returning 0 as we can't determine price
                // In production, you should:
                // 1. Use Chainlink price feeds
                // 2. Or calculate using common pairs (USDC/token, ETH/token)
                // 3. Or skip these pairs entirely
                U256::zero()
            }
        }
    }
}

// Example thresholds for different risk levels
pub const LOW_LIQUIDITY_THRESHOLD: U256 = U256([1_000_000_000_000, 0, 0, 0]);     // $1,000
pub const MEDIUM_LIQUIDITY_THRESHOLD: U256 = U256([10_000_000_000_000, 0, 0, 0]); // $10,000
pub const HIGH_LIQUIDITY_THRESHOLD: U256 = U256([100_000_000_000_000, 0, 0, 0]);  // $100,000

pub async fn load_all_pools_from_v2(
    wss_url: String,
    factory_addresses: Vec<&str>,
    from_blocks: Vec<u64>,
) -> Result<Vec<Pool>> {
    // Load from cached file if the file exists
    let file_path = Path::new("src/.cached-pools.csv");
    if file_path.exists() {
        let mut reader = csv::Reader::from_path(file_path)?;

        let mut pools_vec: Vec<Pool> = Vec::new();
        for row in reader.records() {
            let row = row.unwrap();
            let pool = Pool::from(row);
            pools_vec.push(pool);
        }
        return Ok(pools_vec);
    }

    let ws = Ws::connect(wss_url).await?;
    let provider = Arc::new(Provider::new(ws));

    let mut dexes_data = Vec::new();

    for i in 0..factory_addresses.len() {
        dexes_data.push((
            factory_addresses[i].clone(),
            CfmmsDexVariant::UniswapV2,
            from_blocks[i],
        ))
    }

    let dexes: Vec<_> = dexes_data
        .into_iter()
        .map(|(address, variant, number)| {
            Dex::new(
                H160::from_str(&address).unwrap(),
                variant,
                number,
                Some(3000),
            )
        })
        .collect();

    let pools_vec: Vec<CfmmsPool> = sync_pairs(dexes.clone(), provider.clone(), None).await?;
    let pools_vec: Vec<Pool> = pools_vec
        .into_iter()
        .map(|pool| match pool {
            CfmmsPool::UniswapV2(pool) => Pool {
                address: pool.address,
                version: DexVariant::UniswapV2,
                token0: pool.token_a,
                token1: pool.token_b,
                decimals0: pool.token_a_decimals,
                decimals1: pool.token_b_decimals,
                fee: pool.fee,
                reserve0: pool.reserve_a,
                reserve1: pool.reserve_b,
            },
            CfmmsPool::UniswapV3(pool) => Pool {
                address: pool.address,
                version: DexVariant::UniswapV3,
                token0: pool.token_a,
                token1: pool.token_b,
                decimals0: pool.token_a_decimals,
                decimals1: pool.token_b_decimals,
                fee: pool.fee,
                reserve0: pool.reserve_a,
                reserve1: pool.reserve_b,
            },
        })
        .collect();
    info!("Synced to {} pools", pools_vec.len());

    let mut writer = csv::Writer::from_path(file_path)?;
    writer.write_record(&[
        "address",
        "version",
        "token0",
        "token1",
        "decimals0",
        "decimals1",
        "fee",
    ])?;

    for pool in &pools_vec {
        writer.serialize(pool.cache_row())?;
    }
    writer.flush()?;

    Ok(pools_vec)
}
