use scoop_process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ValidatedCBridgeToolchainInvocation;
use object::endian::Endian;
use object::read::Object;
use object::read::macho::MachOFile64;
use object::{ObjectKind, macho};
use scoop_lir::{
    AppleClangCompilerIdentityV1, DarwinBuildToolIdV1, DarwinBuildToolVersionContractV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, LirTargetProfile,
};

use crate::ToolchainError;

const XCRUN: &str = "/usr/bin/xcrun";
const MACHO_TOOL_LLD: u32 = 4;

pub(super) fn deployment_version(target: LirTargetProfile) -> Result<String, ToolchainError> {
    match std::env::var("MACOSX_DEPLOYMENT_TARGET") {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => crate::DARWIN_DEPLOYMENT_BASELINES
            .iter()
            .find(|(canonical, _)| *canonical == target.contract().canonical_triple())
            .map(|(_, version)| (*version).to_owned())
            .ok_or_else(|| {
                ToolchainError(format!(
                    "no Darwin deployment baseline for {}",
                    target.contract().canonical_triple()
                ))
            }),
        Err(std::env::VarError::NotUnicode(_)) => Err(ToolchainError(
            "MACOSX_DEPLOYMENT_TARGET must be UTF-8 major.minor[.patch]".into(),
        )),
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
pub(super) fn resolve_system_c_bridge_toolchain()
-> Result<ValidatedCBridgeToolchainInvocation, ToolchainError> {
    probe_system_c_bridge_toolchain(scoop_target_data::DARWIN_AARCH64, None, None)
        .map(|(invocation, _)| invocation)
}

pub(super) fn probe_system_c_bridge_toolchain(
    target: LirTargetProfile,
    compiler: Option<&Path>,
    sdk: Option<&Path>,
) -> Result<(ValidatedCBridgeToolchainInvocation, Vec<PathBuf>), ToolchainError> {
    let deployment_version = deployment_version(target)?;
    let minimum_os = parse_darwin_version(&deployment_version, "MACOSX_DEPLOYMENT_TARGET")?;
    let compiler_path = match compiler {
        Some(path) => crate::paths::find_program(
            path.as_os_str(),
            &std::env::var_os("PATH").unwrap_or_default(),
        )?,
        None => PathBuf::from(command_text(XCRUN, &["--find", "clang"])?),
    };
    let sdk_path = match sdk {
        Some(path) => std::path::absolute(path)
            .map_err(|error| ToolchainError(format!("macOS SDK path: {error}")))?,
        None => PathBuf::from(command_text(
            XCRUN,
            &["--sdk", "macosx", "--show-sdk-path"],
        )?),
    };
    let compiler_driver = canonical_existing_path(&compiler_path, "Apple Clang compiler", true)?;
    let sdk_root = canonical_existing_path(&sdk_path, "macOS SDK root", false)?;
    let compiler_version = command_text_from_path(&compiler_driver, &["--version"])?;
    let settings_path = sdk_root.join("SDKSettings.json");
    let settings: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).map_err(|error| {
            ToolchainError(format!("cannot read {}: {error}", settings_path.display()))
        })?)
        .map_err(|error| ToolchainError(format!("invalid macOS SDK settings: {error}")))?;
    let sdk_version = settings
        .get("Version")
        .and_then(|value| value.as_str())
        .ok_or_else(|| ToolchainError("macOS SDK settings are missing Version".into()))?
        .to_owned();
    let sdk_version_value = parse_darwin_version(&sdk_version, "macOS SDK version")?;
    let tools = probe_build_tools(
        target,
        &compiler_driver,
        &sdk_root,
        minimum_os,
        sdk_version_value,
        parse_apple_clang_identity(&compiler_version)?,
    )?;
    let mut inputs = vec![
        compiler_path,
        sdk_path,
        compiler_driver.clone(),
        settings_path,
    ];
    if let Some(compiler) = compiler {
        let mut candidates = std::collections::BTreeSet::new();
        crate::native_record::program_candidates(
            compiler.as_os_str(),
            &std::env::var_os("PATH").unwrap_or_default(),
            &mut candidates,
        );
        inputs.extend(candidates);
    }
    if compiler.is_none() || sdk.is_none() {
        inputs.extend([
            PathBuf::from(XCRUN),
            PathBuf::from("/var/db/xcode_select_link"),
            PathBuf::from("/Applications/Xcode.app"),
            PathBuf::from("/Library/Developer/CommandLineTools"),
        ]);
    }
    let facts = CBridgeToolchainFacts {
        target,
        compiler_driver,
        sdk_root,
        sdk_version,
        deployment_version,
        compiler_version,
        tools,
    };
    Ok((resolve_c_bridge_toolchain(facts)?, inputs))
}

