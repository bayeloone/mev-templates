use ethers::types::{Address, U256};
use lazy_static::lazy_static;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StargatePoolConfig {
    pub pool_id: U256,
    pub decimals: u8,
    pub version: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub router_address: Address,
    pub factory_address: Address,
    pub pools: HashMap<Address, StargatePoolConfig>,
}

lazy_static! {
    // Mainnet Pools
    pub static ref MAINNET_POOLS: HashMap<Address, StargatePoolConfig> = {
        let mut m = HashMap::new();
        // USDC
        m.insert(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(1),
                decimals: 6,
                version: 1,
            }
        );
        // USDT
        m.insert(
            "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(2),
                decimals: 6,
                version: 1,
            }
        );
        // DAI
        m.insert(
            "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(3),
                decimals: 18,
                version: 1,
            }
        );
        // FRAX
        m.insert(
            "0x853d955aCEf822Db058eb8505911ED77F175b99e".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(7),
                decimals: 18,
                version: 1,
            }
        );
        m
    };

    // Polygon Pools
    pub static ref POLYGON_POOLS: HashMap<Address, StargatePoolConfig> = {
        let mut m = HashMap::new();
        // USDC
        m.insert(
            "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(1),
                decimals: 6,
                version: 1,
            }
        );
        // USDT
        m.insert(
            "0xc2132D05D31c914a87C6611C10748AEb04B58e8F".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(2),
                decimals: 6,
                version: 1,
            }
        );
        // DAI
        m.insert(
            "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(3),
                decimals: 18,
                version: 1,
            }
        );
        m
    };

    // Arbitrum Pools
    pub static ref ARBITRUM_POOLS: HashMap<Address, StargatePoolConfig> = {
        let mut m = HashMap::new();
        // USDC
        m.insert(
            "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(1),
                decimals: 6,
                version: 1,
            }
        );
        // USDT
        m.insert(
            "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(2),
                decimals: 6,
                version: 1,
            }
        );
        // FRAX
        m.insert(
            "0x17FC002b466eEc40DaE837Fc4bE5c67993ddBd6F".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(7),
                decimals: 18,
                version: 1,
            }
        );
        m
    };

    // Optimism Pools
    pub static ref OPTIMISM_POOLS: HashMap<Address, StargatePoolConfig> = {
        let mut m = HashMap::new();
        // USDC
        m.insert(
            "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(1),
                decimals: 6,
                version: 1,
            }
        );
        // DAI
        m.insert(
            "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(3),
                decimals: 18,
                version: 1,
            }
        );
        // FRAX
        m.insert(
            "0x2E3D870790dC77A83DD1d18184Acc7439A53f475".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(7),
                decimals: 18,
                version: 1,
            }
        );
        m
    };

    // Base Pools
    pub static ref BASE_POOLS: HashMap<Address, StargatePoolConfig> = {
        let mut m = HashMap::new();
        // USDC
        m.insert(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".parse().unwrap(),
            StargatePoolConfig {
                pool_id: U256::from(1),
                decimals: 6,
                version: 1,
            }
        );
        m
    };

    // Chain Configurations
    pub static ref CHAIN_CONFIGS: HashMap<u64, ChainConfig> = {
        let mut m = HashMap::new();
        // Ethereum Mainnet (1)
        m.insert(1, ChainConfig {
            chain_id: 1,
            router_address: "0x8731d54E9D02c286767d56ac03e8037C07e01e98".parse().unwrap(),
            factory_address: "0x06D538690AF257Da524f25D0CD52fD85b1c2173E".parse().unwrap(),
            pools: MAINNET_POOLS.clone(),
        });
        // Polygon (137)
        m.insert(137, ChainConfig {
            chain_id: 137,
            router_address: "0x45A01E4e04F14f7A4a6702c74187c5F6222033cd".parse().unwrap(),
            factory_address: "0x808d7c71ad2ba3FA531b068a2417C63106BC0949".parse().unwrap(),
            pools: POLYGON_POOLS.clone(),
        });
        // Arbitrum (42161)
        m.insert(42161, ChainConfig {
            chain_id: 42161,
            router_address: "0x53Bf833A5d6c4ddA888F69c22C88C9f356a41614".parse().unwrap(),
            factory_address: "0x55bDb4164D28FBaF0898e0eF14a589ac09Ac9970".parse().unwrap(),
            pools: ARBITRUM_POOLS.clone(),
        });
        // Optimism (10)
        m.insert(10, ChainConfig {
            chain_id: 10,
            router_address: "0xB0D502E938ed5f4df2E681fE6E419ff29631d62b".parse().unwrap(),
            factory_address: "0xE3B53AF74a4BF62Ae5511055290838050bf764Df".parse().unwrap(),
            pools: OPTIMISM_POOLS.clone(),
        });
        // Base (8453)
        m.insert(8453, ChainConfig {
            chain_id: 8453,
            router_address: "0x45f1A95A4D3f3836523F5c83673c797f4d4d263B".parse().unwrap(),
            factory_address: "0x115335Eb24c14e6E4fE2Bd8B51a6722c6F2125B8".parse().unwrap(),
            pools: BASE_POOLS.clone(),
        });
        m
    };
}

pub fn get_pool_config(chain_id: u64, token: Address) -> Option<&'static StargatePoolConfig> {
    CHAIN_CONFIGS.get(&chain_id)?.pools.get(&token)
}

pub fn get_router_address(chain_id: u64) -> Option<Address> {
    Some(CHAIN_CONFIGS.get(&chain_id)?.router_address)
}

pub fn get_factory_address(chain_id: u64) -> Option<Address> {
    Some(CHAIN_CONFIGS.get(&chain_id)?.factory_address)
}

pub fn is_supported_chain(chain_id: u64) -> bool {
    CHAIN_CONFIGS.contains_key(&chain_id)
}

pub fn is_supported_token(chain_id: u64, token: Address) -> bool {
    if let Some(config) = CHAIN_CONFIGS.get(&chain_id) {
        config.pools.contains_key(&token)
    } else {
        false
    }
}
