//! Same-host compiler requests retain the selected native tool locators.
use scoop_lir::TargetProfileId;
use scoop_protocol::{HostPathCarrier, TargetSelectionRequestV1};

use crate::{CToolchainOptions, ResolvedTargetProfile, ToolchainError};

impl CToolchainOptions {
    pub fn from_request(request: &TargetSelectionRequestV1) -> Result<Self, ToolchainError> {
        let path = |value: Option<&HostPathCarrier>| {
            value
                .map(HostPathCarrier::to_path_buf)
                .transpose()
                .map_err(|error| ToolchainError(error.to_string()))
        };
        Ok(Self {
            compiler: path(request.compiler())?,
            native_sysroot: path(request.native_sysroot())?,
        })
    }
}

impl ResolvedTargetProfile {
    pub fn resolve_request(request: &TargetSelectionRequestV1) -> Result<Self, ToolchainError> {
        Self::resolve_with(
            request.canonical_triple(),
            &CToolchainOptions::from_request(request)?,
        )
    }

    pub fn child_request(&self) -> Result<TargetSelectionRequestV1, ToolchainError> {
        let request = TargetSelectionRequestV1::new(self.canonical_triple().to_owned())
            .map_err(|error| ToolchainError(error.to_string()))?;
        if self.id() == TargetProfileId::DarwinAarch64 {
            return Ok(request);
        }
        let invocation = self.c_bridge_toolchain();
        Ok(request.with_c_toolchain(
            Some(
                HostPathCarrier::from_path(invocation.compiler_driver())
                    .map_err(|error| ToolchainError(error.to_string()))?,
            ),
            invocation
                .native_sysroot()
                .map(HostPathCarrier::from_path)
                .transpose()
                .map_err(|error| ToolchainError(error.to_string()))?,
        ))
    }
}
