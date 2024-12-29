use ethers_core::abi::Abi;
use std::fs;
use anyhow::Result;

pub struct ABI {
    // Core protocol ABIs
    pub erc20: Abi,
    pub weth: Abi,
    
    // DEX ABIs
    pub uniswap_v2_factory: Abi,
    pub uniswap_v2_pair: Abi,
    pub uniswap_v3_factory: Abi,
    pub uniswap_v3_pool: Abi,
    pub curve_pool: Abi,
    pub balancer_vault: Abi,
    
    // Lending protocol ABIs
    pub aave_lending_pool: Abi,
    pub compound_ctoken: Abi,
    pub dydx_solo_margin: Abi,
    
    // Flashloan bot ABIs
    pub flashloan_executor: Abi,
    pub vault: Abi,
    pub market_maker: Abi,
    pub v2_arb_bot: Abi,
    
    // Protocol adapters
    pub dex_adapter: Abi,
    pub lending_adapter: Abi,
    
    // Safety modules
    pub emergency_stop: Abi,
    pub access_control: Abi,
}

impl ABI {
    pub fn new() -> Result<Self> {
        Ok(Self {
            // Load core ABIs
            erc20: Self::load_abi("ERC20.json")?,
            weth: Self::load_abi("WETH.json")?,
            
            // Load DEX ABIs
            uniswap_v2_factory: Self::load_abi("UniswapV2Factory.json")?,
            uniswap_v2_pair: Self::load_abi("UniswapV2Pair.json")?,
            uniswap_v3_factory: Self::load_abi("UniswapV3Factory.json")?,
            uniswap_v3_pool: Self::load_abi("UniswapV3Pool.json")?,
            curve_pool: Self::load_abi("CurvePool.json")?,
            balancer_vault: Self::load_abi("BalancerVault.json")?,
            
            // Load lending ABIs
            aave_lending_pool: Self::load_abi("AaveLendingPool.json")?,
            compound_ctoken: Self::load_abi("CompoundCToken.json")?,
            dydx_solo_margin: Self::load_abi("DydxSoloMargin.json")?,
            
            // Load bot ABIs
            flashloan_executor: Self::load_abi("FlashloanExecutor.json")?,
            vault: Self::load_abi("Vault.json")?,
            market_maker: Self::load_abi("MarketMaker.json")?,
            v2_arb_bot: Self::load_abi("V2ArbBot.json")?,
            
            // Load adapter ABIs
            dex_adapter: Self::load_abi("DexAdapter.json")?,
            lending_adapter: Self::load_abi("LendingAdapter.json")?,
            
            // Load safety ABIs
            emergency_stop: Self::load_abi("EmergencyStop.json")?,
            access_control: Self::load_abi("AccessControl.json")?,
        })
    }

    fn load_abi(filename: &str) -> Result<Abi> {
        let path = format!("src/abi/{}", filename);
        let json = fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read ABI file {}: {}", path, e))?;
            
        serde_json::from_str(&json)
            .map_err(|e| anyhow::anyhow!("Failed to parse ABI {}: {}", path, e))
    }

    /// Get ABI for a specific DEX protocol
    pub fn get_dex_abi(&self, protocol: &str) -> Result<&Abi> {
        match protocol {
            "uniswap_v2" => Ok(&self.uniswap_v2_pair),
            "uniswap_v3" => Ok(&self.uniswap_v3_pool),
            "curve" => Ok(&self.curve_pool),
            "balancer" => Ok(&self.balancer_vault),
            _ => Err(anyhow::anyhow!("Unknown DEX protocol: {}", protocol)),
        }
    }

    /// Get ABI for a lending protocol
    pub fn get_lending_abi(&self, protocol: &str) -> Result<&Abi> {
        match protocol {
            "aave" => Ok(&self.aave_lending_pool),
            "compound" => Ok(&self.compound_ctoken),
            "dydx" => Ok(&self.dydx_solo_margin),
            _ => Err(anyhow::anyhow!("Unknown lending protocol: {}", protocol)),
        }
    }
}
