//! Non-persistent host invocation selected for the generated-C producer.
//!
//! The semantic toolchain contract remains in `c_bridge_toolchain`; this
//! projection adds the absolute host locators needed to execute that contract.
//! Host paths are never encoded into LIR metadata or artifact fingerprints.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    CBridgeEnvironmentProjectionV1, CBridgeToolchainProfileV1, CanonicalCBridgeFlagContractV1,
    CanonicalCBridgeFlagV1, LirTargetProfile,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCBridgeToolchainInvocation {
    profile: CBridgeToolchainProfileV1,
    compiler_driver: PathBuf,
    parameters: CBridgeCommandParameters,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CBridgeCommandParameters {
    Darwin {
        sdk_root: PathBuf,
        minimum_os: crate::DarwinPackedVersionV1,
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
            parameters: CBridgeCommandParameters::Darwin {
                sdk_root,
                minimum_os,
            },
        };
        value.validate_target(target)?;
        Ok(value)
    }

    pub const fn profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.profile
    }

    pub fn compiler_driver(&self) -> &Path {
        &self.compiler_driver
    }

    pub fn sdk_root(&self) -> Result<&Path, crate::CBridgePlatformError> {
        match &self.parameters {
            CBridgeCommandParameters::Darwin { sdk_root, .. } => Ok(sdk_root),
            CBridgeCommandParameters::Linux { .. } => {
                Err(crate::CBridgePlatformError::ExpectedDarwin)
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
            crate::CBridgePlatformContractV1::Linux { .. }
        ) {
            return Err(CBridgeToolchainInvocationError::Platform(
                crate::CBridgePlatformError::ExpectedLinux,
            ));
        }
        let value = Self {
            profile,
            compiler_driver,
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

    pub fn object_compilation_command(&self, source: &Path, object: &Path) -> Command {
        let mut command = match &self.parameters {
            CBridgeCommandParameters::Darwin {
                sdk_root,
                minimum_os,
            } => canonical_object_compilation_command(
                &self.compiler_driver,
                sdk_root,
                self.profile.contract().canonical_triple(),
                minimum_os.to_string(),
                self.environment(),
                source,
                object,
            ),
            CBridgeCommandParameters::Linux {
                native_sysroot,
                real_gcc,
                search_path,
            } => {
                let mut command = Command::new(&self.compiler_driver);
                command
                    .env_clear()
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .env("TZ", "UTC")
                    .env("PATH", search_path)
                    .env("REALGCC", real_gcc);
                if let Some(sysroot) = native_sysroot {
                    command.arg("--sysroot").arg(sysroot);
                }
                command
                    .arg("-std=c11")
                    .arg("-c")
                    .arg(source)
                    .arg("-o")
                    .arg(object)
                    .args([
                        "-O0",
                        "-g0",
                        "-fno-common",
                        "-fno-ident",
                        "-fno-stack-protector",
                        "-fno-unwind-tables",
                        "-fno-asynchronous-unwind-tables",
                        "-fno-builtin",
                        "-fPIC",
                    ]);
                command
            }
        };
        command.env(
            "TMPDIR",
            object
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        );
        command
    }
}

fn canonical_object_compilation_command(
    compiler: &Path,
    sdk_root: &Path,
    canonical_triple: &str,
    minimum_os: String,
    environment: CBridgeEnvironmentProjectionV1,
    source: &Path,
    object: &Path,
) -> Command {
    let mut command = Command::new(compiler);
    command
        .env_clear()
        .env("LC_ALL", environment.locale())
        .env("LANG", environment.locale())
        .env("TZ", environment.timezone());
    for flag in CanonicalCBridgeFlagContractV1::CURRENT.flags() {
        match flag {
            CanonicalCBridgeFlagV1::ExplicitCanonicalTarget => {
                command.args(["-target", canonical_triple]);
            }
            CanonicalCBridgeFlagV1::ExplicitResolvedSdkRoot => {
                command.arg("-isysroot").arg(sdk_root);
            }
            CanonicalCBridgeFlagV1::ExplicitMinimumDeployment => {
                command.arg(format!("-mmacosx-version-min={minimum_os}"));
            }
            CanonicalCBridgeFlagV1::C11 => {
                command.arg("-std=c11");
            }
            CanonicalCBridgeFlagV1::RelocatableObject => {
                command.arg("-c").arg(source).arg("-o").arg(object);
            }
            CanonicalCBridgeFlagV1::Unoptimized => {
                command.arg("-O0");
            }
            CanonicalCBridgeFlagV1::NoDebugInformation => {
                command.arg("-g0");
            }
            CanonicalCBridgeFlagV1::NoCommonSymbols => {
                command.arg("-fno-common");
            }
            CanonicalCBridgeFlagV1::OmitCompilerIdentification => {
                command.arg("-fno-ident");
            }
            CanonicalCBridgeFlagV1::NoStackProtector => {
                command.arg("-fno-stack-protector");
            }
            CanonicalCBridgeFlagV1::NoUnwindTables => {
                command.arg("-fno-unwind-tables");
            }
            CanonicalCBridgeFlagV1::NoAsynchronousUnwindTables => {
                command.arg("-fno-asynchronous-unwind-tables");
            }
            CanonicalCBridgeFlagV1::NoBuiltinSubstitution => {
                command.arg("-fno-builtin");
            }
            CanonicalCBridgeFlagV1::PositionIndependent => {
                command.arg("-fPIC");
            }
        }
    }
    command
}

#[derive(Debug)]
pub enum CBridgeToolchainInvocationError {
    RelativeCompilerDriver,
    RelativeSdkRoot,
    RelativeNativeSysroot,
    Platform(crate::CBridgePlatformError),
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
mod tests {
    use super::*;
    use crate::{
        AppleClangCompilerIdentityV1, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    };

    #[test]
    fn invocation_builds_only_the_canonical_command() {
        let profile = profile();
        let invocation = ValidatedCBridgeToolchainInvocation::new(
            LirTargetProfile::DARWIN_AARCH64,
            profile,
            PathBuf::from("/toolchain/bin/clang"),
            PathBuf::from("/toolchain/SDKs/MacOSX.sdk"),
        )
        .unwrap();
        let command = invocation.object_compilation_command(
            Path::new("/temporary/input.c"),
            Path::new("/temporary/output.o"),
        );
        assert_eq!(command.get_program(), "/toolchain/bin/clang");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "-target",
                "aarch64-apple-darwin",
                "-isysroot",
                "/toolchain/SDKs/MacOSX.sdk",
                "-mmacosx-version-min=15.6.2",
                "-std=c11",
                "-c",
                "/temporary/input.c",
                "-o",
                "/temporary/output.o",
                "-O0",
                "-g0",
                "-fno-common",
                "-fno-ident",
                "-fno-stack-protector",
                "-fno-unwind-tables",
                "-fno-asynchronous-unwind-tables",
                "-fno-builtin",
            ]
        );
    }

    #[test]
    fn invocation_rejects_relative_host_locators() {
        let error = ValidatedCBridgeToolchainInvocation::new(
            LirTargetProfile::DARWIN_AARCH64,
            profile(),
            PathBuf::from("clang"),
            PathBuf::from("/toolchain/SDKs/MacOSX.sdk"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CBridgeToolchainInvocationError::RelativeCompilerDriver
        ));
    }

    fn profile() -> CBridgeToolchainProfileV1 {
        let deployment = DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::from_components(15, 6, 2).unwrap(),
            DarwinPackedVersionV1::from_components(26, 5, 0).unwrap(),
            Vec::new(),
        )
        .unwrap();
        let compiler = AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap();
        CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(deployment, compiler).unwrap()
    }
}
