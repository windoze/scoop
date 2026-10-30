use super::*;
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgePlatformContractV1, DarwinBuildToolIdV1,
    DarwinBuildToolVersionContractV1, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    GccCompilerIdentityV1, LinuxCCompilerIdentityV1,
};

#[derive(Serialize, Deserialize)]
pub(super) struct InvocationRecord {
    driver: HostPathCarrier,
    platform: PlatformRecord,
}

#[derive(Serialize, Deserialize)]
enum PlatformRecord {
    Darwin {
        sdk_root: HostPathCarrier,
        minimum_os: u32,
        sdk: u32,
        compiler: (u32, u32, u32, String),
        tools: Vec<(u32, u32)>,
    },
    Linux {
        native_sysroot: Option<HostPathCarrier>,
        real_gcc: Option<HostPathCarrier>,
        search_path: Option<HostPathCarrier>,
        compiler: LinuxCompilerRecord,
        configuration: [u8; 32],
    },
    Windows {
        native_sysroot: Option<HostPathCarrier>,
        includes: Vec<HostPathCarrier>,
        libraries: Vec<HostPathCarrier>,
        search_path: HostPathCarrier,
        system_root: Option<HostPathCarrier>,
        version: [u32; 3],
        configuration: [u8; 32],
    },
}

#[derive(Serialize, Deserialize)]
enum LinuxCompilerRecord {
    Gcc(String),
    Clang([u32; 3]),
}

impl InvocationRecord {
    pub(super) fn from_invocation(
        c: &ValidatedCBridgeToolchainInvocation,
    ) -> Result<Self, ToolchainError> {
        let platform = match c.profile().contract().platform() {
            CBridgePlatformContractV1::Windows { compiler } => {
                let environment = c.windows_environment().map_err(error)?;
                PlatformRecord::Windows {
                    native_sysroot: environment
                        .native_sysroot
                        .as_deref()
                        .map(path)
                        .transpose()?,
                    includes: environment
                        .includes
                        .iter()
                        .map(|p| path(p))
                        .collect::<Result<_, _>>()?,
                    libraries: environment
                        .libraries
                        .iter()
                        .map(|p| path(p))
                        .collect::<Result<_, _>>()?,
                    search_path: path(Path::new(&environment.search_path))?,
                    system_root: environment
                        .system_root
                        .as_ref()
                        .map(|p| path(Path::new(p)))
                        .transpose()?,
                    version: compiler.version(),
                    configuration: *compiler.inputs().as_array(),
                }
            }
            CBridgePlatformContractV1::Darwin {
                deployment,
                compiler,
            } => PlatformRecord::Darwin {
                sdk_root: path(c.sdk_root().map_err(error)?)?,
                minimum_os: deployment.minimum_os().packed(),
                sdk: deployment.sdk().packed(),
                compiler: (
                    compiler.version_major(),
                    compiler.version_minor(),
                    compiler.version_patch(),
                    compiler.build().to_owned(),
                ),
                tools: deployment
                    .tools()
                    .iter()
                    .map(|t| (t.tool().macho_value(), t.version().packed()))
                    .collect(),
            },
            CBridgePlatformContractV1::Linux { compiler } => {
                let command = c.driver_command();
                let env_path = |key| {
                    command
                        .get_envs()
                        .find(|(k, _)| *k == key)
                        .and_then(|(_, v)| v)
                        .filter(|v| !v.is_empty())
                        .map(|v| path(Path::new(v)))
                        .transpose()
                };
                PlatformRecord::Linux {
                    native_sysroot: c.native_sysroot().map(path).transpose()?,
                    real_gcc: env_path("REALGCC")?,
                    search_path: env_path("PATH")?,
                    compiler: match compiler {
                        LinuxCCompilerIdentityV1::Gcc(compiler) => {
                            LinuxCompilerRecord::Gcc(compiler.version().to_owned())
                        }
                        LinuxCCompilerIdentityV1::Clang(compiler) => {
                            LinuxCompilerRecord::Clang(compiler.version())
                        }
                    },
                    configuration: *compiler.inputs().as_array(),
                }
            }
        };
        Ok(Self {
            driver: path(c.compiler_driver())?,
            platform,
        })
    }

