use std::path::{Path, PathBuf};
use std::process::Command;

use object::endian::Endian;
use object::read::Object;
use object::read::macho::MachOFile64;
use object::{Architecture, ObjectKind, macho};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeEnvironmentProjectionV1, CBridgeToolchainProfileV1,
    CanonicalCBridgeFlagContractV1, CanonicalCBridgeFlagV1, DarwinBuildToolIdV1,
    DarwinBuildToolVersionContractV1, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    LirTargetProfile,
};

use crate::CodegenError;

const XCRUN: &str = "/usr/bin/xcrun";
const SW_VERS: &str = "/usr/bin/sw_vers";
const MACHO_TOOL_LLD: u32 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCBridgeToolchainProfile {
    profile: CBridgeToolchainProfileV1,
    compiler_driver: PathBuf,
    sdk_root: PathBuf,
}

impl ValidatedCBridgeToolchainProfile {
    pub const fn profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.profile
    }

    pub fn compiler_driver(&self) -> &Path {
        &self.compiler_driver
    }

    pub fn sdk_root(&self) -> &Path {
        &self.sdk_root
    }

    pub const fn environment(&self) -> CBridgeEnvironmentProjectionV1 {
        self.profile.contract().environment()
    }

    pub fn object_compilation_command(&self, source: &Path, object: &Path) -> Command {
        canonical_object_compilation_command(
            &self.compiler_driver,
            &self.sdk_root,
            self.profile.contract().canonical_triple(),
            self.profile.contract().deployment().minimum_os(),
            self.profile.contract().environment(),
            source,
            object,
        )
    }

    pub(crate) fn validate_lir_target_profile(
        &self,
        actual: LirTargetProfile,
    ) -> Result<(), CodegenError> {
        let contract = self.profile.contract();
        let actual_fingerprint = actual.fingerprint().map_err(|error| {
            CodegenError(format!(
                "cannot fingerprint LIR target profile for generated-C production: {error}"
            ))
        })?;
        if contract.target() == &actual.wire_id()
            && contract.target_fingerprint() == actual_fingerprint
        {
            return Ok(());
        }
        let profile_id = self.profile.id().capability();
        Err(CodegenError(format!(
            "LIR target profile `{}` does not match generated-C toolchain profile `{}/{}/{}`",
            actual.id().canonical_name(),
            profile_id.namespace(),
            profile_id.name(),
            profile_id.major_version()
        )))
    }
}

pub(super) fn resolve_system_c_bridge_toolchain()
-> Result<ValidatedCBridgeToolchainProfile, CodegenError> {
    let compiler_driver = canonical_existing_path(
        &command_text(XCRUN, &["--find", "clang"])?,
        "Apple Clang compiler",
        true,
    )?;
    let sdk_root = canonical_existing_path(
        &command_text(XCRUN, &["--sdk", "macosx", "--show-sdk-path"])?,
        "macOS SDK root",
        false,
    )?;
    let compiler_version = command_text_from_path(&compiler_driver, &["--version"])?;
    let sdk_version = command_text(XCRUN, &["--sdk", "macosx", "--show-sdk-version"])?;
    let deployment_version = command_text(SW_VERS, &["-productVersion"])?;
    let sdk = parse_darwin_version(&sdk_version, "macOS SDK version")?;
    let minimum_os = parse_darwin_version(&deployment_version, "macOS deployment version")?;
    let tools = probe_build_tools(&compiler_driver, &sdk_root, minimum_os, sdk)?;
    let facts = CBridgeToolchainFacts {
        compiler_driver,
        sdk_root,
        sdk_version,
        deployment_version,
        compiler_version,
        tools,
    };
    resolve_c_bridge_toolchain(facts)
}

struct CBridgeToolchainFacts {
    compiler_driver: PathBuf,
    sdk_root: PathBuf,
    sdk_version: String,
    deployment_version: String,
    compiler_version: String,
    tools: Vec<DarwinBuildToolVersionContractV1>,
}

