//! Per-cone build artifact disk layout and read/write API.
//!
//! A cone artifact is a directory tree rooted at
//! `build/<profile>/cones/<cone-name>@<version>/`. The root contains a JSON
//! manifest with cone identity, compiler version, and schema versions for every
//! persisted product. Stage products are stored next to it as bincode payloads:
//! `hir_facts.bin`, `mir_facts.bin`, `effect_facts.bin`, `lir_facts.bin`, and
//! `lir_program.bin`; frontend import metadata is stored as
//! `frontend_import.json` because it reuses the existing JSON-oriented `.cone`
//! API schemas. Object files live under `objs/`, while
//! `inputs.fingerprint` and `outputs.fingerprint` record cache identity.
//!
//! Compatibility is intentionally coarse-grained: if the compiler version or any
//! persisted schema version in `manifest.json` is incompatible with the current
//! compiler, or if the frontend import payload is absent, the whole cone should
//! be rebuilt instead of attempting partial migration.

use std::fs;
use std::path::{Path, PathBuf};

use scoopc_ast as ast;
use scoopc_effect_facts::EffectFacts;
use scoopc_hir::resolve::Index;
use scoopc_hir::session::Session;
use scoopc_hir::typecheck::TypeEnv;
use scoopc_hir_facts::HirFacts;
use scoopc_lir::LateLoweredProgram;
use scoopc_lir_facts::LirFacts;
use scoopc_mir_facts::MirFacts;
use scoopc_project_model::{ConeKind, ConeManifest, StableConeKey};
use scoopc_source::SourceFile;
use scoopc_types::{WIRE_SCHEMA_VERSION, WireSchemaVersion};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::annotations::ConeAnnotationClassesFile;
use crate::pre_specialize::ConePreSpecializeFile;
use crate::scoopir::ScoopIrFile;
use crate::visibility::ConeSymbolVisibilityFile;

pub const CONE_ARTIFACT_MANIFEST_FILE_NAME: &str = "manifest.json";
pub const CONE_ARTIFACT_HIR_FACTS_FILE_NAME: &str = "hir_facts.bin";
pub const CONE_ARTIFACT_MIR_FACTS_FILE_NAME: &str = "mir_facts.bin";
pub const CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME: &str = "effect_facts.bin";
pub const CONE_ARTIFACT_LIR_FACTS_FILE_NAME: &str = "lir_facts.bin";
pub const CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME: &str = "lir_program.bin";
pub const CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME: &str = "frontend_import.json";
pub const CONE_ARTIFACT_OBJS_DIR_NAME: &str = "objs";
pub const CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME: &str = "inputs.fingerprint";
pub const CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME: &str = "outputs.fingerprint";

const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result type for per-cone artifact IO.
pub type Result<T> = std::result::Result<T, ConeArtifactError>;

/// Errors reported while reading or writing a cone artifact directory.
#[derive(Debug, Error)]
pub enum ConeArtifactError {
    #[error("failed to access cone artifact path `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode cone artifact JSON manifest")]
    ManifestEncode(#[from] serde_json::Error),
    #[error("failed to encode or decode cone artifact binary payload `{path}`")]
    Binary {
        path: PathBuf,
        #[source]
        source: Box<bincode::ErrorKind>,
    },
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
    #[error(
        "cone artifact inputs fingerprint mismatch (expected {expected:?}, found {found:?}); rebuild the cone"
    )]
    InputsFingerprintMismatch { expected: Vec<u8>, found: Vec<u8> },
}

/// JSON metadata stored in `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeArtifactManifest {
    pub cone_name: String,
    pub cone_version: String,
    /// Cone kind (lib/bin/syslib) — needed by downstream stages that rebuild Index/TypeEnv
    /// from `compilation_sources` so they can reattach this cone's synthetic `decl_file`
    /// to the correct `ConeKind` (P10-T04-b).
    pub cone_kind: ConeKind,
    pub compiler_version: String,
    pub schema_versions: ConeArtifactSchemaVersions,
    pub object_files: Vec<String>,
}

