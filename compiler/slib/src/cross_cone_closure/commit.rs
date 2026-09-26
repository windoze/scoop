//! Atomic identity commit for one fully validated cross-Cone Compile closure.

use std::fmt;

use scoop_hir::{CallableInterfaceRecordV1, ImportedHirFoundation};
use scoop_identity::{
    CallableTemplateOrigin, ConeCoordinate, ConeIdentity, DependencyCallableDeclarationId,
    PersistentId, SemanticIdentityImport, SemanticIdentityImportError, SemanticIdentitySession,
};
use scoop_lir::{ImportedLirFoundation, ParamFreeLirCallableExportV1, ValidatedLirTargetSelection};
use scoop_mir::{ImportedMirFoundation, ParamFreeMirCallableExportV1};

use super::CrossConeProviderRole;
use crate::{
    CompileCommitError, CrossConeLayoutStrongProfile, ValidatedCompileArtifact,
    ValidatedCrossConeSemanticsProduction,
};

mod projection;
mod world;
pub use projection::*;

/// A complete layout dependency closure whose identity graphs were committed
/// to one semantic session as a single transaction.
///
/// The provider storage is intentionally private. Public lookup can enumerate
/// only `DirectCrossConeSemanticProvider`; support providers expose typed
/// lookup methods but no binding-table iterator.
pub struct ValidatedCrossConeSemanticClosure {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<ValidatedCompileArtifact<CrossConeLayoutStrongProfile>>,
    positions: std::collections::BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn from_layout(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        artifacts: &[std::rc::Rc<crate::PhysicalImportsReplayedCrossConeLayoutSections>],
        session: &mut SemanticIdentitySession,
    ) -> Result<Self, CrossConeSemanticCommitError> {
        let imports = artifacts
            .iter()
            .map(|artifact| {
                let semantic = artifact.manifest().semantic_fingerprints();
                SemanticIdentityImport::new(
                    artifact.identity(),
                    scoop_identity::SemanticOriginFingerprint::new(
                        *semantic.hir().as_array(),
                        *semantic.mir().as_array(),
                        *semantic.lir().as_array(),
                    ),
                    artifact.identity_graph(),
                )
            })
            .collect::<Vec<_>>();
        let imported = session
            .import_batch(&imports)
            .map_err(CrossConeSemanticCommitError::SemanticImport)?;
        let positions = artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| (artifact.identity(), index))
            .collect::<std::collections::BTreeMap<_, _>>();
        let dependency_positions = artifacts
            .iter()
            .map(|artifact| {
                artifact
                    .manifest()
                    .direct_dependencies()
                    .iter()
                    .map(|dependency| positions[&dependency.identity()])
                    .collect()
            })
            .collect();
        let dependency_first = artifacts
            .iter()
            .zip(imported)
            .map(|(artifact, imported)| {
                let (hir, mir, lir) = imported.into_parts();
                ValidatedCompileArtifact::from_parts(
                    artifact.shared_metadata(),
                    artifact.shared_identity_graph(),
                    ImportedHirFoundation::from_odr_free(artifact.hir_foundation().clone(), hir),
                    ImportedMirFoundation::from_odr_free(artifact.mir_foundation().clone(), mir),
                    ImportedLirFoundation::from_odr_free(artifact.lir_foundation().clone(), lir),
                    ValidatedCrossConeSemanticsProduction::new(std::rc::Rc::clone(artifact)),
                )
            })
            .collect();
        Ok(Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        })
    }

    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    /// Borrows the complete dependency records for current-cone type queries.
    pub fn identity_inputs(
        &self,
    ) -> impl Iterator<Item = (&ConeCoordinate, &scoop_identity::ValidatedIdentityGraph)> {
        self.dependency_first
            .iter()
            .filter(|artifact| artifact.identity() != self.current)
            .map(|artifact| (artifact.coordinate(), artifact.identity_graph()))
    }

    /// Complete dependency products retained by the shared artifact reader.
    pub fn layout_dependencies(
        &self,
    ) -> impl Iterator<Item = &crate::PhysicalImportsReplayedCrossConeLayoutSections> {
        self.dependency_first
            .iter()
            .filter(|artifact| artifact.identity() != self.current)
            .map(|artifact| artifact.production().layout())
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn provider_count(&self) -> usize {
        self.dependency_first
            .len()
            .saturating_sub(usize::from(self.positions.contains_key(&self.current)))
    }

    pub fn direct_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<DirectCrossConeSemanticProvider<'_>> {
        self.direct
            .binary_search(&identity)
            .ok()
            .and_then(|_| self.provider(identity))
            .map(|artifact| DirectCrossConeSemanticProvider { artifact })
    }

    pub fn support_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<SupportCrossConeSemanticProvider<'_>> {
        self.direct
            .binary_search(&identity)
            .is_err()
            .then(|| self.provider(identity))
            .flatten()
            .map(|artifact| SupportCrossConeSemanticProvider { artifact })
    }

    pub fn direct_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = DirectCrossConeSemanticProvider<'_>> {
        self.direct.iter().map(|identity| {
            let artifact = self
                .provider(*identity)
                .expect("validated direct providers belong to the committed closure");
            DirectCrossConeSemanticProvider { artifact }
        })
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        if identity == self.current {
            return None;
        }
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&identity)
            .map(|position| self.dependency_positions[*position].len())
    }

    fn provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ValidatedCompileArtifact<CrossConeLayoutStrongProfile>> {
        if identity == self.current {
            return None;
        }
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    /// Returns the completed current artifact when this closure was created
    /// with [`DecodedCrossConeClosure::with_current_artifact`].
    pub fn current_artifact(
        &self,
    ) -> Option<&ValidatedCompileArtifact<CrossConeLayoutStrongProfile>> {
        self.positions
            .get(&self.current)
            .map(|position| &self.dependency_first[*position])
    }

    pub(super) fn artifact_at(
        &self,
        position: usize,
    ) -> &ValidatedCompileArtifact<CrossConeLayoutStrongProfile> {
        &self.dependency_first[position]
    }
}