fn resolve_c_bridge_toolchain(
    facts: CBridgeToolchainFacts,
) -> Result<ValidatedCBridgeToolchainProfile, CodegenError> {
    require_absolute(&facts.compiler_driver, "Apple Clang compiler")?;
    require_absolute(&facts.sdk_root, "macOS SDK root")?;
    let sdk = parse_darwin_version(&facts.sdk_version, "macOS SDK version")?;
    let minimum_os = parse_darwin_version(&facts.deployment_version, "macOS deployment version")?;
    let compiler = parse_apple_clang_identity(&facts.compiler_version)?;
    let deployment = DarwinCBridgeDeploymentContractV1::new(minimum_os, sdk, facts.tools)
        .map_err(|error| CodegenError(format!("invalid generated-C deployment facts: {error}")))?;
    let profile = CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(deployment, compiler)
        .map_err(|error| {
        CodegenError(format!("cannot fingerprint C bridge toolchain: {error}"))
    })?;
    Ok(ValidatedCBridgeToolchainProfile {
        profile,
        compiler_driver: facts.compiler_driver,
        sdk_root: facts.sdk_root,
    })
}

fn probe_build_tools(
    compiler: &Path,
    sdk_root: &Path,
    minimum_os: DarwinPackedVersionV1,
    sdk: DarwinPackedVersionV1,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, CodegenError> {
    let directory = tempfile::Builder::new()
        .prefix("scoop-c-bridge-probe-")
        .tempdir()
        .map_err(|error| {
            CodegenError(format!("cannot create C bridge probe directory: {error}"))
        })?;
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.o");
    std::fs::write(&source, b"void scoop_c_bridge_probe(void) {}\n")
        .map_err(|error| CodegenError(format!("cannot write C bridge probe source: {error}")))?;
    let output = canonical_object_compilation_command(
        compiler,
        sdk_root,
        LirTargetProfile::DARWIN_AARCH64
            .contract()
            .canonical_triple(),
        minimum_os,
        CBridgeEnvironmentProjectionV1::CLEAN_C_LOCALE_UTC,
        &source,
        &object,
    )
    .output()
    .map_err(|error| CodegenError(format!("failed to run C bridge probe compiler: {error}")))?;
    if !output.status.success() {
        return Err(CodegenError(format!(
            "C bridge probe compilation failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let bytes = std::fs::read(&object)
        .map_err(|error| CodegenError(format!("cannot read C bridge probe object: {error}")))?;
    parse_probe_deployment(&bytes, minimum_os, sdk)
}

fn canonical_object_compilation_command(
    compiler: &Path,
    sdk_root: &Path,
    canonical_triple: &str,
    minimum_os: DarwinPackedVersionV1,
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
        }
    }
    command
}

fn parse_probe_deployment(
    bytes: &[u8],
    expected_minimum_os: DarwinPackedVersionV1,
    expected_sdk: DarwinPackedVersionV1,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, CodegenError> {
    let file: MachOFile64<'_> = MachOFile64::parse(bytes)
        .map_err(|error| CodegenError(format!("invalid C bridge probe Mach-O: {error}")))?;
    if file.architecture() != Architecture::Aarch64
        || file.kind() != ObjectKind::Relocatable
        || !file.endian().is_little_endian()
    {
        return Err(CodegenError(
            "C bridge probe did not produce little-endian AArch64 MH_OBJECT".to_owned(),
        ));
    }
    let endian = file.endian();
    let mut deployment = None;
    let mut commands = file
        .macho_load_commands()
        .map_err(|error| CodegenError(format!("invalid C bridge probe load commands: {error}")))?;
    while let Some(command) = commands
        .next()
        .map_err(|error| CodegenError(format!("invalid C bridge probe load command: {error}")))?
    {
        if command.cmd() != macho::LC_BUILD_VERSION {
            continue;
        }
        if deployment.is_some() {
            return Err(CodegenError(
                "C bridge probe contains multiple LC_BUILD_VERSION commands".to_owned(),
            ));
        }
        let build = command
            .build_version()
            .map_err(|error| CodegenError(format!("invalid C bridge probe deployment: {error}")))?
            .ok_or_else(|| {
                CodegenError("LC_BUILD_VERSION command did not decode as deployment".to_owned())
            })?;
        if build.platform.get(endian) != macho::PLATFORM_MACOS {
            return Err(CodegenError(format!(
                "C bridge probe selected platform {}, expected macOS",
                build.platform.get(endian)
            )));
        }
        let minimum_os = DarwinPackedVersionV1::new(build.minos.get(endian))
            .map_err(|error| CodegenError(format!("invalid probe minimum OS: {error}")))?;
        let sdk = DarwinPackedVersionV1::new(build.sdk.get(endian))
            .map_err(|error| CodegenError(format!("invalid probe SDK version: {error}")))?;
        if minimum_os != expected_minimum_os || sdk != expected_sdk {
            return Err(CodegenError(format!(
                "C bridge probe deployment drift: expected min OS {expected_minimum_os} / SDK {expected_sdk}, found min OS {minimum_os} / SDK {sdk}"
            )));
        }
        deployment = Some(parse_probe_tools(
            command.raw_data(),
            build.ntools.get(endian),
        )?);
    }
    deployment.ok_or_else(|| CodegenError("C bridge probe is missing LC_BUILD_VERSION".to_owned()))
}

fn parse_probe_tools(
    command: &[u8],
    tool_count: u32,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, CodegenError> {
    let expected_length = usize::try_from(tool_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .and_then(|size| size.checked_add(24))
        .ok_or_else(|| CodegenError("C bridge probe tool table size overflow".to_owned()))?;
    if command.len() != expected_length {
        return Err(CodegenError(format!(
            "C bridge probe tool table length mismatch: expected {expected_length}, found {}",
            command.len()
        )));
    }
    command[24..]
        .chunks_exact(8)
        .enumerate()
        .map(|(index, record)| {
            let tool = u32::from_le_bytes([record[0], record[1], record[2], record[3]]);
            let tool = match tool {
                macho::TOOL_CLANG => DarwinBuildToolIdV1::Clang,
                macho::TOOL_LD => DarwinBuildToolIdV1::Ld,
                MACHO_TOOL_LLD => DarwinBuildToolIdV1::Lld,
                value => {
                    return Err(CodegenError(format!(
                        "unsupported C bridge probe build tool {value} at index {index}"
                    )));
                }
            };
            let version = u32::from_le_bytes([record[4], record[5], record[6], record[7]]);
            let version = DarwinPackedVersionV1::new(version).map_err(|error| {
                CodegenError(format!(
                    "invalid C bridge probe build tool version at index {index}: {error}"
                ))
            })?;
            Ok(DarwinBuildToolVersionContractV1::new(tool, version))
        })
        .collect()
}

fn command_text(program: &str, args: &[&str]) -> Result<String, CodegenError> {
    command_text_from_path(Path::new(program), args)
}

fn command_text_from_path(program: &Path, args: &[&str]) -> Result<String, CodegenError> {
    let output = Command::new(program)
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("TZ", "UTC")
        .args(args)
        .output()
        .map_err(|error| {
            CodegenError(format!(
                "failed to query C bridge toolchain with `{}`: {error}",
                program.display()
            ))
        })?;
    if !output.status.success() {
        return Err(CodegenError(format!(
            "C bridge toolchain query `{}` failed (status {}): {}",
            program.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = std::str::from_utf8(&output.stdout).map_err(|error| {
        CodegenError(format!(
            "C bridge toolchain query `{}` returned non-UTF-8 output: {error}",
            program.display()
        ))
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(CodegenError(format!(
            "C bridge toolchain query `{}` returned empty output",
            program.display()
        )));
    }
    Ok(value.to_owned())
}

fn canonical_existing_path(
    queried: &str,
    role: &'static str,
    require_file: bool,
) -> Result<PathBuf, CodegenError> {
    let path = PathBuf::from(queried);
    require_absolute(&path, role)?;
    let path = std::fs::canonicalize(&path).map_err(|error| {
        CodegenError(format!("cannot resolve {role} {}: {error}", path.display()))
    })?;
    let valid_kind = if require_file {
        path.is_file()
    } else {
        path.is_dir()
    };
    if !valid_kind {
        return Err(CodegenError(format!(
            "{role} has the wrong file kind: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn require_absolute(path: &Path, role: &'static str) -> Result<(), CodegenError> {
    if !path.is_absolute() {
        return Err(CodegenError(format!(
            "{role} locator must be absolute, found {}",
            path.display()
        )));
    }
    Ok(())
}

fn parse_apple_clang_identity(output: &str) -> Result<AppleClangCompilerIdentityV1, CodegenError> {
    let first_line = output.lines().next().unwrap_or_default();
    let remainder = first_line
        .strip_prefix("Apple clang version ")
        .ok_or_else(|| CodegenError(format!("unsupported C bridge compiler: {first_line:?}")))?;
    let mut fields = remainder.split_ascii_whitespace();
    let version = fields.next().unwrap_or_default();
    let build = fields.next().unwrap_or_default();
    if fields.next().is_some() || !build.starts_with('(') || !build.ends_with(')') {
        return Err(CodegenError(format!(
            "noncanonical Apple Clang identity: {first_line:?}"
        )));
    }
    let packed = parse_darwin_version(version, "Apple Clang version")?;
    let (major, minor, patch) = packed.components();
    let build = &build[1..build.len() - 1];
    let compiler = AppleClangCompilerIdentityV1::new(major, minor, patch, build)
        .map_err(|error| CodegenError(format!("invalid Apple Clang identity: {error}")))?;
    Ok(compiler)
}

fn parse_darwin_version(
    spelling: &str,
    role: &'static str,
) -> Result<DarwinPackedVersionV1, CodegenError> {
    let components = spelling.split('.').collect::<Vec<_>>();
    if !(2..=3).contains(&components.len()) {
        return Err(CodegenError(format!(
            "invalid {role} {spelling:?}: expected major.minor[.patch]"
        )));
    }
    let parse = |component: &str| {
        component.parse::<u32>().map_err(|_| {
            CodegenError(format!(
                "invalid {role} {spelling:?}: non-numeric component"
            ))
        })
    };
    let major = parse(components[0])?;
    let minor = parse(components[1])?;
    let patch = components
        .get(2)
        .map(|value| parse(value))
        .transpose()?
        .unwrap_or(0);
    DarwinPackedVersionV1::from_components(major, minor, patch)
        .map_err(|error| CodegenError(format!("invalid {role} {spelling:?}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn system_resolver_qualifies_the_actual_probe_object() {
        let resolved = resolve_system_c_bridge_toolchain().unwrap();
        assert!(resolved.compiler_driver().is_file());
        assert!(resolved.sdk_root().is_dir());
        assert_eq!(
            resolved.profile().contract().canonical_triple(),
            "aarch64-apple-darwin"
        );
    }

    #[test]
    fn fake_host_facts_resolve_without_host_paths_entering_the_contract() {
        let resolved = resolve_c_bridge_toolchain(facts()).unwrap();
        assert_eq!(
            resolved.compiler_driver(),
            Path::new("/toolchain/bin/clang")
        );
        assert_eq!(resolved.sdk_root(), Path::new("/toolchain/SDKs/MacOSX.sdk"));
        let contract = resolved.profile().contract();
        assert_eq!(contract.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(contract.deployment().minimum_os().components(), (15, 6, 2));
        assert_eq!(contract.deployment().sdk().components(), (26, 5, 0));
        assert!(contract.deployment().tools().is_empty());
        assert_eq!(contract.compiler().version_major(), 21);
        assert_eq!(contract.compiler().build(), "clang-2100.1.1.101");
    }

    #[test]
    fn validated_profile_builds_only_the_canonical_object_command() {
        let resolved = resolve_c_bridge_toolchain(facts()).unwrap();
        let command = resolved.object_compilation_command(
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
            ]
        );
        assert_eq!(
            command.get_envs().collect::<Vec<_>>(),
            [
                (
                    std::ffi::OsStr::new("LANG"),
                    Some(std::ffi::OsStr::new("C"))
                ),
                (
                    std::ffi::OsStr::new("LC_ALL"),
                    Some(std::ffi::OsStr::new("C"))
                ),
                (
                    std::ffi::OsStr::new("TZ"),
                    Some(std::ffi::OsStr::new("UTC"))
                ),
            ]
        );
    }

    #[test]
    fn resolver_rejects_relative_locators_and_non_apple_clang() {
        let mut relative = facts();
        relative.compiler_driver = PathBuf::from("clang");
        assert!(
            resolve_c_bridge_toolchain(relative)
                .unwrap_err()
                .0
                .contains("absolute")
        );

        let mut upstream = facts();
        upstream.compiler_version = "clang version 21.0.0".to_owned();
        assert!(
            resolve_c_bridge_toolchain(upstream)
                .unwrap_err()
                .0
                .contains("unsupported C bridge compiler")
        );
    }

    #[test]
    fn version_parser_rejects_open_or_out_of_range_shapes() {
        assert_eq!(
            parse_darwin_version("26.5", "SDK").unwrap().components(),
            (26, 5, 0)
        );
        for spelling in ["26", "26.5.1.2", "26.x", "26.256"] {
            assert!(parse_darwin_version(spelling, "SDK").is_err(), "{spelling}");
        }
    }

    fn facts() -> CBridgeToolchainFacts {
        CBridgeToolchainFacts {
            compiler_driver: PathBuf::from("/toolchain/bin/clang"),
            sdk_root: PathBuf::from("/toolchain/SDKs/MacOSX.sdk"),
            sdk_version: "26.5".to_owned(),
            deployment_version: "15.6.2".to_owned(),
            compiler_version:
                "Apple clang version 21.0.0 (clang-2100.1.1.101)\nTarget: arm64-apple-darwin"
                    .to_owned(),
            tools: Vec::new(),
        }
    }
}
