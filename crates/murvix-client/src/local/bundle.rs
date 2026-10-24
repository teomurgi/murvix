use std::io;
use std::path::Path;

pub use murvix_core::bundle::{BundleLease, lock_replacement, lock_unused};

pub fn acquire_runtime(executable: &Path) -> io::Result<Option<BundleLease>> {
    murvix_core::bundle::acquire_runtime(
        executable,
        &serde_json::to_vec(&murvix_protocol::BuildInfo::current())?,
    )
}