struct CBridgeToolchainFacts {
    target: LirTargetProfile,
    compiler_driver: PathBuf,
    sdk_root: PathBuf,
    sdk_version: String,
    deployment_version: String,
    compiler_version: String,
    tools: Vec<DarwinBuildToolVersionContractV1>,
}

fn resolve_c_bridge_toolchain(
    facts: CBridgeToolchainFacts,
) -> Result<ValidatedCBridgeToolchainInvocation, ToolchainError> {
    require_absolute(&facts.compiler_driver, "Apple Clang compiler")?;
    require_absolute(&facts.sdk_root, "macOS SDK root")?;
    let sdk = parse_darwin_version(&facts.sdk_version, "macOS SDK version")?;
    let minimum_os = parse_darwin_version(&facts.deployment_version, "macOS deployment version")?;
    let compiler = parse_apple_clang_identity(&facts.compiler_version)?;
    let deployment =
        DarwinCBridgeDeploymentContractV1::new(minimum_os, sdk, facts.tools).map_err(|error| {
            ToolchainError(format!("invalid generated-C deployment facts: {error}"))
        })?;
    let profile = scoop_target_data::darwin_c_bridge_profile(facts.target, deployment, compiler)
        .map_err(|error| {
            ToolchainError(format!("cannot fingerprint C bridge toolchain: {error}"))
        })?;
    ValidatedCBridgeToolchainInvocation::new(
        facts.target,
        profile,
        facts.compiler_driver,
        facts.sdk_root,
    )
    .map_err(|error| ToolchainError(format!("invalid generated-C invocation: {error}")))
}