impl ConeArtifactManifest {
    /// Build manifest metadata for the current compiler and wire schema.
    pub fn current(cone: &StableConeKey, cone_kind: ConeKind, object_files: Vec<String>) -> Self {
        Self {
            cone_name: cone.name().to_owned(),
            cone_version: cone.version().to_owned(),
            cone_kind,
            compiler_version: COMPILER_VERSION.to_owned(),
            schema_versions: ConeArtifactSchemaVersions::current(),
            object_files,
        }
    }

    /// Return this manifest's stable cone identity.
    pub fn stable_cone_key(&self) -> StableConeKey {
        StableConeKey::new(&self.cone_name, &self.cone_version)
    }

    /// Reject artifacts produced by an incompatible compiler or wire schema.
    pub fn ensure_compatible(&self) -> Result<()> {
        if self.compiler_version != COMPILER_VERSION {
            return Err(ConeArtifactError::IncompatibleCompilerVersion {
                expected: COMPILER_VERSION.to_owned(),
                found: self.compiler_version.clone(),
            });
        }
        if !self.schema_versions.has_frontend_import_payload() {
            return Err(ConeArtifactError::MissingFrontendImportPayload {
                file_name: CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
            });
        }
        let expected = ConeArtifactSchemaVersions::current();
        if self.schema_versions != expected {
            return Err(ConeArtifactError::IncompatibleSchemaVersions {
                expected,
                found: self.schema_versions,
            });
        }
        Ok(())
    }
}

/// Schema versions for every persisted fact and LIR payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeArtifactSchemaVersions {
    pub hir_facts: WireSchemaVersion,
    pub mir_facts: WireSchemaVersion,
    pub effect_facts: WireSchemaVersion,
    pub lir_facts: WireSchemaVersion,
    pub lir_program: WireSchemaVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend_import: Option<WireSchemaVersion>,
}

impl ConeArtifactSchemaVersions {
    /// Use the currently linked wire schema for every persisted payload.
    pub const fn current() -> Self {
        Self {
            hir_facts: WIRE_SCHEMA_VERSION,
            mir_facts: WIRE_SCHEMA_VERSION,
            effect_facts: WIRE_SCHEMA_VERSION,
            lir_facts: WIRE_SCHEMA_VERSION,
            lir_program: WIRE_SCHEMA_VERSION,
            frontend_import: Some(WIRE_SCHEMA_VERSION),
        }
    }

    /// Return whether this manifest declares the required frontend import payload.
    pub const fn has_frontend_import_payload(self) -> bool {
        self.frontend_import.is_some()
    }
}

/// Object file payload stored under the artifact `objs/` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeArtifactObject {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

impl ConeArtifactObject {
    /// Construct an object entry and reject names that would escape `objs/`.
    pub fn new(file_name: impl Into<String>, bytes: Vec<u8>) -> Result<Self> {
        let file_name = file_name.into();
        validate_object_file_name(&file_name)?;
        Ok(Self { file_name, bytes })
    }
}

/// Persisted stage products stored as bincode files in the artifact root.
#[derive(Debug, Clone)]
pub struct ConeArtifactStageProducts {
    pub hir_facts: HirFacts,
    pub mir_facts: MirFacts,
    pub effect_facts: EffectFacts,
    pub lir_facts: LirFacts,
    pub lir_program: LateLoweredProgram,
}

impl ConeArtifactStageProducts {
    /// Construct the complete set of persisted stage products.
    pub fn new(
        hir_facts: HirFacts,
        mir_facts: MirFacts,
        effect_facts: EffectFacts,
        lir_facts: LirFacts,
        lir_program: LateLoweredProgram,
    ) -> Self {
        Self {
            hir_facts,
            mir_facts,
            effect_facts,
            lir_facts,
            lir_program,
        }
    }
}

