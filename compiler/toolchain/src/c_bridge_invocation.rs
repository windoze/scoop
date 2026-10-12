//! Non-persistent host invocation selected for the generated-C producer.
//!
//! The semantic toolchain contract remains in `c_bridge_toolchain`; this
//! projection adds the absolute host locators needed to execute that contract.
//! Host paths are never encoded into LIR metadata or artifact fingerprints.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

mod command;

use scoop_lir::{CBridgeEnvironmentProjectionV1, CBridgeToolchainProfileV1, LirTargetProfile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCBridgeToolchainInvocation {
    profile: CBridgeToolchainProfileV1,
    compiler_driver: PathBuf,
    parameters: CBridgeCommandParameters,
    optimization: scoop_lir::OptimizationMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CBridgeCommandParameters {
    Darwin {
        sdk_root: PathBuf,
        minimum_os: scoop_lir::DarwinPackedVersionV1,
    },
    Linux {
        native_sysroot: Option<PathBuf>,
        real_gcc: PathBuf,
        search_path: OsString,
    },
}

impl ValidatedCBridgeToolchainInvocation {
    pub fn new(
        target: LirTargetProfile,
        profile: CBridgeToolchainProfileV1,
        compiler_driver: PathBuf,
        sdk_root: PathBuf,
    ) -> Result<Self, CBridgeToolchainInvocationError> {
        if !compiler_driver.is_absolute() {
            return Err(CBridgeToolchainInvocationError::RelativeCompilerDriver);
        }
        if !sdk_root.is_absolute() {
            return Err(CBridgeToolchainInvocationError::RelativeSdkRoot);
        }
        let minimum_os = profile
            .contract()
            .deployment()
            .map_err(CBridgeToolchainInvocationError::Platform)?
            .minimum_os();
        let value = Self {
            profile,
            compiler_driver,
            optimization: scoop_lir::OptimizationMode::Debug,
            parameters: CBridgeCommandParameters::Darwin {
                sdk_root,
                minimum_os,
            },
        };
        value.validate_target(target)?;
        Ok(value)
    }

    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.optimization
    }

    pub const fn profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.profile
    }

    pub fn compiler_driver(&self) -> &Path {
        &self.compiler_driver
    }

    pub fn sdk_root(&self) -> Result<&Path, scoop_lir::CBridgePlatformError> {
        match &self.parameters {
            CBridgeCommandParameters::Darwin { sdk_root, .. } => Ok(sdk_root),
            CBridgeCommandParameters::Linux { .. } => {
                Err(scoop_lir::CBridgePlatformError::ExpectedDarwin)
            }
        }
    }

    pub fn new_linux(
        target: LirTargetProfile,
        profile: CBridgeToolchainProfileV1,
        compiler_driver: PathBuf,
        native_sysroot: Option<PathBuf>,
        real_gcc: PathBuf,
        search_path: OsString,
    ) -> Result<Self, CBridgeToolchainInvocationError> {
        if !compiler_driver.is_absolute() || !real_gcc.is_absolute() {
            return Err(CBridgeToolchainInvocationError::RelativeCompilerDriver);
        }
        if native_sysroot
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err(CBridgeToolchainInvocationError::RelativeNativeSysroot);
        }
        if !matches!(
            profile.contract().platform(),
            scoop_lir::CBridgePlatformContractV1::Linux { .. }
        ) {
            return Err(CBridgeToolchainInvocationError::Platform(
                scoop_lir::CBridgePlatformError::ExpectedLinux,
            ));
        }
        let value = Self {
            profile,
            compiler_driver,
            optimization: scoop_lir::OptimizationMode::Debug,
            parameters: CBridgeCommandParameters::Linux {
                native_sysroot,
                real_gcc,
                search_path,
            },
        };
        value.validate_target(target)?;
        Ok(value)
    }

    pub fn native_sysroot(&self) -> Option<&Path> {
        match &self.parameters {
            CBridgeCommandParameters::Darwin { sdk_root, .. } => Some(sdk_root),
            CBridgeCommandParameters::Linux { native_sysroot, .. } => native_sysroot.as_deref(),
        }
    }

    pub const fn environment(&self) -> CBridgeEnvironmentProjectionV1 {
        self.profile.contract().environment()
    }

    pub fn driver_environment(&self) -> impl Iterator<Item = (&'static str, &OsStr)> {
        match &self.parameters {
            CBridgeCommandParameters::Darwin { .. } => None,
            CBridgeCommandParameters::Linux {
                real_gcc,
                search_path,
                ..
            } => Some([
                ("PATH", search_path.as_os_str()),
                ("REALGCC", real_gcc.as_os_str()),
            ]),
        }
        .into_iter()
        .flatten()
    }

    /// Start another native-driver action with the same resolved environment.
    pub fn driver_command(&self) -> Command {
        self.driver_command_with(&self.compiler_driver)
    }

    /// Use a paired native driver with the same target and environment.
    pub fn driver_command_with(&self, driver: &Path) -> Command {
        let mut command = Command::new(driver);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("TZ", "UTC")
            .env("TMPDIR", std::env::temp_dir())
            .envs(self.driver_environment());
        match &self.parameters {
            CBridgeCommandParameters::Darwin {
                sdk_root,
                minimum_os,
            } => {
                command
                    .args(["-target", self.profile.contract().canonical_triple()])
                    .arg("-isysroot")
                    .arg(sdk_root)
                    .arg(format!("-mmacosx-version-min={minimum_os}"));
            }
            CBridgeCommandParameters::Linux { native_sysroot, .. } => {
                if let Some(root) = native_sysroot {
                    command.arg("--sysroot").arg(root);
                }
            }
        }
        command
    }

    pub fn validate_target(
        &self,
        target: LirTargetProfile,
    ) -> Result<(), CBridgeToolchainInvocationError> {
        let contract = self.profile.contract();
        let target_fingerprint = target
            .fingerprint()
            .map_err(CBridgeToolchainInvocationError::TargetFingerprint)?;
        if contract.target() == &target.wire_id()
            && contract.target_fingerprint() == target_fingerprint
        {
            Ok(())
        } else {
            Err(CBridgeToolchainInvocationError::TargetMismatch)
        }
    }
}

#[derive(Debug)]
pub enum CBridgeToolchainInvocationError {
    RelativeCompilerDriver,
    RelativeSdkRoot,
    RelativeNativeSysroot,
    Platform(scoop_lir::CBridgePlatformError),
    TargetFingerprint(scoop_wire::HashError),
    TargetMismatch,
}

impl fmt::Display for CBridgeToolchainInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeCompilerDriver => {
                formatter.write_str("generated-C compiler driver must be absolute")
            }
            Self::RelativeSdkRoot => formatter.write_str("generated-C SDK root must be absolute"),
            Self::RelativeNativeSysroot => {
                formatter.write_str("generated-C native sysroot must be absolute")
            }
            Self::Platform(error) => error.fmt(formatter),
            Self::TargetFingerprint(error) => {
                write!(formatter, "cannot fingerprint generated-C target: {error}")
            }
            Self::TargetMismatch => formatter
                .write_str("generated-C toolchain profile does not match the selected LIR target"),
        }
    }
}

impl std::error::Error for CBridgeToolchainInvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetFingerprint(error) => Some(error),
            Self::Platform(error) => Some(error),
            Self::RelativeCompilerDriver
            | Self::RelativeSdkRoot
            | Self::RelativeNativeSysroot
            | Self::TargetMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests;
