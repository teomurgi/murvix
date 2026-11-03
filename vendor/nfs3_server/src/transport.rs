//! Murvix patch only: public generic stream transport for Unix-domain NFS connections.

use std::sync::Arc;
use std::time::Duration;

use crate::context::RPCContext;
use crate::transaction_tracker::TransactionTracker;
use crate::vfs::NfsFileSystem;
use crate::vfs::handle::FileHandleConverter;

/// Handles a single NFS RPC connection over an already-established stream.
pub struct Connection<T: NfsFileSystem + 'static> {
    context: RPCContext<T>,
}

impl<T: NfsFileSystem + 'static> Connection<T> {
    /// Creates a connection context for an exported filesystem.
    #[must_use]
    pub fn new(export_name: &str, vfs: Arc<T>) -> Self {
        Self {
            context: RPCContext {
                local_port: 0,
                client_addr: "unix".to_owned(),
                auth: nfs3_types::rpc::auth_unix::default(),
                vfs,
                mount_signal: None,
                export_name: Arc::new(format!(
                    "/{}",
                    export_name.trim_end_matches('/').trim_start_matches('/')
                )),
                transaction_tracker: Arc::new(TransactionTracker::new(
                    Duration::from_secs(60),
                    256,
                    2048,
                )),
                file_handle_converter: FileHandleConverter::new(),
            },
        }
    }

    /// Serves NFS RPC messages over the supplied connected stream.
    pub async fn serve<IO>(&self, stream: IO) -> Result<(), anyhow::Error>
    where
        IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + 'static,
    {
        crate::tcp::process_socket(stream, self.context.clone()).await
    }
}
