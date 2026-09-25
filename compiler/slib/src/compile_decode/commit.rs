//! Typed Compile artifacts and semantic identity commit support.

use std::fmt;
use std::marker::PhantomData;

use scoop_hir::ImportedHirFoundation;
use scoop_identity::{
    ImportedIdentityLayers, SemanticIdentityImport, SemanticIdentityImportError,
    SemanticIdentitySession, SemanticOriginFingerprint, ValidatedIdentityGraph,
};
use scoop_lir::{ImportedLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::ImportedMirFoundation;
use scoop_wire::WireError;

use super::{NativeBoundaryValidatedFoundations, StructurallyValidatedFoundations};
use crate::{
    ArtifactFingerprint, ConeKind, ConeSourceForm, DependencyRecord, SemanticFingerprintRecord,
    ValidatedGraphArtifact,
};

mod profile_seal {
    pub trait Sealed {}
}

/// Marker implemented only by compile capability profiles whose proof chain
/// is available in this compiler.
pub trait CompileCapabilityProfile: profile_seal::Sealed {
    type Production;
}

/// The non-publishable identity-foundation Compile profile implemented by
/// M23-2.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IdentityFoundationProfile;

impl profile_seal::Sealed for IdentityFoundationProfile {}
impl CompileCapabilityProfile for IdentityFoundationProfile {
    type Production = ();
}

/// The M23-3 production Compile profile for one Cone whose definitions all
/// use strong linkage.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SingleConeStrongProfile;

impl profile_seal::Sealed for SingleConeStrongProfile {}
impl CompileCapabilityProfile for SingleConeStrongProfile {
    type Production = crate::ValidatedSingleConeStrongProduction;
}

/// The M23-5 production Compile profile after closure-wide semantic
/// validation and atomic identity import.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CrossConeSemanticsStrongProfile;

impl profile_seal::Sealed for CrossConeSemanticsStrongProfile {}
impl CompileCapabilityProfile for CrossConeSemanticsStrongProfile {
    type Production = crate::ValidatedCrossConeSemanticsProduction;
}

/// A graph artifact whose complete Compile profile was structurally
/// validated, typed-remapped, and committed to the caller's semantic session.
///
/// This type deliberately exposes no publication, dependency, object, or Link
/// conversion.
pub struct ValidatedCompileArtifact<'input, P: CompileCapabilityProfile> {
    graph: ValidatedGraphArtifact<'input>,
    hir: ImportedHirFoundation,
    mir: ImportedMirFoundation,
    lir: ImportedLirFoundation,
    production: P::Production,
    profile: PhantomData<fn() -> P>,
}

impl<'input> NativeBoundaryValidatedFoundations<'input> {
    /// Atomically imports all three layers into one session. Identity ids are
    /// assigned only after every origin and canonical-key conflict check has
    /// passed.
    pub fn commit(
        self,
        session: &mut SemanticIdentitySession,
    ) -> Result<ValidatedCompileArtifact<'input, IdentityFoundationProfile>, CompileCommitError>
    {
        let StructurallyValidatedFoundations {
            mut graph,
            identities,
            hir,
            mir,
            lir,
        } = self.foundations;
        let imported = commit_identity_graph(&mut graph, &identities, session)?;
        let (hir_identities, mir_identities, lir_identities) = imported.into_parts();
        Ok(ValidatedCompileArtifact {
            graph,
            hir: ImportedHirFoundation::from_validated(hir, hir_identities),
            mir: ImportedMirFoundation::from_validated(mir, mir_identities),
            lir: ImportedLirFoundation::from_validated(lir, lir_identities),
            production: (),
            profile: PhantomData,
        })
    }
}

impl<'input, P: CompileCapabilityProfile> ValidatedCompileArtifact<'input, P> {
    pub(crate) fn from_parts(
        graph: ValidatedGraphArtifact<'input>,
        hir: ImportedHirFoundation,
        mir: ImportedMirFoundation,
        lir: ImportedLirFoundation,
        production: P::Production,
    ) -> Self {
        Self {
            graph,
            hir,
            mir,
            lir,
            production,
            profile: PhantomData,
        }
    }

    pub const fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub const fn kind(&self) -> ConeKind {
        self.graph.kind()
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.graph.source_form()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.graph.target_selection()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn compatibility(&self) -> &crate::CompatibilityRecord {
        self.graph.compatibility()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.graph.envelope.manifest().semantic_fingerprints()
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

pub(crate) fn commit_identity_graph(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &ValidatedIdentityGraph,
    session: &mut SemanticIdentitySession,
) -> Result<ImportedIdentityLayers, CompileCommitError> {
    let import = semantic_identity_import(graph, identities);
    session
        .import(import.origin(), import.fingerprint(), import.graph())
        .map_err(CompileCommitError::SemanticImport)
}

pub(crate) fn semantic_identity_import<'a>(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &'a ValidatedIdentityGraph,
) -> SemanticIdentityImport<'a> {
    let semantic = graph.envelope.manifest().semantic_fingerprints();
    let fingerprint = SemanticOriginFingerprint::new(
        *semantic.hir().as_array(),
        *semantic.mir().as_array(),
        *semantic.lir().as_array(),
    );
    SemanticIdentityImport::new(graph.identity(), fingerprint, identities)
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
