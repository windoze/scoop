//! Per-cone build artifact disk layout and read/write API.
//!
//! A cone artifact is a directory tree rooted at
//! `build/<profile>/cones/<cone-name>@<version>/`. The root contains a JSON
//! manifest with cone identity, compiler version, and schema versions for every
//! persisted product. Stage products are stored next to it as bincode payloads:
//! `hir_facts.bin`, `mir_facts.bin`, `effect_facts.bin`, `lir_program.bin`,
//! and `type_store.bin`; frontend import metadata is stored as
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

pub use scoop_project_model::{
    CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME, CONE_ARTIFACT_FRONTEND_IMPORT_FILE_NAME,
    CONE_ARTIFACT_HIR_FACTS_FILE_NAME, CONE_ARTIFACT_INPUTS_FINGERPRINT_FILE_NAME,
    CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME, CONE_ARTIFACT_MANIFEST_FILE_NAME,
    CONE_ARTIFACT_MIR_FACTS_FILE_NAME, CONE_ARTIFACT_OBJS_DIR_NAME,
    CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME, CONE_ARTIFACT_TYPE_STORE_FILE_NAME,
    ConeArtifactFingerprints, ConeArtifactManifest, ConeArtifactSchemaVersions,
    compute_outputs_fingerprint, validate_object_file_name,
};
use scoop_project_model::{ConeKind, ConeManifest, StableConeKey};
use scoopc_ast as ast;
use scoopc_effect_facts::EffectFacts;
use scoopc_hir::resolve::Index;
use scoopc_hir::session::Session;
use scoopc_hir::typecheck::TypeEnv;
use scoopc_hir_facts::HirFacts;
use scoopc_lir::LateLoweredProgram;
use scoopc_mir_facts::MirFacts;
use scoopc_source::SourceFile;
use scoopc_types::TypeStore;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::annotations::ConeAnnotationClassesFile;
use crate::pre_specialize::ConePreSpecializeFile;
use crate::scoopir::ScoopIrFile;
use crate::visibility::ConeSymbolVisibilityFile;

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
    #[error("failed to encode or decode cone artifact binary payload `{path}`: {source}")]
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

impl From<scoop_project_model::ConeArtifactMetadataError> for ConeArtifactError {
    fn from(value: scoop_project_model::ConeArtifactMetadataError) -> Self {
        match value {
            scoop_project_model::ConeArtifactMetadataError::Io { path, source } => {
                Self::Io { path, source }
            }
            scoop_project_model::ConeArtifactMetadataError::ManifestJson(source) => {
                Self::ManifestEncode(source)
            }
            scoop_project_model::ConeArtifactMetadataError::IncompatibleCompilerVersion {
                expected,
                found,
            } => Self::IncompatibleCompilerVersion { expected, found },
            scoop_project_model::ConeArtifactMetadataError::IncompatibleSchemaVersions {
                expected,
                found,
            } => Self::IncompatibleSchemaVersions { expected, found },
            scoop_project_model::ConeArtifactMetadataError::InvalidObjectFileName { file_name } => {
                Self::InvalidObjectFileName { file_name }
            }
            scoop_project_model::ConeArtifactMetadataError::MissingFrontendImportPayload {
                file_name,
            } => Self::MissingFrontendImportPayload { file_name },
        }
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
    pub lir_program: LateLoweredProgram,
    pub type_store: TypeStore,
}

impl ConeArtifactStageProducts {
    /// Construct the complete set of persisted stage products.
    pub fn new(
        hir_facts: HirFacts,
        mir_facts: MirFacts,
        effect_facts: EffectFacts,
        lir_program: LateLoweredProgram,
        type_store: TypeStore,
    ) -> Self {
        Self {
            hir_facts,
            mir_facts,
            effect_facts,
            lir_program,
            type_store,
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

/// Complete on-disk artifact for one source cone.
#[derive(Debug, Clone)]
pub struct ConeArtifact {
    pub manifest: ConeArtifactManifest,
    pub hir_facts: HirFacts,
    pub mir_facts: MirFacts,
    pub effect_facts: EffectFacts,
    pub lir_program: LateLoweredProgram,
    pub type_store: TypeStore,
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
        lir_program: LateLoweredProgram,
        type_store: TypeStore,
        frontend_import: ConeArtifactFrontendImport,
    ) -> Self {
        Self::with_parts(
            cone,
            cone_kind,
            ConeArtifactStageProducts::new(
                hir_facts,
                mir_facts,
                effect_facts,
                lir_program,
                type_store,
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
            lir_program: products.lir_program,
            type_store: products.type_store,
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
            &dir.join(CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME),
            &self.lir_program,
        )?;
        write_bincode(
            &dir.join(CONE_ARTIFACT_TYPE_STORE_FILE_NAME),
            &self.type_store,
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
            lir_program: read_bincode(&dir.join(CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME))?,
            type_store: read_bincode(&dir.join(CONE_ARTIFACT_TYPE_STORE_FILE_NAME))?,
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

    /// Peek at the manifest plus `inputs.fingerprint` of an artifact directory.
    ///
    /// Used by subprocess single-cone driver setup to map upstream artifacts to
    /// dep cones without paying the full bincode read cost; the cache hit path
    /// will re-read the artifact in full when its dep cone unit is iterated.
    pub fn read_manifest_and_inputs_fingerprint(
        dir: &Path,
    ) -> Result<(ConeArtifactManifest, Vec<u8>)> {
        scoop_project_model::read_manifest_and_inputs_fingerprint(dir).map_err(Into::into)
    }
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

#[cfg(test)]
mod tests {
    use scoopc_lir::LateLoweredProgram;
    use scoopc_types::{WIRE_SCHEMA_VERSION, WireSchemaVersion};
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
        assert_eq!(decoded.manifest.compiler_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            decoded.manifest.schema_versions,
            ConeArtifactSchemaVersions::current()
        );
        assert_eq!(decoded.hir_facts, artifact.hir_facts);
        assert_eq!(decoded.mir_facts, artifact.mir_facts);
        assert_eq!(decoded.effect_facts, artifact.effect_facts);
        assert!(decoded.lir_program.is_empty());
        assert_eq!(decoded.type_store, artifact.type_store);
        assert_eq!(decoded.frontend_import, artifact.frontend_import);
        assert_eq!(decoded.objects, artifact.objects);
        assert_eq!(decoded.inputs_fingerprint, artifact.inputs_fingerprint);
        assert_eq!(decoded.outputs_fingerprint, artifact.outputs_fingerprint);

        for relative in [
            CONE_ARTIFACT_MANIFEST_FILE_NAME,
            CONE_ARTIFACT_HIR_FACTS_FILE_NAME,
            CONE_ARTIFACT_MIR_FACTS_FILE_NAME,
            CONE_ARTIFACT_EFFECT_FACTS_FILE_NAME,
            CONE_ARTIFACT_LIR_PROGRAM_FILE_NAME,
            CONE_ARTIFACT_TYPE_STORE_FILE_NAME,
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
                LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                TypeStore::new(),
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
                    is_interior_mutable: false,
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
