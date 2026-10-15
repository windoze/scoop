//! Typed Compile artifacts and semantic identity commit support.

use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use scoop_hir::ImportedHirFoundation;
use scoop_identity::{SemanticIdentityImportError, ValidatedIdentityGraph};
use scoop_lir::{ImportedLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::ImportedMirFoundation;
use scoop_wire::WireError;

use crate::{
    ArtifactFingerprint, ConeKind, ConeSourceForm, DependencyRecord, SemanticFingerprintRecord,
};

mod profile_seal {
    pub trait Sealed {}
}

/// The complete production data carried by each supported Compile profile.
pub trait CompileCapabilityProfile: profile_seal::Sealed {
    type Production;
}

/// The complete M23-6 type, layout and object profile in one semantic session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CrossConeGenericProfile;

impl profile_seal::Sealed for CrossConeGenericProfile {}
impl CompileCapabilityProfile for CrossConeGenericProfile {
    type Production = crate::ValidatedCrossConeSemanticsProduction;
}

/// A graph artifact whose complete Compile profile was structurally
/// validated, typed-remapped, and committed to the caller's semantic session.
///
/// This type deliberately exposes no publication, dependency, object, or Link
/// conversion.
pub struct ValidatedCompileArtifact<P: CompileCapabilityProfile> {
    metadata: Rc<crate::graph::ArtifactMetadata>,
    identities: Rc<ValidatedIdentityGraph>,
    hir: ImportedHirFoundation,
    mir: ImportedMirFoundation,
    lir: ImportedLirFoundation,
    production: P::Production,
    profile: PhantomData<fn() -> P>,
}

impl<P: CompileCapabilityProfile> ValidatedCompileArtifact<P> {
    pub(crate) fn from_parts(
        metadata: Rc<crate::graph::ArtifactMetadata>,
        identities: Rc<ValidatedIdentityGraph>,
        hir: ImportedHirFoundation,
        mir: ImportedMirFoundation,
        lir: ImportedLirFoundation,
        production: P::Production,
    ) -> Self {
        Self {
            metadata,
            identities,
            hir,
            mir,
            lir,
            production,
            profile: PhantomData,
        }
    }

    pub(crate) fn identity_graph(&self) -> &ValidatedIdentityGraph {
        &self.identities
    }

    pub fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.metadata.coordinate()
    }

    pub fn identity(&self) -> scoop_identity::ConeIdentity {
        self.metadata.identity()
    }

    pub fn kind(&self) -> ConeKind {
        self.metadata.kind()
    }

    pub fn source_form(&self) -> ConeSourceForm {
        self.metadata.source_form()
    }

    pub fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.metadata.target_selection()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.metadata.direct_dependencies()
    }

    pub fn compatibility(&self) -> &crate::CompatibilityRecord {
        self.metadata.compatibility()
    }

    pub fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.metadata.artifact_fingerprint()
    }

    pub fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.metadata.manifest.semantic_fingerprints()
    }

    /// Projects the exact graph/semantic authority of this validated artifact
    /// into the record consumed by a direct dependent.
    pub fn dependency_record(&self) -> DependencyRecord {
        let semantic = self.semantic_fingerprints();
        DependencyRecord::from_validated(
            self.coordinate().clone(),
            self.identity(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
        )
    }

    pub const fn hir(&self) -> &ImportedHirFoundation {
        &self.hir
    }

    pub const fn mir(&self) -> &ImportedMirFoundation {
        &self.mir
    }

    pub const fn lir(&self) -> &ImportedLirFoundation {
        &self.lir
    }

    pub const fn production(&self) -> &P::Production {
        &self.production
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCommitError {
    Resource(WireError),
    SemanticImport(SemanticIdentityImportError),
}

impl fmt::Display for CompileCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::SemanticImport(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::SemanticImport(error) => Some(error),
        }
    }
}
