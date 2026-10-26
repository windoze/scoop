use std::process::Command;

use scoop_manifest::NativeSourceLanguage;

use super::*;
use crate::ValidatedCxxToolchain;

pub struct NativeToolchain {
    c: ValidatedCBridgeToolchainInvocation,
    cxx: Option<ValidatedCxxToolchain>,
}

impl NativeToolchain {
    pub fn prepare(
        &self,
        manifest: &LoadedConeManifest,
        optimization: OptimizationMode,
        public_include: &Path,
    ) -> Result<PreparedNativeInputs, ToolchainError> {
        prepare::prepare(
            manifest,
            self.c.profile().contract().target().id(),
            self,
            optimization,
            public_include,
        )
    }

    pub fn resolve(
        c: &ValidatedCBridgeToolchainInvocation,
        manifest: &LoadedConeManifest,
    ) -> Result<Self, ToolchainError> {
        let cxx = manifest.parsed().semantic().native().cxx();
        Ok(Self {
            c: c.clone(),
            cxx: cxx
                .then(|| ValidatedCxxToolchain::resolve(c))
                .transpose()
                .map_err(|err| {
                    ToolchainError(format!("{err}; C++ required by {}", manifest.coordinate()))
                })?,
        })
    }

    pub(super) fn resolve_cached(
        target: &crate::ResolvedTargetProfile,
        manifest: &LoadedConeManifest,
        cache: &Path,
    ) -> Result<Self, ToolchainError> {
        Ok(Self {
            c: target.c_bridge_toolchain().clone(),
            cxx: manifest
                .parsed()
                .semantic()
                .native()
                .cxx()
                .then(|| ValidatedCxxToolchain::resolve_cached(target, cache))
                .transpose()
                .map_err(|err| {
                    ToolchainError(format!("{err}; C++ required by {}", manifest.coordinate()))
                })?,
        })
    }

    pub(super) fn cxx_fingerprint(&self) -> Option<Digest256> {
        self.cxx.as_ref().map(ValidatedCxxToolchain::fingerprint)
    }

    pub(super) fn input_metadata(&self) -> Vec<crate::NativeInputMetadata> {
        self.cxx
            .as_ref()
            .map_or_else(Vec::new, |cxx| cxx.input_metadata().to_vec())
    }

    pub(super) fn c(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.c
    }

    pub(super) fn command(
        &self,
        language: NativeSourceLanguage,
    ) -> Result<Command, ToolchainError> {
        match language {
            NativeSourceLanguage::C => Ok(self.c.driver_command()),
            NativeSourceLanguage::Cxx => self
                .cxx
                .as_ref()
                .map(|cxx| cxx.command(&self.c))
                .ok_or_else(|| error("C++ translation unit requires native.cxx = true")),
        }
    }
}