fn probe_build_tools(
    target: LirTargetProfile,
    compiler: &Path,
    sdk_root: &Path,
    minimum_os: DarwinPackedVersionV1,
    sdk: DarwinPackedVersionV1,
    compiler_identity: AppleClangCompilerIdentityV1,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, ToolchainError> {
    let directory = tempfile::Builder::new()
        .prefix("scoop-c-bridge-probe-")
        .tempdir()
        .map_err(|error| {
            ToolchainError(format!("cannot create C bridge probe directory: {error}"))
        })?;
    let source = directory.path().join("probe.c");
    let object = directory.path().join("probe.o");
    std::fs::write(&source, b"void scoop_c_bridge_probe(void) {}\n")
        .map_err(|error| ToolchainError(format!("cannot write C bridge probe source: {error}")))?;
    let deployment = DarwinCBridgeDeploymentContractV1::new(minimum_os, sdk, Vec::new())
        .map_err(|error| ToolchainError(format!("invalid C bridge probe deployment: {error}")))?;
    let profile = scoop_target_data::darwin_c_bridge_profile(target, deployment, compiler_identity)
        .map_err(|error| ToolchainError(format!("cannot fingerprint C bridge probe: {error}")))?;
    let invocation = ValidatedCBridgeToolchainInvocation::new(
        target,
        profile,
        compiler.to_path_buf(),
        sdk_root.to_path_buf(),
    )
    .map_err(|error| ToolchainError(format!("invalid C bridge probe invocation: {error}")))?;
    let output = invocation
        .object_compilation_command(&source, &object)
        .scoop_output()
        .map_err(|error| {
            ToolchainError(format!("failed to run C bridge probe compiler: {error}"))
        })?;
    if !output.status.success() {
        return Err(ToolchainError(format!(
            "C bridge probe compilation failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let bytes = std::fs::read(&object)
        .map_err(|error| ToolchainError(format!("cannot read C bridge probe object: {error}")))?;
    parse_probe_deployment(target, &bytes, minimum_os, sdk)
}

fn parse_probe_deployment(
    target: LirTargetProfile,
    bytes: &[u8],
    expected_minimum_os: DarwinPackedVersionV1,
    expected_sdk: DarwinPackedVersionV1,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, ToolchainError> {
    let file: MachOFile64<'_> = MachOFile64::parse(bytes)
        .map_err(|error| ToolchainError(format!("invalid C bridge probe Mach-O: {error}")))?;
    let machine = scoop_slib::MachOArchitecture::for_target(target.id())
        .map_err(|error| ToolchainError(format!("invalid Darwin probe target: {error}")))?;
    if file.architecture() != machine.object_architecture()
        || file.kind() != ObjectKind::Relocatable
        || !file.endian().is_little_endian()
    {
        return Err(ToolchainError(
            "C bridge probe did not produce little-endian MH_OBJECT for the selected machine"
                .to_owned(),
        ));
    }
    let endian = file.endian();
    let mut deployment = None;
    let mut commands = file.macho_load_commands().map_err(|error| {
        ToolchainError(format!("invalid C bridge probe load commands: {error}"))
    })?;
    while let Some(command) = commands
        .next()
        .map_err(|error| ToolchainError(format!("invalid C bridge probe load command: {error}")))?
    {
        if command.cmd() != macho::LC_BUILD_VERSION {
            continue;
        }
        if deployment.is_some() {
            return Err(ToolchainError(
                "C bridge probe contains multiple LC_BUILD_VERSION commands".to_owned(),
            ));
        }
        let build = command
            .build_version()
            .map_err(|error| ToolchainError(format!("invalid C bridge probe deployment: {error}")))?
            .ok_or_else(|| {
                ToolchainError("LC_BUILD_VERSION command did not decode as deployment".to_owned())
            })?;
        if build.platform.get(endian) != macho::PLATFORM_MACOS {
            return Err(ToolchainError(format!(
                "C bridge probe selected platform {}, expected macOS",
                build.platform.get(endian)
            )));
        }
        let minimum_os = DarwinPackedVersionV1::new(build.minos.get(endian))
            .map_err(|error| ToolchainError(format!("invalid probe minimum OS: {error}")))?;
        let sdk = DarwinPackedVersionV1::new(build.sdk.get(endian))
            .map_err(|error| ToolchainError(format!("invalid probe SDK version: {error}")))?;
        if minimum_os != expected_minimum_os || sdk != expected_sdk {
            return Err(ToolchainError(format!(
                "C bridge probe deployment drift: expected min OS {expected_minimum_os} / SDK {expected_sdk}, found min OS {minimum_os} / SDK {sdk}"
            )));
        }
        deployment = Some(parse_probe_tools(
            command.raw_data(),
            build.ntools.get(endian),
        )?);
    }
    deployment
        .ok_or_else(|| ToolchainError("C bridge probe is missing LC_BUILD_VERSION".to_owned()))
}

fn parse_probe_tools(
    command: &[u8],
    tool_count: u32,
) -> Result<Vec<DarwinBuildToolVersionContractV1>, ToolchainError> {
    let expected_length = usize::try_from(tool_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .and_then(|size| size.checked_add(24))
        .ok_or_else(|| ToolchainError("C bridge probe tool table size overflow".to_owned()))?;
    if command.len() != expected_length {
        return Err(ToolchainError(format!(
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
                    return Err(ToolchainError(format!(
                        "unsupported C bridge probe build tool {value} at index {index}"
                    )));
                }
            };
            let version = u32::from_le_bytes([record[4], record[5], record[6], record[7]]);
            let version = DarwinPackedVersionV1::new(version).map_err(|error| {
                ToolchainError(format!(
                    "invalid C bridge probe build tool version at index {index}: {error}"
                ))
            })?;
            Ok(DarwinBuildToolVersionContractV1::new(tool, version))
        })
        .collect()
}

fn command_text(program: &str, args: &[&str]) -> Result<String, ToolchainError> {
    command_text_from_path(Path::new(program), args)
}

pub(crate) fn command_text_from_path(
    program: &Path,
    args: &[&str],
) -> Result<String, ToolchainError> {
    let output = Command::new(program)
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("TZ", "UTC")
        .args(args)
        .scoop_output()
        .map_err(|error| {
            ToolchainError(format!(
                "failed to query C bridge toolchain with `{}`: {error}",
                program.display()
            ))
        })?;
    if !output.status.success() {
        return Err(ToolchainError(format!(
            "C bridge toolchain query `{}` failed (status {}): {}",
            program.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = std::str::from_utf8(&output.stdout).map_err(|error| {
        ToolchainError(format!(
            "C bridge toolchain query `{}` returned non-UTF-8 output: {error}",
            program.display()
        ))
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ToolchainError(format!(
            "C bridge toolchain query `{}` returned empty output",
            program.display()
        )));
    }
    Ok(value.to_owned())
}

fn canonical_existing_path(
    queried: &Path,
    role: &'static str,
    require_file: bool,
) -> Result<PathBuf, ToolchainError> {
    let path = PathBuf::from(queried);
    require_absolute(&path, role)?;
    let path = std::fs::canonicalize(&path).map_err(|error| {
        ToolchainError(format!("cannot resolve {role} {}: {error}", path.display()))
    })?;
    let valid_kind = if require_file {
        path.is_file()
    } else {
        path.is_dir()
    };
    if !valid_kind {
        return Err(ToolchainError(format!(
            "{role} has the wrong file kind: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn require_absolute(path: &Path, role: &'static str) -> Result<(), ToolchainError> {
    if !path.is_absolute() {
        return Err(ToolchainError(format!(
            "{role} locator must be absolute, found {}",
            path.display()
        )));
    }
    Ok(())
}

fn parse_apple_clang_identity(
    output: &str,
) -> Result<AppleClangCompilerIdentityV1, ToolchainError> {
    let first_line = output.lines().next().unwrap_or_default();
    let remainder = first_line
        .strip_prefix("Apple clang version ")
        .ok_or_else(|| ToolchainError(format!("unsupported C bridge compiler: {first_line:?}")))?;
    let mut fields = remainder.split_ascii_whitespace();
    let version = fields.next().unwrap_or_default();
    let build = fields.next().unwrap_or_default();
    if fields.next().is_some() || !build.starts_with('(') || !build.ends_with(')') {
        return Err(ToolchainError(format!(
            "noncanonical Apple Clang identity: {first_line:?}"
        )));
    }
    let packed = parse_darwin_version(version, "Apple Clang version")?;
    let (major, minor, patch) = packed.components();
    let build = &build[1..build.len() - 1];
    let compiler = AppleClangCompilerIdentityV1::new(major, minor, patch, build)
        .map_err(|error| ToolchainError(format!("invalid Apple Clang identity: {error}")))?;
    Ok(compiler)
}

pub(crate) fn parse_darwin_version(
    spelling: &str,
    role: &'static str,
) -> Result<DarwinPackedVersionV1, ToolchainError> {
    let components = spelling.split('.').collect::<Vec<_>>();
    if !(2..=3).contains(&components.len()) {
        return Err(ToolchainError(format!(
            "invalid {role} {spelling:?}: expected major.minor[.patch]"
        )));
    }
    let parse = |component: &str| {
        component.parse::<u32>().map_err(|_| {
            ToolchainError(format!(
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
        .map_err(|error| ToolchainError(format!("invalid {role} {spelling:?}: {error}")))
}

#[cfg(test)]
mod tests;
