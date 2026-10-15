use std::cell::OnceCell;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

use scoop_slib::{ArtifactFile, ArtifactManifestSummaryV1};

/// Build metadata is immediately available; artifact files open only when needed.
#[derive(Debug)]
pub struct BuildArtifact {
    path: PathBuf,
    file: OnceCell<ArtifactFile>,
    pub(super) summary: ArtifactManifestSummaryV1,
}

impl BuildArtifact {
    pub(crate) fn from_result(path: PathBuf, summary: ArtifactManifestSummaryV1) -> Rc<Self> {
        Rc::new(Self {
            path,
            file: OnceCell::new(),
            summary,
        })
    }

    pub fn file(&self) -> io::Result<&ArtifactFile> {
        if let Some(file) = self.file.get() {
            return Ok(file);
        }
        let file = ArtifactFile::open(&self.path, self.summary.target_selection())?;
        Ok(self.file.get_or_init(|| file))
    }

    pub const fn summary(&self) -> &ArtifactManifestSummaryV1 {
        &self.summary
    }
}
