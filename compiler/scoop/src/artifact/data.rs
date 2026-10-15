use std::cell::OnceCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use scoop_slib::{ArtifactManifestSummaryV1, ArtifactSnapshot};

/// Build metadata is immediately available; artifact bytes are read on demand.
#[derive(Debug)]
pub struct BuildArtifact {
    path: PathBuf,
    snapshot: OnceCell<Result<Arc<ArtifactSnapshot>, io::Error>>,
    pub(super) summary: ArtifactManifestSummaryV1,
}

impl BuildArtifact {
    pub(crate) fn from_result(path: PathBuf, summary: ArtifactManifestSummaryV1) -> Rc<Self> {
        Rc::new(Self {
            path,
            snapshot: OnceCell::new(),
            summary,
        })
    }

    pub fn snapshot(&self) -> io::Result<&Arc<ArtifactSnapshot>> {
        self.snapshot
            .get_or_init(|| {
                std::fs::read(&self.path).map(|bytes| Arc::new(ArtifactSnapshot::from_bytes(bytes)))
            })
            .as_ref()
            .map_err(|error| io::Error::new(error.kind(), error.to_string()))
    }

    pub const fn summary(&self) -> &ArtifactManifestSummaryV1 {
        &self.summary
    }
}
