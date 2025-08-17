use clap::{Args, Parser};
use reth::{builder::NodeHandle, cli::Cli, consensus::noop::NoopConsensus};
use reth_bsc::{
    chainspec::parser::BscChainSpecParser,
    node::{evm::config::BscEvmConfig, BscNode},
};
use std::sync::Arc;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// No Additional arguments
#[derive(Debug, Clone, Copy, Default, Args)]
#[non_exhaustive]
struct NoArgs;

fn main() -> eyre::Result<()> {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    Cli::<BscChainSpecParser, NoArgs>::parse().run_with_components::<BscNode>(
        |spec| {
            // ComponentsBuilder BscConsensusBuilder automatically overwrite it to ParliaConsensus.
            (BscEvmConfig::new(spec.clone()), NoopConsensus::arc())
        },
        async move |builder, _| {
            // Create node with proper engine handle communication (matches official BSC)
            let (node, engine_handle_tx) = BscNode::new();
            
            let NodeHandle { node, node_exit_future: exit_future } =
                builder.node(node)
                    .extend_rpc_modules(move |ctx| {
                        tracing::info!("Start to register Parlia RPC API: parlia_getSnapshot");
                        use reth_bsc::rpc::parlia::{ParliaApiImpl, ParliaApiServer, DynSnapshotProvider};
                        
                        let snapshot_provider = if let Some(provider) = reth_bsc::shared::get_snapshot_provider() {
                            provider.clone()
                        } else {
                            tracing::error!("Failed to register Parlia RPC due to can not get snapshot provider");
                            return Err(eyre::eyre!("Failed to get snapshot provider"));
                        };
                        
                        let wrapped_provider = Arc::new(DynSnapshotProvider::new(snapshot_provider));
                        let parlia_api = ParliaApiImpl::new(wrapped_provider);
                        ctx.modules.merge_configured(parlia_api.into_rpc())?;

                        tracing::info!("Succeed to register Parlia RPC API");
                        Ok(())
                    })
                    .launch().await?;

            // Send the engine handle to the network
            engine_handle_tx.send(node.beacon_engine_handle.clone()).unwrap();
            
            // Start header cache monitoring (optional - for performance debugging)
            tokio::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    let (cache_size, cache_capacity) = reth_bsc::consensus::parlia::header_cache::cache_stats();
                    let cache_usage_pct = (cache_size as f64 / cache_capacity as f64) * 100.0;
                    tracing::info!("🗂️  Header cache stats: {}/{} ({:.1}%) - Cache hit ratio should improve performance", 
                                 cache_size, cache_capacity, cache_usage_pct);
                }
            });

            exit_future.await
        },
    )?;
    Ok(())
}