/// Frontend-facing import data persisted with a cone artifact.
///
/// This reuses the existing `.cone` schemas so downstream frontend injection can
/// consume an artifact without re-reading upstream source or re-exporting
/// temporary ScoopIR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeArtifactFrontendImport {
    pub public_api: ScoopIrFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_classes: Option<ConeAnnotationClassesFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_visibility: Option<ConeSymbolVisibilityFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_specialize: Option<ConePreSpecializeFile>,
}

impl ConeArtifactFrontendImport {
    /// Construct a frontend import payload from already exported cone metadata.
    pub fn new(
        public_api: ScoopIrFile,
        annotation_classes: Option<ConeAnnotationClassesFile>,
        symbol_visibility: Option<ConeSymbolVisibilityFile>,
        pre_specialize: Option<ConePreSpecializeFile>,
    ) -> Self {
        Self {
            public_api,
            annotation_classes,
            symbol_visibility,
            pre_specialize,
        }
    }

    /// Construct an empty import payload for cones with no exported frontend API.
    pub fn empty() -> Self {
        Self::new(
            ScoopIrFile::new_v0(Vec::new(), Vec::new()),
            None,
            None,
            None,
        )
    }
}

/// Input/output fingerprint payloads stored next to stage products.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConeArtifactFingerprints {
    pub inputs: Vec<u8>,
    pub outputs: Vec<u8>,
}

impl ConeArtifactFingerprints {
    /// Construct artifact fingerprints from precomputed bytes.
    pub fn new(inputs: Vec<u8>, outputs: Vec<u8>) -> Self {
        Self { inputs, outputs }
    }
}

/// Complete on-disk artifact for one source cone.
#[derive(Debug, Clone)]
pub struct ConeArtifact {
    pub manifest: ConeArtifactManifest,
    pub hir_facts: HirFacts,
    pub mir_facts: MirFacts,
    pub effect_facts: EffectFacts,
    pub lir_facts: LirFacts,
    pub lir_program: LateLoweredProgram,
    pub frontend_import: ConeArtifactFrontendImport,
    pub objects: Vec<ConeArtifactObject>,
    pub inputs_fingerprint: Vec<u8>,
    pub outputs_fingerprint: Vec<u8>,
}

