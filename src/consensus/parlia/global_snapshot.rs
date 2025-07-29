use std::sync::{Arc, OnceLock};

use crate::consensus::parlia::validator::SnapshotProvider;

static SNAPSHOT_PROVIDER: OnceLock<Arc<dyn SnapshotProvider + Send + Sync>> = OnceLock::new();

/// Register the global snapshot provider. Safe to call multiple times; only the
/// first call sets the value.
pub fn set(p: Arc<dyn SnapshotProvider + Send + Sync>) {
    let _ = SNAPSHOT_PROVIDER.set(p);
}

/// Returns the global snapshot provider.
pub fn get() -> &'static dyn SnapshotProvider {
    SNAPSHOT_PROVIDER
        .get()
        .expect("snapshot provider not initialized")
        .as_ref()
} 