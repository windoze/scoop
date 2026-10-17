use std::path::{Path, PathBuf};

use scoop_lir::ValidatedLirTargetSelection;

const CORE_SOURCE_RELATIVE_PATH: &str = "src/scoop.core";
const CORE_ARTIFACTS_RELATIVE_PATH: &str = "lib";
const CORE_ARTIFACT_FILE_NAME: &str = "scoop.core.slib";
/// Pure target-qualified sysroot layout shared by the orchestration parent and
/// the paired single-Cone compiler. This type grants no filesystem authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreSlotLayoutV1 {
    source_root: PathBuf,
    artifact_root: PathBuf,
    artifact: PathBuf,
}

impl TrustedCoreSlotLayoutV1 {
    pub fn new(sysroot: &Path, target: ValidatedLirTargetSelection) -> Self {
        let artifact_root = sysroot
            .join(CORE_ARTIFACTS_RELATIVE_PATH)
            .join(target.target().contract().canonical_triple());
        Self {
            source_root: sysroot.join(CORE_SOURCE_RELATIVE_PATH),
            artifact: artifact_root.join(CORE_ARTIFACT_FILE_NAME),
            artifact_root,
        }
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    pub fn artifact(&self) -> &Path {
        &self.artifact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_qualified_layout_has_one_shared_spelling() {
        let layout = TrustedCoreSlotLayoutV1::new(
            Path::new("/opt/scoop"),
            scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        );
        assert_eq!(layout.source_root(), Path::new("/opt/scoop/src/scoop.core"));
        assert_eq!(
            layout.artifact_root(),
            Path::new("/opt/scoop/lib/aarch64-apple-darwin")
        );
        assert_eq!(
            layout.artifact(),
            Path::new("/opt/scoop/lib/aarch64-apple-darwin/scoop.core.slib")
        );
    }
}