impl ConeArtifact {
    /// Construct an artifact with current manifest schema metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cone: StableConeKey,
        cone_kind: ConeKind,
        hir_facts: HirFacts,
        mir_facts: MirFacts,
        effect_facts: EffectFacts,
        lir_facts: LirFacts,
        lir_program: LateLoweredProgram,
        frontend_import: ConeArtifactFrontendImport,
    ) -> Self {
        Self::with_parts(
            cone,
            cone_kind,
            ConeArtifactStageProducts::new(
                hir_facts,
                mir_facts,
                effect_facts,
                lir_facts,
                lir_program,
            ),
            frontend_import,
            Vec::new(),
            ConeArtifactFingerprints::default(),
        )
    }

    /// Construct an artifact with object and fingerprint payloads.
    pub fn with_parts(
        cone: StableConeKey,
        cone_kind: ConeKind,
        products: ConeArtifactStageProducts,
        frontend_import: ConeArtifactFrontendImport,
        objects: Vec<ConeArtifactObject>,
        fingerprints: ConeArtifactFingerprints,
    ) -> Self {
        let object_files = objects
            .iter()
            .map(|object| object.file_name.clone())
            .collect();
        Self {
            manifest: ConeArtifactManifest::current(&cone, cone_kind, object_files),
            hir_facts: products.hir_facts,
            mir_facts: products.mir_facts,
            effect_facts: products.effect_facts,
            lir_facts: products.lir_facts,
            lir_program: products.lir_program,
            frontend_import,
            objects,
            inputs_fingerprint: fingerprints.inputs,
            outputs_fingerprint: fingerprints.outputs,
        }
    }

    /// Write this artifact into `dir`, creating the documented layout.
    pub fn write(&self, dir: &Path) -> Result<()> {
        create_dir_all(dir)?;
        let objs_dir = dir.join(CONE_ARTIFACT_OBJS_DIR_NAME);
        create_dir_all(&objs_dir)?;

        let mut manifest = self.manifest.clone();
        manifest.object_files = self
            .objects
            .iter()
            .map(|object| object.file_name.clone())
            .collect();

        write_json(&dir.join(CONE_ARTIFACT_MANIFEST_FILE_NAME), &manifest)?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_HIR_FACTS_FILE_NAME),
            &self.hir_facts,
        )?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_MIR_FACTS_FILE_NAME),
            &self.mir_facts,
        )?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME),
            &self.effect_facts,
        )?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_LIR_FACTS_FILE_NAME),
            &self.lir_facts,
        )?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME),
            &self.lir_program,
        )?;
        write_json(
            &dir.join(CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME),
            &self.frontend_import,
        )?;
        write_bytes(
            &dir.join(CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME),
            &self.inputs_fingerprint,
        )?;
        write_bytes(
            &dir.join(CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME),
            &self.outputs_fingerprint,
        )?;

        for object in &self.objects {
            validate_object_file_name(&object.file_name)?;
            write_bytes(&objs_dir.join(&object.file_name), &object.bytes)?;
        }

        Ok(())
    }

    /// Write this artifact and fill `outputs.fingerprint` from the written payloads.
    ///
    /// The output fingerprint excludes `outputs.fingerprint` itself to avoid a
    /// self-referential digest.
    pub fn write_with_computed_outputs_fingerprint(&mut self, dir: &Path) -> Result<()> {
        self.outputs_fingerprint.clear();
        self.write(dir)?;
        self.outputs_fingerprint = compute_outputs_fingerprint(dir)?;
        write_bytes(
            &dir.join(CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME),
            &self.outputs_fingerprint,
        )
    }

    /// Read a complete cone artifact from `dir`.
    pub fn read(dir: &Path) -> Result<Self> {
        let manifest: ConeArtifactManifest =
            read_json(&dir.join(CONE_ARTIFACT_MANIFEST_FILE_NAME))?;
        manifest.ensure_compatible()?;
        let objs_dir = dir.join(CONE_ARTIFACT_OBJS_DIR_NAME);
        let mut objects = Vec::with_capacity(manifest.object_files.len());
        for file_name in &manifest.object_files {
            validate_object_file_name(file_name)?;
            objects.push(ConeArtifactObject {
                file_name: file_name.clone(),
                bytes: read_bytes(&objs_dir.join(file_name))?,
            });
        }

        let frontend_import_path = dir.join(CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME);
        if !frontend_import_path.exists() {
            return Err(ConeArtifactError::MissingFrontendImportPayload {
                file_name: CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
            });
        }

        Ok(Self {
            hir_facts: read_bincode(&dir.join(CONE_ARTIFACT_HIR_FACTS_FILE_NAME))?,
            mir_facts: read_bincode(&dir.join(CONE_ARTIFACT_MIR_FACTS_FILE_NAME))?,
            effect_facts: read_bincode(&dir.join(CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME))?,
            lir_facts: read_bincode(&dir.join(CONE_ARTIFACT_LIR_FACTS_FILE_NAME))?,
            lir_program: read_bincode(&dir.join(CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME))?,
            frontend_import: read_json(&frontend_import_path)?,
            inputs_fingerprint: read_bytes(&dir.join(CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME))?,
            outputs_fingerprint: read_bytes(
                &dir.join(CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME),
            )?,
            manifest,
            objects,
        })
    }

    /// Read a complete cone artifact and require an exact inputs fingerprint match.
    pub fn read_with_inputs_fingerprint(dir: &Path, expected: &[u8]) -> Result<Self> {
        let artifact = Self::read(dir)?;
        if artifact.inputs_fingerprint != expected {
            return Err(ConeArtifactError::InputsFingerprintMismatch {
                expected: expected.to_vec(),
                found: artifact.inputs_fingerprint,
            });
        }
        Ok(artifact)
    }
}

