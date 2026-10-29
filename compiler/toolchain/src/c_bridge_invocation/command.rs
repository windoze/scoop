use super::*;
use scoop_lir::{CanonicalCBridgeFlagContractV1, CanonicalCBridgeFlagV1};
use scoop_process::CommandExt;

impl ValidatedCBridgeToolchainInvocation {
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
                self.optimization,
                source,
                object,
            ),
            CBridgeCommandParameters::Linux { .. } => {
                let mut command = self.driver_command();
                command
                    .arg("-std=c11")
                    .arg("-c")
                    .arg(source)
                    .arg("-o")
                    .arg(object)
                    .args([
                        self.optimization.c_flag(),
                        "-g0",
                        "-fno-common",
                        "-fno-ident",
                        "-fno-stack-protector",
                        "-funwind-tables",
                        "-fasynchronous-unwind-tables",
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

#[allow(clippy::too_many_arguments)]
fn canonical_object_compilation_command(
    compiler: &Path,
    sdk_root: &Path,
    canonical_triple: &str,
    minimum_os: String,
    environment: CBridgeEnvironmentProjectionV1,
    optimization: scoop_lir::OptimizationMode,
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
            CanonicalCBridgeFlagV1::SelectedOptimization => {
                command.arg(optimization.c_flag());
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
            CanonicalCBridgeFlagV1::PreserveAllFramePointers => {
                command.args(["-fno-omit-frame-pointer", "-mno-omit-leaf-frame-pointer"]);
            }
            CanonicalCBridgeFlagV1::UnwindTables => {
                command.arg("-funwind-tables");
            }
            CanonicalCBridgeFlagV1::AsynchronousUnwindTables => {
                command.arg("-fasynchronous-unwind-tables");
            }
            CanonicalCBridgeFlagV1::NoBuiltinSubstitution => {
                command.arg("-fno-builtin");
            }
            CanonicalCBridgeFlagV1::PositionIndependent => {
                command.arg("-fPIC");
            }
            CanonicalCBridgeFlagV1::X86_64Baseline => {
                command.arg("-march=x86-64");
            }
        }
    }
    command
}

impl ValidatedCBridgeToolchainInvocation {
    pub fn compile_object(
        &self,
        source: &Path,
        object: &Path,
    ) -> Result<(), crate::ToolchainError> {
        let output = self
            .object_compilation_command(source, object)
            .scoop_output()
            .map_err(|error| {
                crate::ToolchainError(format!(
                    "failed to run generated-C compiler `{}`: {error}",
                    self.compiler_driver().display()
                ))
            })?;
        if !output.status.success() {
            return Err(crate::ToolchainError(format!(
                "generated-C compilation failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }
}
