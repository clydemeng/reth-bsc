use crate::{
    evm::{
        api::{BscContext, BscEvm},
        transaction::BscTxEnv,
    },
    hardforks::bsc::BscHardfork,
};
use alloy_primitives::{Address, Bytes};

use reth::{
    api::{FullNodeTypes, NodeTypes},
    builder::{components::ExecutorBuilder, BuilderContext},
};
use reth_evm::{precompiles::PrecompilesMap, Database, Evm, EvmEnv};
use revm::{
    context::{
        result::{EVMError, HaltReason, ResultAndState},
        BlockEnv,
    },
    Context, ExecuteEvm, InspectEvm, Inspector,
};

mod assembler;
pub mod config;
pub use config::BscEvmConfig;
mod executor;
mod factory;
mod patch;

impl<DB, I> Evm for BscEvm<DB, I>
where
    DB: Database,
    I: Inspector<BscContext<DB>>,
{
    type DB = DB;
    type Tx = BscTxEnv;
    type Error = EVMError<DB::Error>;
    type HaltReason = HaltReason;
    type Spec = BscHardfork;
    type Precompiles = PrecompilesMap;
    type Inspector = I;

    fn chain_id(&self) -> u64 {
        self.cfg.chain_id
    }

    fn block(&self) -> &BlockEnv {
        &self.block
    }

    fn transact_raw(
        &mut self,
        tx: Self::Tx,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        if self.inspect {
            self.inspect_tx(tx)
        } else if tx.is_system_transaction {
            let mut gas_limit = tx.base.gas_limit;
            let mut basefee = 0;
            let mut disable_nonce_check = true;

            // ensure the block gas limit is >= the tx
            core::mem::swap(&mut self.block.gas_limit, &mut gas_limit);
            // disable the base fee check for this call by setting the base fee to zero
            core::mem::swap(&mut self.block.basefee, &mut basefee);
            // disable the nonce check
            core::mem::swap(&mut self.cfg.disable_nonce_check, &mut disable_nonce_check);
            let res = ExecuteEvm::transact(self, tx);

            // swap back to the previous gas limit
            core::mem::swap(&mut self.block.gas_limit, &mut gas_limit);
            // swap back to the previous base fee
            core::mem::swap(&mut self.block.basefee, &mut basefee);
            // swap back to the previous nonce check flag
            core::mem::swap(&mut self.cfg.disable_nonce_check, &mut disable_nonce_check);
            res
        } else {
            ExecuteEvm::transact(self, tx)
        }
    }

    fn transact_system_call(
        &mut self,
        _caller: Address,
        _contract: Address,
        _data: Bytes,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        unimplemented!()
    }

    fn db_mut(&mut self) -> &mut Self::DB {
        &mut self.journaled_state.database
    }

    fn finish(self) -> (Self::DB, EvmEnv<Self::Spec>) {
        let Context { block: block_env, cfg: cfg_env, journaled_state, .. } = self.inner.ctx;

        (journaled_state.database, EvmEnv { block_env, cfg_env })
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        self.inspect = enabled;
    }

    fn precompiles_mut(&mut self) -> &mut Self::Precompiles {
        &mut self.inner.precompiles
    }

    fn inspector_mut(&mut self) -> &mut Self::Inspector {
        &mut self.inner.inspector
    }

    fn precompiles(&self) -> &Self::Precompiles {
        &self.inner.precompiles
    }

    fn inspector(&self) -> &Self::Inspector {
        &self.inner.inspector
    }
}

/// A regular bsc evm and executor builder.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BscExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for BscExecutorBuilder
where
    Node: FullNodeTypes,
    Node::Types: NodeTypes<Primitives = crate::node::primitives::BscPrimitives, ChainSpec = crate::chainspec::BscChainSpec, Payload = crate::node::rpc::engine_api::payload::BscPayloadTypes, StateCommitment = reth_trie_db::MerklePatriciaTrie, Storage = crate::node::storage::BscStorage>,
{
    type EVM = BscEvmConfig;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = BscEvmConfig::bsc(ctx.chain_spec());
        Ok(evm_config)
    }
}
