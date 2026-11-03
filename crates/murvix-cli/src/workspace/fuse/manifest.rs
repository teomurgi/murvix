use std::borrow::Cow;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use murvix_protocol::{ProjectId, ServerIdentity, SessionId, WorkspaceRoot, WorkspaceSnapshot};
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

#[derive(Serialize)]
struct Document<'a> {
    server: &'a ServerIdentity,
    session: &'a SessionId,
    project: &'a ProjectId,
    revision: &'a str,
    roots: Roots<'a>,
}

struct Roots<'a>(&'a [WorkspaceRoot]);

#[derive(Serialize)]
struct Root<'a> {
    directory: &'a str,
    key: &'a str,
    primary: bool,
    path_bytes: &'a [u8],
    path: Option<&'a str>,
    name: Cow<'a, str>,
    error: Option<&'a str>,
}

impl Serialize for Roots<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for root in self.0 {
            let path = Path::new(OsStr::from_bytes(&root.path.0));
            sequence.serialize_element(&Root {
                directory: if root.primary { "primary" } else { &root.key },
                key: &root.key,
                primary: root.primary,
                path_bytes: &root.path.0,
                path: std::str::from_utf8(&root.path.0).ok(),
                name: path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy(),
                error: root.error.as_deref(),
            })?;
        }
        sequence.end()
    }
}

pub(super) fn render(snapshot: &WorkspaceSnapshot) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&Document {
        server: &snapshot.server,
        session: &snapshot.session,
        project: &snapshot.project,
        revision: &snapshot.revision,
        roots: Roots(&snapshot.roots),
    })
}