    pub(super) fn into_invocation(
        self,
        target: scoop_lir::LirTargetProfile,
    ) -> Result<ValidatedCBridgeToolchainInvocation, ToolchainError> {
        let driver = self.driver.to_path_buf().map_err(error)?;
        match self.platform {
            PlatformRecord::Windows {
                native_sysroot,
                includes,
                libraries,
                search_path,
                system_root,
                version,
                configuration,
            } => {
                let decode = |path: HostPathCarrier| path.to_path_buf().map_err(error);
                let environment = crate::windows_c::WindowsCEnvironment {
                    native_sysroot: native_sysroot.map(decode).transpose()?,
                    includes: includes.into_iter().map(decode).collect::<Result<_, _>>()?,
                    libraries: libraries
                        .into_iter()
                        .map(decode)
                        .collect::<Result<_, _>>()?,
                    search_path: decode(search_path)?.into_os_string(),
                    system_root: system_root
                        .map(decode)
                        .transpose()?
                        .map(PathBuf::into_os_string),
                };
                let compiler = scoop_lir::ClangCompilerIdentityV1::new(
                    version,
                    Digest256::from_array(configuration),
                );
                let profile =
                    scoop_target_data::windows_c_bridge_profile(target, compiler).map_err(error)?;
                ValidatedCBridgeToolchainInvocation::new_windows(
                    target,
                    profile,
                    driver,
                    environment,
                )
                .map_err(error)
            }
            PlatformRecord::Darwin {
                sdk_root,
                minimum_os,
                sdk,
                compiler: (major, minor, patch, build),
                tools,
            } => {
                let tools = tools
                    .into_iter()
                    .map(|(tool, version)| {
                        let tool = match tool {
                            1 => DarwinBuildToolIdV1::Clang,
                            3 => DarwinBuildToolIdV1::Ld,
                            4 => DarwinBuildToolIdV1::Lld,
                            _ => return Err(error("unknown Darwin build tool")),
                        };
                        Ok(DarwinBuildToolVersionContractV1::new(
                            tool,
                            DarwinPackedVersionV1::new(version).map_err(error)?,
                        ))
                    })
                    .collect::<Result<_, ToolchainError>>()?;
                let deployment = DarwinCBridgeDeploymentContractV1::new(
                    DarwinPackedVersionV1::new(minimum_os).map_err(error)?,
                    DarwinPackedVersionV1::new(sdk).map_err(error)?,
                    tools,
                )
                .map_err(error)?;
                let compiler = AppleClangCompilerIdentityV1::new(major, minor, patch, &build)
                    .map_err(error)?;
                let profile =
                    scoop_target_data::darwin_c_bridge_profile(target, deployment, compiler)
                        .map_err(error)?;
                ValidatedCBridgeToolchainInvocation::new(
                    target,
                    profile,
                    driver,
                    sdk_root.to_path_buf().map_err(error)?,
                )
                .map_err(error)
            }
            PlatformRecord::Linux {
                native_sysroot,
                real_gcc,
                search_path,
                compiler,
                configuration,
            } => {
                let inputs = Digest256::from_array(configuration);
                let compiler = match compiler {
                    LinuxCompilerRecord::Gcc(version) => LinuxCCompilerIdentityV1::Gcc(
                        GccCompilerIdentityV1::new(&version, inputs).map_err(error)?,
                    ),
                    LinuxCompilerRecord::Clang(version) => LinuxCCompilerIdentityV1::Clang(
                        scoop_lir::ClangCompilerIdentityV1::new(version, inputs),
                    ),
                };
                let profile =
                    scoop_target_data::linux_c_bridge_profile(target, compiler).map_err(error)?;
                let decode = |value: HostPathCarrier| value.to_path_buf().map_err(error);
                ValidatedCBridgeToolchainInvocation::new_linux(
                    target,
                    profile,
                    driver,
                    native_sysroot.map(decode).transpose()?,
                    real_gcc.map(decode).transpose()?,
                    search_path
                        .map(decode)
                        .transpose()?
                        .unwrap_or_default()
                        .into_os_string(),
                )
                .map_err(error)
            }
        }
    }
}
