//! The fixed system provider is read from the selected SDK, including re-exports.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use scoop_lir::DarwinPackedVersionV1;
use scoop_wire::{Digest256, sha256};

use crate::ToolchainError;

mod tbd;
pub use tbd::{TextStubInterface, read_text_stubs, write_link_stub};
#[cfg(test)]
mod tests;

pub const LIBSYSTEM_INSTALL_NAME: &str = "/usr/lib/libSystem.B.dylib";
const LIBSYSTEM_STUB: &str = "usr/lib/libSystem.tbd";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemExportKind {
    /// Text stubs do not distinguish C functions from ordinary data.
    Symbol,
    ThreadLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeExport {
    pub kind: SystemExportKind,
    pub weak: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemStubFile {
    relative_path: PathBuf,
    bytes: Vec<u8>,
    digest: Digest256,
}

impl SystemStubFile {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemProvider {
    files: Vec<SystemStubFile>,
    exports: BTreeMap<String, NativeExport>,
    reexports: BTreeSet<String>,
    current_version: u32,
    compatibility_version: u32,
}

impl SystemProvider {
    pub fn read(sdk: &Path, deployment: DarwinPackedVersionV1) -> Result<Self, ToolchainError> {
        let mut records = BTreeMap::new();
        let mut files = BTreeMap::new();
        read_file(sdk, Path::new(LIBSYSTEM_STUB), &mut files, &mut records)?;
        let mut pending = vec![LIBSYSTEM_INSTALL_NAME.to_owned()];
        let mut reexports = BTreeSet::new();
        let mut exports = BTreeMap::new();
        while let Some(name) = pending.pop() {
            if !reexports.insert(name.clone()) {
                continue;
            }
            if !records.contains_key(&name) {
                let relative = name.strip_prefix('/').ok_or_else(|| {
                    ToolchainError(format!(
                        "SDK re-export has no absolute install name: {name}"
                    ))
                })?;
                let path = Path::new(relative);
                if path
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    return Err(ToolchainError(format!(
                        "invalid SDK re-export path: {name}"
                    )));
                }
                read_file(sdk, &path.with_extension("tbd"), &mut files, &mut records)?;
            }
            let record = records.get(&name).ok_or_else(|| {
                ToolchainError(format!("SDK stub does not define re-export {name}"))
            })?;
            record.collect(deployment, true, &mut exports, &mut pending)?;
        }
        let root = records
            .get(LIBSYSTEM_INSTALL_NAME)
            .ok_or_else(|| ToolchainError("SDK stub has no libSystem record".into()))?;
        Ok(Self {
            files: files.into_values().collect(),
            exports,
            reexports,
            current_version: root.current_version,
            compatibility_version: root.compatibility_version,
        })
    }

    pub fn install_name(&self) -> &'static str {
        LIBSYSTEM_INSTALL_NAME
    }
    pub fn root_stub(&self) -> &'static Path {
        Path::new(LIBSYSTEM_STUB)
    }
    pub fn files(&self) -> &[SystemStubFile] {
        &self.files
    }
    pub fn exports(&self) -> &BTreeMap<String, NativeExport> {
        &self.exports
    }
    pub fn reexports(&self) -> &BTreeSet<String> {
        &self.reexports
    }
    pub fn current_version(&self) -> u32 {
        self.current_version
    }
    pub fn compatibility_version(&self) -> u32 {
        self.compatibility_version
    }

    /// Materialize precisely the bytes used to construct the export directory.
    pub fn write_to(&self, directory: &Path) -> Result<PathBuf, ToolchainError> {
        use std::io::Write;
        for file in &self.files {
            let path = directory.join(&file.relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(io_error)?;
            }
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(io_error)?;
            output.write_all(&file.bytes).map_err(io_error)?;
        }
        Ok(directory.join(LIBSYSTEM_STUB))
    }
}

fn read_file(
    sdk: &Path,
    relative: &Path,
    files: &mut BTreeMap<PathBuf, SystemStubFile>,
    records: &mut BTreeMap<String, tbd::Record>,
) -> Result<(), ToolchainError> {
    if files.contains_key(relative) {
        return Err(ToolchainError(format!(
            "missing re-export in SDK stub {}",
            relative.display()
        )));
    }
    let path = sdk.join(relative);
    let bytes = std::fs::read(&path).map_err(|error| {
        ToolchainError(format!("cannot read SDK stub {}: {error}", path.display()))
    })?;
    for record in tbd::parse(&bytes)? {
        let name = record.install_name.clone();
        if let Some(previous) = records.insert(name.clone(), record.clone())
            && previous != record
        {
            return Err(ToolchainError(format!(
                "conflicting SDK stub definitions for {name}"
            )));
        }
    }
    files.insert(
        relative.to_owned(),
        SystemStubFile {
            relative_path: relative.to_owned(),
            digest: sha256(&bytes),
            bytes,
        },
    );
    Ok(())
}

fn io_error(error: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("cannot materialize system provider: {error}"))
}