/// Compute the output fingerprint for an already-written artifact directory.
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

/// Build the frontend import payload for a cone from already processed frontend state.
pub fn build_frontend_import_for_typechecked_cone(
    session: &Session,
    sources: &[SourceFile],
    asts: &[ast::File],
    manifest: &ConeManifest,
    index: &Index,
    env: &TypeEnv,
    lowering_context_files: &[(&SourceFile, &ast::File)],
) -> miette::Result<ConeArtifactFrontendImport> {
    let public_api = crate::scoopir::export_public_api_for_typechecked_cone_sources(
        sources,
        asts,
        manifest,
        index,
        env,
        lowering_context_files,
    )?;
    let annotation_classes =
        crate::annotations::collect_cone_preserved_annotation_classes_from_index_env(
            sources, index, env,
        );
    let symbol_visibility =
        crate::visibility::collect_non_public_symbols_from_index(sources, index);
    let pre_specialize = crate::pre_specialize::build_pre_specialize_file_for_cone_sources(
        session, sources, manifest,
    )?;

    Ok(ConeArtifactFrontendImport::new(
        public_api,
        Some(annotation_classes),
        Some(symbol_visibility),
        pre_specialize,
    ))
}

fn validate_object_file_name(file_name: &str) -> Result<()> {
    let path = Path::new(file_name);
    if file_name.is_empty() || path.components().count() != 1 || path.file_name().is_none() {
        return Err(ConeArtifactError::InvalidObjectFileName {
            file_name: file_name.to_owned(),
        });
    }
    Ok(())
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| ConeArtifactError::Io {
        path: path.to_owned(),
        source,
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes(path, &bytes)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(ConeArtifactError::ManifestEncode)
}

fn write_bincode<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = bincode::serialize(value).map_err(|source| ConeArtifactError::Binary {
        path: path.to_owned(),
        source,
    })?;
    write_bytes(path, &bytes)
}

fn read_bincode<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read_bytes(path)?;
    bincode::deserialize(&bytes).map_err(|source| ConeArtifactError::Binary {
        path: path.to_owned(),
        source,
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|source| ConeArtifactError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| ConeArtifactError::Io {
        path: path.to_owned(),
        source,
    })
}

