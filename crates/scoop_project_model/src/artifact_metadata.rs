//! Stage-independent cone artifact metadata and fingerprint helpers.
//!
//! This module owns the JSON manifest schema, stable file names, and lightweight
//! fingerprint readers used by the facade and linker-facing code. Full bincode
//! stage payloads remain in `scoopc_cone`.

use std::fs;
use std::path::{Path, PathBuf};

use scoopc_types::{WIRE_SCHEMA_VERSION, WireSchemaVersion};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{ConeKind, StableConeKey};

pub const CONE_ARTIFACT_MANIFEST_FILE_NAME: &str = "manifest.json";
pub const CONE_ARTIFACT_HIR_FACTS_FILE_NAME: &str = "hir_facts.bin";
pub const CONE_ARTIFACT_MIR_FACTS_FILE_NAME: &str = "mir_facts.bin";
pub const CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME: &str = "effect_facts.bin";
pub const CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME: &str = "lir_program.bin";
pub const CONE_ARTIFACT_TYPE_STORE_FILE_NAME: &str = "type_store.bin";
pub const CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME: &str = "frontend_import.json";
pub const CONE_ARTIFACT_OBJS_DIR_NAME: &str = "objs";
pub const CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME: &str = "inputs.fingerprint";
pub const CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME: &str = "outputs.fingerprint";

const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Result<T> = std::result::Result<T, ConeArtifactMetadataError>;

#[derive(Debug, Error)]
pub enum ConeArtifactMetadataError {
    #[error("failed to access cone artifact path `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode or decode cone artifact JSON manifest")]
    ManifestJson(#[from] serde_json::Error),
    #[error(
        "incompatible cone artifact compiler version `{found}` (expected `{expected}`); rebuild the cone"
    )]
    IncompatibleCompilerVersion { expected: String, found: String },
    #[error(
        "incompatible cone artifact schema versions {found:?} (expected {expected:?}); rebuild the cone"
    )]
    IncompatibleSchemaVersions {
        expected: ConeArtifactSchemaVersions,
        found: ConeArtifactSchemaVersions,
    },
    #[error("invalid object file name `{file_name}` in cone artifact")]
    InvalidObjectFileName { file_name: String },
    #[error(
        "cone artifact is missing required frontend import payload `{file_name}`; rebuild the cone"
    )]
    MissingFrontendImportPayload { file_name: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeArtifactManifest {
    pub cone_name: String,
    pub cone_version: String,
    pub cone_kind: ConeKind,
    pub compiler_version: String,
    pub schema_versions: ConeArtifactSchemaVersions,
    pub object_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extern_libs: Vec<String>,
}

impl ConeArtifactManifest {
    pub fn current(cone: &StableConeKey, cone_kind: ConeKind, object_files: Vec<String>) -> Self {
        Self {
            cone_name: cone.name().to_owned(),
            cone_version: cone.version().to_owned(),
            cone_kind,
            compiler_version: COMPILER_VERSION.to_owned(),
            schema_versions: ConeArtifactSchemaVersions::current(),
            object_files,
            extern_libs: Vec::new(),
        }
    }

    pub fn stable_cone_key(&self) -> StableConeKey {
        StableConeKey::new(&self.cone_name, &self.cone_version)
    }

    pub fn ensure_compatible(&self) -> Result<()> {
        if self.compiler_version != COMPILER_VERSION {
            return Err(ConeArtifactMetadataError::IncompatibleCompilerVersion {
                expected: COMPILER_VERSION.to_owned(),
                found: self.compiler_version.clone(),
            });
        }
        if !self.schema_versions.has_frontend_import_payload() {
            return Err(ConeArtifactMetadataError::MissingFrontendImportPayload {
                file_name: CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
            });
        }
        let expected = ConeArtifactSchemaVersions::current();
        if self.schema_versions != expected {
            return Err(ConeArtifactMetadataError::IncompatibleSchemaVersions {
                expected,
                found: self.schema_versions,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeArtifactSchemaVersions {
    pub hir_facts: WireSchemaVersion,
    pub mir_facts: WireSchemaVersion,
    pub effect_facts: WireSchemaVersion,
    pub lir_program: WireSchemaVersion,
    pub type_store: WireSchemaVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend_import: Option<WireSchemaVersion>,
}

impl ConeArtifactSchemaVersions {
    pub const fn current() -> Self {
        Self {
            hir_facts: WIRE_SCHEMA_VERSION,
            mir_facts: WIRE_SCHEMA_VERSION,
            effect_facts: WIRE_SCHEMA_VERSION,
            lir_program: WIRE_SCHEMA_VERSION,
            type_store: WIRE_SCHEMA_VERSION,
            frontend_import: Some(WIRE_SCHEMA_VERSION),
        }
    }

    pub const fn has_frontend_import_payload(self) -> bool {
        self.frontend_import.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConeArtifactFingerprints {
    pub inputs: Vec<u8>,
    pub outputs: Vec<u8>,
}

impl ConeArtifactFingerprints {
    pub fn new(inputs: Vec<u8>, outputs: Vec<u8>) -> Self {
        Self { inputs, outputs }
    }
}

pub fn read_manifest_and_inputs_fingerprint(dir: &Path) -> Result<(ConeArtifactManifest, Vec<u8>)> {
    let manifest: ConeArtifactManifest = read_json(&dir.join(CONE_ARTIFACT_MANIFEST_FILE_NAME))?;
    manifest.ensure_compatible()?;
    let inputs_fingerprint = read_bytes(&dir.join(CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME))?;
    Ok((manifest, inputs_fingerprint))
}

pub fn compute_outputs_fingerprint(dir: &Path) -> Result<Vec<u8>> {
    let mut files = Vec::new();
    collect_artifact_payload_files(dir, dir, &mut files)?;
    files.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

    let mut hasher = Sha256::new();
    hasher.update(b"scoop.cone.artifact.outputs.v0\n");
    for (rel, path) in files {
        hasher.update(rel.as_bytes());
        hasher.update(b"\n");
        let bytes = read_bytes(&path)?;
        hasher.update(Sha256::digest(&bytes));
        hasher.update(b"\n");
    }
    Ok(hasher.finalize().to_vec())
}

pub fn validate_object_file_name(file_name: &str) -> Result<()> {
    let path = Path::new(file_name);
    if file_name.is_empty() || path.components().count() != 1 || path.file_name().is_none() {
        return Err(ConeArtifactMetadataError::InvalidObjectFileName {
            file_name: file_name.to_owned(),
        });
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(ConeArtifactMetadataError::ManifestJson)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| ConeArtifactMetadataError::Io {
        path: path.to_owned(),
        source,
    })
}

fn collect_artifact_payload_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|source| ConeArtifactMetadataError::Io {
        path: dir.to_owned(),
        source,
    })? {
        let entry = entry.map_err(|source| ConeArtifactMetadataError::Io {
            path: dir.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| ConeArtifactMetadataError::Io {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_artifact_payload_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Some(rel) = path.strip_prefix(root).ok().and_then(|rel| rel.to_str()) else {
            continue;
        };
        let rel = rel.replace(std::path::MAIN_SEPARATOR, "/");
        if rel == CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME {
            continue;
        }
        out.push((rel, path));
    }
    Ok(())
}
