//! Same-host compiler requests retain the selected native tool locators.
use scoop_protocol::{HostPathCarrier, TargetSelectionRequestV1};

use crate::{CToolchainOptions, ResolvedTargetProfile, ToolchainError};

impl CToolchainOptions {
    pub fn request(&self, triple: String) -> Result<TargetSelectionRequestV1, ToolchainError> {
        let path = |value: &Option<std::path::PathBuf>| {
            value
                .as_deref()
                .map(|p| {
                    let p = if p.components().count() > 1 {
                        std::path::absolute(p).map_err(|e| ToolchainError(e.to_string()))?
                    } else {
                        p.to_owned()
                    };
                    HostPathCarrier::from_path(&p).map_err(|e| ToolchainError(e.to_string()))
                })
                .transpose()
        };
        Ok(TargetSelectionRequestV1::new(triple)
            .map_err(|error| ToolchainError(error.to_string()))?
            .with_native_toolchain(
                path(&self.compiler)?,
                path(
                    &self
                        .native_sysroot
                        .as_deref()
                        .map(std::path::absolute)
                        .transpose()
                        .map_err(|e| ToolchainError(e.to_string()))?,
                )?,
                path(&self.cxx_compiler)?,
            ))
    }

    pub fn from_request(request: &TargetSelectionRequestV1) -> Result<Self, ToolchainError> {
        let path = |value: Option<&HostPathCarrier>| {
            value
                .map(HostPathCarrier::to_path_buf)
                .transpose()
                .map_err(|error| ToolchainError(error.to_string()))
        };
        Ok(Self {
            compiler: path(request.compiler())?,
            cxx_compiler: path(request.cxx_compiler())?,
            native_sysroot: path(request.native_sysroot())?,
        })
    }
}

impl ResolvedTargetProfile {
    pub fn resolve_with_cached(
        triple: &str,
        options: &CToolchainOptions,
        cache: &std::path::Path,
    ) -> Result<Self, ToolchainError> {
        Self::resolve_request_cached(&options.request(triple.to_owned())?, cache)
    }

    pub fn resolve_request(request: &TargetSelectionRequestV1) -> Result<Self, ToolchainError> {
        if let Some(bytes) = request.resolved_toolchain() {
            let record: crate::native_record::ResolvedRecord = serde_json::from_slice(bytes)
                .map_err(|e| ToolchainError(format!("C toolchain request: {e}")))?;
            return record.into_target(request.canonical_triple());
        }
        Self::resolve_with(
            request.canonical_triple(),
            &CToolchainOptions::from_request(request)?,
        )
    }

    pub fn resolve_request_cached(
        request: &TargetSelectionRequestV1,
        cache: &std::path::Path,
    ) -> Result<Self, ToolchainError> {
        if request.resolved_toolchain().is_some() {
            return Self::resolve_request(request);
        }
        crate::native_record::resolve_cached(request, cache)
    }

    pub fn child_request(&self) -> Result<TargetSelectionRequestV1, ToolchainError> {
        let request = TargetSelectionRequestV1::new(self.canonical_triple().to_owned())
            .map_err(|error| ToolchainError(error.to_string()))?;
        let record = crate::native_record::ResolvedRecord::from_target(self)?;
        Ok(request.with_resolved_toolchain(
            serde_json::to_vec(&record)
                .map_err(|e| ToolchainError(format!("C toolchain request: {e}")))?,
        ))
    }
}