fn collect_artifact_payload_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|source| ConeArtifactError::Io {
        path: dir.to_owned(),
        source,
    })? {
        let entry = entry.map_err(|source| ConeArtifactError::Io {
            path: dir.to_owned(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| ConeArtifactError::Io {
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
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        if rel == CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME {
            continue;
        }
        out.push((rel, path));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoopc_lir::LateLoweredProgram;
    use scoopc_project_model::OptLevel;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn cone_artifact_round_trip_preserves_stage_products_and_layout() {
        let dir = tempdir().expect("create temp dir");
        let cone = StableConeKey::new("upstream-lib", "1.2.3");
        let artifact = sample_artifact(cone.clone());

        artifact.write(dir.path()).expect("write artifact");
        let decoded = ConeArtifact::read(dir.path()).expect("read artifact");

        assert_eq!(decoded.manifest.stable_cone_key(), cone);
        assert_eq!(decoded.manifest.compiler_version, COMPILER_VERSION);
        assert_eq!(
            decoded.manifest.schema_versions,
            ConeArtifactSchemaVersions::current()
        );
        assert_eq!(decoded.hir_facts, artifact.hir_facts);
        assert_eq!(decoded.mir_facts, artifact.mir_facts);
        assert_eq!(decoded.effect_facts, artifact.effect_facts);
        assert_eq!(decoded.lir_facts, artifact.lir_facts);
        assert!(decoded.lir_program.is_empty());
        assert_eq!(decoded.frontend_import, artifact.frontend_import);
        assert_eq!(decoded.objects, artifact.objects);
        assert_eq!(decoded.inputs_fingerprint, artifact.inputs_fingerprint);
        assert_eq!(decoded.outputs_fingerprint, artifact.outputs_fingerprint);

        for relative in [
            CONE_ARTIFACT_MANIFEST_FILE_NAME,
            CONE_ARTIFACT_HIR_FACTS_FILE_NAME,
            CONE_ARTIFACT_MIR_FACTS_FILE_NAME,
            CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME,
            CONE_ARTIFACT_LIR_FACTS_FILE_NAME,
            CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME,
            CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
            CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME,
            CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME,
        ] {
            assert!(dir.path().join(relative).is_file(), "missing {relative}");
        }
        assert!(
            dir.path()
                .join(CONE_ARTIFACT_OBJS_DIR_NAME)
                .join("scoop.o")
                .is_file()
        );
        assert!(
            dir.path()
                .join(CONE_ARTIFACT_OBJS_DIR_NAME)
                .join("native_runtime.o")
                .is_file()
        );
    }

    #[test]
    fn read_rejects_incompatible_compiler_version() {
        let dir = tempdir().expect("create temp dir");
        let artifact = sample_artifact(StableConeKey::new("upstream-lib", "1.2.3"));
        artifact.write(dir.path()).expect("write artifact");

        let manifest_path = dir.path().join(CONE_ARTIFACT_MANIFEST_FILE_NAME);
        let mut manifest: ConeArtifactManifest = read_json(&manifest_path).expect("read manifest");
        manifest.compiler_version = "0.0.0".to_owned();
        write_json(&manifest_path, &manifest).expect("write manifest");

        let error = ConeArtifact::read(dir.path()).expect_err("incompatible compiler version");
        assert!(matches!(
            error,
            ConeArtifactError::IncompatibleCompilerVersion { found, .. } if found == "0.0.0"
        ));
    }

    #[test]
    fn read_rejects_incompatible_schema_versions() {
        let dir = tempdir().expect("create temp dir");
        let artifact = sample_artifact(StableConeKey::new("upstream-lib", "1.2.3"));
        artifact.write(dir.path()).expect("write artifact");

        let manifest_path = dir.path().join(CONE_ARTIFACT_MANIFEST_FILE_NAME);
        let mut manifest: ConeArtifactManifest = read_json(&manifest_path).expect("read manifest");
        manifest.schema_versions.hir_facts =
            WireSchemaVersion::new(WIRE_SCHEMA_VERSION.major + 1, WIRE_SCHEMA_VERSION.minor);
        write_json(&manifest_path, &manifest).expect("write manifest");

        let error = ConeArtifact::read(dir.path()).expect_err("incompatible schema versions");
        assert!(matches!(
            error,
            ConeArtifactError::IncompatibleSchemaVersions { found, .. }
                if found.hir_facts.major == WIRE_SCHEMA_VERSION.major + 1
        ));
    }

    #[test]
    fn read_rejects_missing_frontend_import_payload() {
        let dir = tempdir().expect("create temp dir");
        let artifact = sample_artifact(StableConeKey::new("upstream-lib", "1.2.3"));
        artifact.write(dir.path()).expect("write artifact");
        fs::remove_file(dir.path().join(CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME))
            .expect("remove frontend import payload");

        let error = ConeArtifact::read(dir.path()).expect_err("missing frontend import payload");
        assert!(matches!(
            error,
            ConeArtifactError::MissingFrontendImportPayload {
                file_name: CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME
            }
        ));
    }

    #[test]
    fn read_rejects_manifest_without_frontend_import_schema_version() {
        let dir = tempdir().expect("create temp dir");
        let artifact = sample_artifact(StableConeKey::new("upstream-lib", "1.2.3"));
        artifact.write(dir.path()).expect("write artifact");

        let manifest_path = dir.path().join(CONE_ARTIFACT_MANIFEST_FILE_NAME);
        let mut manifest: ConeArtifactManifest = read_json(&manifest_path).expect("read manifest");
        manifest.schema_versions.frontend_import = None;
        write_json(&manifest_path, &manifest).expect("write manifest");

        let error =
            ConeArtifact::read(dir.path()).expect_err("missing frontend import schema version");
        assert!(matches!(
            error,
            ConeArtifactError::MissingFrontendImportPayload {
                file_name: CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME
            }
        ));
    }

    #[test]
    fn object_file_names_must_stay_inside_objs_dir() {
        assert!(ConeArtifactObject::new("../escape.o", Vec::new()).is_err());
        assert!(ConeArtifactObject::new("nested/escape.o", Vec::new()).is_err());
        assert!(ConeArtifactObject::new("", Vec::new()).is_err());
    }

    fn sample_artifact(cone: StableConeKey) -> ConeArtifact {
        ConeArtifact::with_parts(
            cone,
            ConeKind::Lib,
            ConeArtifactStageProducts::new(
                HirFacts::new(),
                MirFacts::new(),
                EffectFacts::new(),
                LirFacts::new(OptLevel::O2),
                LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            ),
            sample_frontend_import(),
            vec![
                ConeArtifactObject::new("scoop.o", b"scoop object".to_vec())
                    .expect("valid scoop object"),
                ConeArtifactObject::new("native_runtime.o", b"native object".to_vec())
                    .expect("valid native object"),
            ],
            ConeArtifactFingerprints::new(
                b"inputs:fingerprint".to_vec(),
                b"outputs:fingerprint".to_vec(),
            ),
        )
    }

    fn sample_frontend_import() -> ConeArtifactFrontendImport {
        ConeArtifactFrontendImport::new(
            ScoopIrFile::new_v0(
                vec![crate::scoopir::IrTypeDecl {
                    fqn: "upstream.Token".to_owned(),
                    kind: crate::scoopir::IrTypeDeclKind::Struct,
                    type_params: Vec::new(),
                    alias_of: None,
                }],
                vec![crate::scoopir::IrFunDecl {
                    fqn: "upstream.make_token".to_owned(),
                    kind: crate::scoopir::IrFunDeclKind::Regular,
                    type_params: Vec::new(),
                    receiver: None,
                    params: Vec::new(),
                    return_ty: crate::scoopir::IrType::Named {
                        fqn: "upstream.Token".to_owned(),
                        args: Vec::new(),
                        eff: None,
                    },
                    effects: crate::scoopir::IrEffectRow::default(),
                }],
            ),
            Some(ConeAnnotationClassesFile::new_v0(vec![
                crate::annotations::ConeAnnotationClassEntry {
                    fqn: "upstream.Trace".to_owned(),
                    targets: Some(vec!["fun".to_owned()]),
                    retention: "cone".to_owned(),
                },
            ])),
            Some(ConeSymbolVisibilityFile::new_v0(vec![
                crate::visibility::ConeSymbolVisibilityEntry {
                    kind: crate::visibility::ConeSymbolKind::Fun,
                    fqn: "upstream.hidden".to_owned(),
                    visibility: crate::visibility::ConeSymbolVisibility::Internal,
                },
            ])),
            Some(ConePreSpecializeFile::new_v0(
                vec![crate::pre_specialize::PreSpecializedFunInstance {
                    key: crate::pre_specialize::PreSpecializedFunKey {
                        fqn: "upstream.id".to_owned(),
                        type_args: vec!["Int".to_owned()],
                    },
                    instance_fqn: "upstream.id::<Int>".to_owned(),
                    mir_debug: "mir".to_owned(),
                }],
                vec![crate::pre_specialize::PreSpecializedTypeInstance {
                    key: crate::pre_specialize::PreSpecializedTypeKey {
                        fqn: "upstream.Box".to_owned(),
                        type_args: vec!["Int".to_owned()],
                    },
                    instance_fqn: "upstream.Box::<Int>".to_owned(),
                }],
            )),
        )
    }
}
