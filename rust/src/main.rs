use anyhow::Result;
use ethers::{
    providers::{Provider, Ws},
    types::Address,
    signers::LocalWallet,
};
use log::{info, error, warn};
use std::sync::Arc;
use tokio::sync::broadcast::{self, Sender};
use tokio::task::JoinSet;
use prometheus::default_registry;
use warp::Filter;

use rust::{
    constants::Env,
    strategy::event_handler,
    streams::{stream_new_blocks, stream_pending_transactions, stream_uniswap_v2_events, Event},
    utils::setup_logger,
    flashbot::{
        arbitrage::ArbitrageManager,
        mev_protection::MEVProtection,
        contracts::ContractManager,
        market_maker::MarketMaker,
        types::{RiskConfig, ExecutionConfig},
    },
    security::SecurityManager,
    dex::DexManager,
    monitoring::{Metrics, HealthChecker, ErrorRecovery},
    config::{BotConfig, RuntimeConfig},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment and logging
    dotenv::dotenv().ok();
    setup_logger()?;

    // Load and validate configurations
    let config = load_config()?;
    config.validate_all()?;
    
    let runtime_config = RuntimeConfig::default();

    // Initialize metrics and monitoring
    let metrics = Arc::new(Metrics::new()?);
    let health_checker = Arc::new(HealthChecker::new(metrics.clone()));
    let error_recovery = Arc::new(ErrorRecovery::new(
        metrics.clone(),
        runtime_config.retry_attempts,
        std::time::Duration::from_millis(runtime_config.backoff_base_ms),
    ));

    // Setup provider and wallet
    let ws = error_recovery
        .retry_with_backoff(|| Ws::connect(&config.rpc_url))
        .await?;
    let provider = Arc::new(Provider::new(ws));
    let wallet = LocalWallet::from_bytes(&hex::decode(&config.private_key)?)?;

    // Initialize core components
    let security_manager = Arc::new(SecurityManager::new(provider.clone()));
    let dex_manager = Arc::new(DexManager::new(provider.clone()));

    // Initialize flashbot components with validated config
    let arbitrage_manager = Arc::new(ArbitrageManager::new(
        dex_manager.clone(),
        security_manager.clone(),
        config.into(),
        config.into(),
    ));

    let mev_protection = Arc::new(MEVProtection::new(
        config.flashbots_rpc.unwrap_or_default(),
        config.eden_rpc,
        None,
        U256::from(config.priority_fee),
    ));

    let contract_manager = Arc::new(ContractManager::new(
        provider.clone(),
        config.executor_address,
        config.vault_address,
    ).await?);

    let market_maker = if config.market_making_enabled {
        Some(Arc::new(MarketMaker::new(
            config.max_position_size,
            config.rebalance_threshold,
            config.min_spread_bps,
        )))
    } else {
        None
    };

    // Setup event channels
    let (event_sender, _): (Sender<Event>, _) = broadcast::channel(512);
    let mut set = JoinSet::new();

    // Spawn monitoring tasks
    spawn_monitoring_tasks(
        &mut set,
        health_checker.clone(),
        metrics.clone(),
        runtime_config.clone(),
    );

    // Spawn core streams with error recovery
    spawn_core_streams(
        &mut set,
        provider.clone(),
        event_sender.clone(),
        error_recovery.clone(),
    );
    
    // Spawn arbitrage handler with monitoring
    spawn_arbitrage_handler(
        &mut set,
        arbitrage_manager.clone(),
        mev_protection.clone(),
        contract_manager.clone(),
        wallet.clone(),
        event_sender.clone(),
        metrics.clone(),
        error_recovery.clone(),
    );

    // Spawn market maker if enabled
    if let Some(market_maker) = market_maker {
        spawn_market_maker(
            &mut set,
            market_maker,
            metrics.clone(),
            error_recovery.clone(),
        );
    }

    // Start metrics server
    let metrics_route = warp::path!("metrics").map(move || {
        let encoder = prometheus::TextEncoder::new();
        let mut buffer = Vec::new();
        encoder.encode(&default_registry().gather(), &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    });

    tokio::spawn(warp::serve(metrics_route).run(([127, 0, 0, 1], runtime_config.metrics_port)));

    // Wait for tasks and handle failures
    while let Some(res) = set.join_next().await {
        match res {
            Ok(_) => continue,
            Err(e) => {
                error!("Task error: {}", e);
                error_recovery.handle_error(e, "Task failure").await;
                
                // Check health status
                if !health_checker.is_healthy().await {
                    warn!("System unhealthy, attempting recovery...");
                    // Implement recovery logic
                }
            }
        }
    }

    Ok(())
}

fn spawn_monitoring_tasks(
    set: &mut JoinSet<Result<()>>,
    health_checker: Arc<HealthChecker>,
    metrics: Arc<Metrics>,
    config: RuntimeConfig,
) {
    // Health check task
    set.spawn({
        let health_checker = health_checker.clone();
        async move {
            loop {
                if let Err(e) = health_checker.check_health().await {
                    error!("Health check failed: {}", e);
                }
                tokio::time::sleep(config.health_check_interval).await;
            }
        }
    });

    // Memory monitoring task
    set.spawn({
        let metrics = metrics.clone();
        async move {
            loop {
                let memory = sys_info::mem_info().unwrap_or_default();
                metrics.memory_usage.set(memory.total as f64);
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        }
    });
}

fn spawn_core_streams(
    set: &mut JoinSet<Result<()>>,
    provider: Arc<Provider<Ws>>,
    event_sender: Sender<Event>,
    error_recovery: Arc<ErrorRecovery>,
) {
    // Block stream with error recovery
    set.spawn({
        let provider = provider.clone();
        let event_sender = event_sender.clone();
        let error_recovery = error_recovery.clone();
        async move {
            error_recovery.retry_with_backoff(|| {
                stream_new_blocks(provider.clone(), event_sender.clone())
            }).await
        }
    });

    // Transaction stream with error recovery
    set.spawn({
        let provider = provider.clone();
        let event_sender = event_sender.clone();
        let error_recovery = error_recovery.clone();
        async move {
            error_recovery.retry_with_backoff(|| {
                stream_pending_transactions(provider.clone(), event_sender.clone())
            }).await
        }
    });
}

fn spawn_arbitrage_handler(
    set: &mut JoinSet<Result<()>>,
    arbitrage_manager: Arc<ArbitrageManager>,
    mev_protection: Arc<MEVProtection>,
    contract_manager: Arc<ContractManager>,
    wallet: LocalWallet,
    event_sender: Sender<Event>,
    metrics: Arc<Metrics>,
    error_recovery: Arc<ErrorRecovery>,
) {
    set.spawn({
        async move {
            let mut rx = event_sender.subscribe();
            while let Ok(event) = rx.recv().await {
                match event {
                    Event::NewBlock(block) => {
                        metrics.last_block_time.set(block.timestamp.as_u64() as f64);
                        
                        // Look for arbitrage opportunities
                        match arbitrage_manager.find_opportunities(block.hash).await {
                            Ok(opportunities) => {
                                metrics.opportunities_found.inc_by(opportunities.len() as f64);
                                
                                for op in opportunities {
                                    let start_time = std::time::Instant::now();
                                    
                                    // Check MEV protection
                                    if !mev_protection.check_sandwich_risk(&op.path).await? {
                                        // Execute arbitrage through contracts
                                        match error_recovery
                                            .retry_with_backoff(|| {
                                                arbitrage_manager.execute_arbitrage(&op, wallet.clone())
                                            })
                                            .await
                                        {
                                            Ok(result) => {
                                                metrics.trades_executed.inc();
                                                metrics.total_profit.add(result.actual_profit.as_u64() as f64);
                                                metrics.execution_time.observe(
                                                    start_time.elapsed().as_millis() as f64
                                                );
                                            }
                                            Err(e) => {
                                                error_recovery.handle_error(e, "Arbitrage execution failed").await;
                                            }
                                        }
                                    } else {
                                        metrics.sandwich_attempts.inc();
                                    }
                                }
                            }
                            Err(e) => error_recovery.handle_error(e, "Finding opportunities failed").await,
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    });
}

fn spawn_market_maker(
    set: &mut JoinSet<Result<()>>,
    market_maker: Arc<MarketMaker>,
    metrics: Arc<Metrics>,
    error_recovery: Arc<ErrorRecovery>,
) {
    set.spawn({
        async move {
            loop {
                for token in market_maker.get_managed_tokens().await? {
                    if let Err(e) = error_recovery
                        .retry_with_backoff(|| market_maker.calculate_spread(token))
                        .await
                    {
                        error_recovery.handle_error(e, "Market making failed").await;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        }
    });
}
