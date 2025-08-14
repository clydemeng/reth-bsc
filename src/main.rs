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
            // ComponentsBuilder will call BscConsensusBuilder to build the consensus.
            (BscEvmConfig::new(spec.clone()), NoopConsensus::arc())
        },
        async move |builder, _| {
            // Create node with proper engine handle communication (matches official BSC)
            let (node, engine_handle_tx) = BscNode::new();
            
            let NodeHandle { node, node_exit_future: exit_future } =
                builder.node(node)
                    .extend_rpc_modules(move |ctx| {
                        tracing::info!("Start to Register Parlia RPC API: parlia_getSnapshot");
                        use reth_bsc::rpc::parlia::{ParliaApiImpl, ParliaApiServer, DynSnapshotProvider};
                        
                        let snapshot_provider = if let Some(provider) = reth_bsc::shared::get_snapshot_provider() {
                            tracing::info!("Using shared persistent snapshot provider from consensus builder");
                            provider.clone()
                        } else {
                            tracing::error!("Shared snapshot provider not available, using fallback");
                            use reth_bsc::consensus::parlia::{InMemorySnapshotProvider, SnapshotProvider};
                            Arc::new(InMemorySnapshotProvider::new(1000)) as Arc<dyn SnapshotProvider + Send + Sync>
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

            exit_future.await
        },
    )?;
    Ok(())
}