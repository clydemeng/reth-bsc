pub mod chainspec;
pub mod cli;
pub mod consensus;
mod evm;
mod hardforks;
pub mod node;
pub use node::primitives::BscPrimitives;
// Re-export the BSC-specific block types so modules can `use crate::{BscBlock, BscBlockBody, …}`
pub use node::primitives::{BscBlock, BscBlockBody, BscBlobTransactionSidecar};
mod system_contracts;
pub use system_contracts::SLASH_CONTRACT;
pub mod snapshot_pool;
pub mod execution_outcome;
#[path = "system_contracts/tx_maker_ext.rs"]
mod system_tx_ext;
// pub use system_tx_ext::*; // keep module private for now to avoid unused import warning
