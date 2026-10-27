use super::*;
use scoop_lir::{CBridgePlatformContractV1, CBridgeToolchainContractV1, DarwinPackedVersionV1};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ObjectDeployment {
    Darwin(DarwinPackedVersionV1),
    Linux,
    Windows,
}

impl ObjectDeployment {
    pub(super) const DARWIN_TEST: Self = Self::Darwin(match DarwinPackedVersionV1::new(11 << 16) {
        Ok(value) => value,
        Err(_) => panic!("valid test deployment"),
    });

    pub(super) fn from_contract(
        selection: ValidatedLirTargetSelection,
        native: &CBridgeToolchainContractV1,
    ) -> Result<Self, CodegenError> {
        if native.target().id() != selection.target().id() {
            return Err(CodegenError(
                "backend and native deployment targets differ".into(),
            ));
        }
        if selection.target().native_object_format() == ObjectFormat::Coff64 {
            return Ok(Self::Windows);
        }
        Ok(match native.platform() {
            CBridgePlatformContractV1::Darwin { deployment, .. } => {
                Self::Darwin(deployment.minimum_os())
            }
            CBridgePlatformContractV1::Linux { .. } => Self::Linux,
        })
    }

    pub(super) fn triple(self, canonical: &str) -> String {
        match self {
            Self::Darwin(version) => format!("aarch64-apple-macosx{version}"),
            Self::Linux => canonical.into(),
            Self::Windows => canonical.into(),
        }
    }
}
