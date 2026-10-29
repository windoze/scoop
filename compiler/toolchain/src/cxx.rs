//! The C++ driver paired with an already selected C toolchain.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::ValidatedCBridgeToolchainInvocation;
use scoop_process::CommandExt;
use scoop_wire::{Digest256, sha256};

use crate::ToolchainError;
use crate::native_driver::{NativeDriver, NativeDriverDiscovery as Discovery, NativeDriverKind};

mod discovery;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidatedCxxToolchain {
    native: NativeDriver,
}

impl ValidatedCxxToolchain {
    pub fn resolve(c: &ValidatedCBridgeToolchainInvocation) -> Result<Self, ToolchainError> {
        Self::resolve_with(c, None)
    }

    pub fn resolve_with(
        c: &ValidatedCBridgeToolchainInvocation,
        driver: Option<&Path>,
    ) -> Result<Self, ToolchainError> {
        Self::from_record(Self::probe(c, driver)?)
    }

    pub fn resolve_cached(
        target: &crate::ResolvedTargetProfile,
        cache: &Path,
    ) -> Result<Self, ToolchainError> {
        let key = crate::build_record::fingerprint(
            &crate::native_record::ResolvedRecord::from_target(target)?,
        )?;
        let record = crate::build_record::cached(
            &cache.join("cxx-tools-v3"),
            key,
            |_| Ok(true),
            || Self::probe(target.c_bridge_toolchain(), target.native_cxx_driver()),
        )?;
        Self::from_record(record)
    }

    fn from_record(record: crate::build_record::Record<Discovery>) -> Result<Self, ToolchainError> {
        Ok(Self {
            native: NativeDriver::from_record(record)?,
        })
    }

    fn probe(
        c: &ValidatedCBridgeToolchainInvocation,
        selected: Option<&Path>,
    ) -> Result<crate::build_record::Record<Discovery>, ToolchainError> {
        if c.profile().contract().target().id() == scoop_target_data::LINUX_X86_64_MUSL_ID {
            return Err(error("native.cxx = true is not supported for Linux musl"));
        }
        if crate::windows_native::is_windows(c) {
            let default = crate::windows_native::paired_driver(
                c,
                scoop_manifest::NativeSourceLanguage::Cxx,
                None,
            );
            return crate::windows_native::discover(
                c,
                scoop_manifest::NativeSourceLanguage::Cxx,
                selected.unwrap_or(&default),
            );
        }
        let driver = match selected {
            Some(path) => crate::paths::find_program(
                path.as_os_str(),
                &std::env::var_os("PATH").unwrap_or_default(),
            )?,
            None => discovery::paired_driver(c)?,
        };
        let mut inputs = std::collections::BTreeSet::new();
        if let Some(selected) = selected {
            crate::native_record::program_candidates(
                selected.as_os_str(),
                &std::env::var_os("PATH").unwrap_or_default(),
                &mut inputs,
            );
        }
        discovery::inputs(c, &driver, &mut inputs)?;
        let headers = run(c
            .driver_command_with(&driver)
            .args([
                "-v",
                "-M",
                "-MT",
                "cxx-probe",
                "-x",
                "c++",
                "-include",
                "string",
                "-",
            ])
            .stdin(Stdio::null()))?;
        let headers_text = std::str::from_utf8(&headers.stdout).map_err(error)?;
        let mut header_paths = scoop_process::parse_make_dependencies(headers_text, "cxx-probe")
            .ok_or_else(|| error("C++ compiler returned an invalid header dependency list"))?
            .into_iter()
            .filter(|p| p != Path::new("-") && p != Path::new("<stdin>"))
            .collect();
        crate::native_record::header_candidates(&headers.stderr, &mut header_paths);
        inputs.extend(header_paths);
        inputs.insert(driver.clone());
        let macros = run(c
            .driver_command_with(&driver)
            .args(["-dM", "-E", "-x", "c++", "-include", "string", "-"])
            .stdin(Stdio::null()))?;
        Ok(crate::build_record::Record {
            inputs: inputs
                .iter()
                .map(|p| crate::build_record::Input::read(p))
                .collect::<Result<_, _>>()?,
            result: Discovery {
                driver: scoop_protocol::HostPathCarrier::from_path(&driver).map_err(error)?,
                configuration: *sha256(&macros.stdout).as_array(),
                kind: NativeDriverKind::Gnu,
            },
        })
    }

    pub(crate) fn input_paths(&self) -> Result<Vec<PathBuf>, ToolchainError> {
        self.native
            .inputs()
            .iter()
            .map(|input| input.path())
            .collect()
    }

    pub(crate) fn input_metadata(&self) -> &[crate::NativeInputMetadata] {
        self.native.inputs()
    }

    pub fn driver(&self) -> &Path {
        self.native.path()
    }

    pub fn fingerprint(&self) -> Digest256 {
        self.native.fingerprint()
    }

    pub fn command(&self, c: &ValidatedCBridgeToolchainInvocation) -> Command {
        self.native
            .command(c, scoop_manifest::NativeSourceLanguage::Cxx)
    }

    pub(crate) fn kind(&self) -> NativeDriverKind {
        self.native.kind()
    }
}

fn run(command: &mut Command) -> Result<std::process::Output, ToolchainError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(output)
}

fn text(command: &mut Command) -> Result<String, ToolchainError> {
    String::from_utf8(run(command)?.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(error)
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native C++ toolchain: {value}"))
}
