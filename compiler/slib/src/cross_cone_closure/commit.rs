//! Atomic identity commit for one fully validated cross-Cone Compile closure.

use std::fmt;

use scoop_hir::{CallableInterfaceRecordV1, ImportedHirFoundation};
use scoop_identity::{
    CallableTemplateOrigin, ConeCoordinate, ConeIdentity, DependencyCallableDeclarationId,
    PersistentId, SemanticIdentityImport, SemanticIdentityImportError, SemanticIdentitySession,
};
use scoop_lir::{ImportedLirFoundation, ParamFreeLirCallableExportV1, ValidatedLirTargetSelection};
use scoop_mir::{ImportedMirFoundation, ParamFreeMirCallableExportV1};

use super::{CrossConeProviderRole, LirBridgeValidatedCrossConeHirClosure};
use crate::{
    CompileCommitError, CrossConeSemanticsStrongProfile, ValidatedCompileArtifact,
    ValidatedCrossConeSemanticsProduction,
};

/// A complete M23-5 dependency closure whose identity graphs were committed
/// to one semantic session as a single transaction.
///
/// The provider storage is intentionally private. Public lookup can enumerate
/// only `DirectCrossConeSemanticProvider`; support providers expose typed
/// lookup methods but no binding-table iterator.
pub struct ValidatedCrossConeSemanticClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile>>,
    positions: std::collections::BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> LirBridgeValidatedCrossConeHirClosure<'input> {
    /// Atomically imports every validated provider identity graph.
    ///
    /// Decode-budget charging happens before the semantic-session mutation;
    /// `SemanticIdentitySession::import_batch` stages the complete batch and
    /// leaves the session untouched if any origin or canonical key conflicts.
    pub fn commit(
        mut self,
        session: &mut SemanticIdentitySession,
    ) -> Result<ValidatedCrossConeSemanticClosure<'input>, CrossConeSemanticCommitError> {
        let artifact_count = self.dependency_first.len();
        if self.type_alias_expansions.len() != artifact_count {
            return Err(CrossConeSemanticCommitError::StateCountMismatch {
                artifacts: artifact_count,
                alias_expansions: self.type_alias_expansions.len(),
            });
        }

        for front in &mut self.dependency_first {
            let identity = front.identity();
            front.charge_identity_import().map_err(|source| {
                CrossConeSemanticCommitError::IdentityPreparation { identity, source }
            })?;
        }

        let mut dependency_first = Vec::new();
        dependency_first
            .try_reserve_exact(artifact_count)
            .map_err(|_| CrossConeSemanticCommitError::Allocation {
                requested_slots: artifact_count,
            })?;
        let imported = {
            let mut imports = Vec::<SemanticIdentityImport<'_>>::new();
            imports.try_reserve_exact(artifact_count).map_err(|_| {
                CrossConeSemanticCommitError::Allocation {
                    requested_slots: artifact_count,
                }
            })?;
            imports.extend(
                self.dependency_first
                    .iter()
                    .map(|front| front.semantic_identity_import()),
            );
            session
                .import_batch(&imports)
                .map_err(CrossConeSemanticCommitError::SemanticImport)?
        };

        dependency_first.extend(
            self.dependency_first
                .into_iter()
                .zip(imported)
                .zip(self.type_alias_expansions)
                .map(|((front, identities), aliases)| front.into_committed(identities, aliases)),
        );

        Ok(ValidatedCrossConeSemanticClosure {
            current: self.current,
            target: self.target,
            direct: self.direct,
            dependency_first,
            positions: self.positions,
            dependency_positions: self.dependency_positions,
        })
    }
}

impl<'input> ValidatedCrossConeSemanticClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn provider_count(&self) -> usize {
        self.dependency_first.len()
    }

    pub fn direct_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<DirectCrossConeSemanticProvider<'_, 'input>> {
        self.direct
            .binary_search(&identity)
            .ok()
            .and_then(|_| self.provider(identity))
            .map(|artifact| DirectCrossConeSemanticProvider { artifact })
    }

    pub fn support_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<SupportCrossConeSemanticProvider<'_, 'input>> {
        self.direct
            .binary_search(&identity)
            .is_err()
            .then(|| self.provider(identity))
            .flatten()
            .map(|artifact| SupportCrossConeSemanticProvider { artifact })
    }

    pub fn direct_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = DirectCrossConeSemanticProvider<'_, 'input>> {
        self.direct.iter().map(|identity| {
            let artifact = self
                .provider(*identity)
                .expect("validated direct providers belong to the committed closure");
            DirectCrossConeSemanticProvider { artifact }
        })
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
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
    ) -> Option<&ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }
}

/// Enumeration-capable view of one validated direct dependency.
#[derive(Clone, Copy)]
pub struct DirectCrossConeSemanticProvider<'closure, 'input> {
    artifact: &'closure ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile>,
}

impl<'closure> DirectCrossConeSemanticProvider<'closure, '_> {
    pub const fn coordinate(self) -> &'closure ConeCoordinate {
        self.artifact.coordinate()
    }

    pub const fn identity(self) -> ConeIdentity {
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
pub struct SupportCrossConeSemanticProvider<'closure, 'input> {
    artifact: &'closure ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile>,
}

impl<'closure> SupportCrossConeSemanticProvider<'closure, '_> {
    pub const fn coordinate(self) -> &'closure ConeCoordinate {
        self.artifact.coordinate()
    }

    pub const fn identity(self) -> ConeIdentity {
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