/// Enumeration-capable view of one validated direct dependency.
#[derive(Clone, Copy)]
pub struct DirectCrossConeSemanticProvider<'closure> {
    artifact: &'closure ValidatedCompileArtifact<CrossConeLayoutStrongProfile>,
}

impl<'closure> DirectCrossConeSemanticProvider<'closure> {
    pub fn coordinate(self) -> &'closure ConeCoordinate {
        self.artifact.coordinate()
    }

    pub fn identity(self) -> ConeIdentity {
        self.artifact.identity()
    }

    pub const fn hir(self) -> &'closure ImportedHirFoundation {
        self.artifact.hir()
    }

    pub const fn mir(self) -> &'closure ImportedMirFoundation {
        self.artifact.mir()
    }

    pub const fn lir(self) -> &'closure ImportedLirFoundation {
        self.artifact.lir()
    }

    pub const fn production(self) -> &'closure ValidatedCrossConeSemanticsProduction {
        self.artifact.production()
    }
}

/// Non-enumerable view of one transitive support provider.
#[derive(Clone, Copy)]
pub struct SupportCrossConeSemanticProvider<'closure> {
    artifact: &'closure ValidatedCompileArtifact<CrossConeLayoutStrongProfile>,
}

impl<'closure> SupportCrossConeSemanticProvider<'closure> {
    pub fn coordinate(self) -> &'closure ConeCoordinate {
        self.artifact.coordinate()
    }

    pub fn identity(self) -> ConeIdentity {
        self.artifact.identity()
    }

    pub fn hir_identity<I: PersistentId + 'static>(
        self,
        identity: I,
    ) -> Option<scoop_hir::ImportedHirId<I>> {
        self.artifact.hir().identity(identity)
    }

    pub fn callable_interface(
        self,
        declaration: CallableTemplateOrigin,
    ) -> Option<&'closure CallableInterfaceRecordV1> {
        self.artifact
            .production()
            .hir_interface()
            .callable_interfaces()
            .get(declaration)
    }

    pub fn mir_callable_export(
        self,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<&'closure ParamFreeMirCallableExportV1> {
        self.artifact
            .production()
            .mir_cross_cone()
            .export(declaration)
    }

    pub fn lir_callable_export(
        self,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<&'closure ParamFreeLirCallableExportV1> {
        self.artifact
            .production()
            .lir_cross_cone()
            .export(declaration)
    }
}

#[derive(Debug)]
pub enum CrossConeSemanticCommitError {
    StateCountMismatch {
        artifacts: usize,
        alias_expansions: usize,
    },
    Allocation {
        requested_slots: usize,
    },
    IdentityPreparation {
        identity: ConeIdentity,
        source: CompileCommitError,
    },
    SemanticImport(SemanticIdentityImportError),
}

impl fmt::Display for CrossConeSemanticCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateCountMismatch {
                artifacts,
                alias_expansions,
            } => write!(
                formatter,
                "validated cross-Cone closure has {artifacts} artifacts but {alias_expansions} alias expansion sets"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot reserve {requested_slots} cross-Cone semantic import slots"
            ),
            Self::IdentityPreparation { identity, source } => {
                write!(
                    formatter,
                    "cannot prepare Cone {identity} for semantic import: {source}"
                )
            }
            Self::SemanticImport(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeSemanticCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IdentityPreparation { source, .. } => Some(source),
            Self::SemanticImport(source) => Some(source),
            Self::StateCountMismatch { .. } | Self::Allocation { .. } => None,
        }
    }
}
