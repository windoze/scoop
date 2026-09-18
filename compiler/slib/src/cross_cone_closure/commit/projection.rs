//! Stage-specific projection of committed ordinary-dependency selections.

use std::fmt;

use scoop_hir::SelectedImportedDependencySet;
use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_lir::{SelectedDependencyLirCallableV1, SelectedDependencyLirSet};
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedDependencyMirSet};

use super::{ValidatedCrossConeSemanticClosure, world::provider_certificate};

impl ValidatedCrossConeSemanticClosure<'_> {
    /// Projects the exact HIR winners into the matching provider MIR exports.
    ///
    /// Constants have already been inlined into HIR and therefore do not
    /// produce MIR roots. Every callable is checked against both its retained
    /// artifact certificate and the terminal provider's canonical export.
    pub fn project_dependency_callables_to_mir(
        &self,
        selected: &SelectedImportedDependencySet,
    ) -> Result<SelectedDependencyMirSet, CrossConeMirSelectionProjectionError> {
        if selected.consumer() != self.current {
            return Err(CrossConeMirSelectionProjectionError::ConsumerMismatch {
                closure: self.current,
                selected: selected.consumer(),
            });
        }

        let mut projected = Vec::with_capacity(selected.callable_count());
        for callable in selected.callables() {
            let provider = callable.provider();
            let artifact = self
                .provider(provider)
                .ok_or(CrossConeMirSelectionProjectionError::MissingProvider { provider })?;
            if callable.certificate() != &provider_certificate(artifact) {
                return Err(
                    CrossConeMirSelectionProjectionError::ProviderCertificateMismatch { provider },
                );
            }

            let capability = callable.capability();
            let declaration = capability.declaration();
            let export = artifact
                .production()
                .mir_cross_cone()
                .export(declaration)
                .ok_or(CrossConeMirSelectionProjectionError::MissingExport {
                    provider,
                    declaration,
                })?;
            if export.implementation() != capability.implementation() {
                return Err(
                    CrossConeMirSelectionProjectionError::ImplementationMismatch {
                        provider,
                        declaration,
                    },
                );
            }
            if export.signature() != capability.signature() {
                return Err(CrossConeMirSelectionProjectionError::SignatureMismatch {
                    provider,
                    declaration,
                });
            }
            projected.push(
                SelectedDependencyMirCallableV1::try_new(
                    provider,
                    declaration,
                    export.implementation(),
                    export.signature().clone(),
                )
                .map_err(CrossConeMirSelectionProjectionError::Record)?,
            );
        }

        SelectedDependencyMirSet::try_from_callables(self.current, projected)
            .map_err(CrossConeMirSelectionProjectionError::Selection)
    }

    /// Projects one MIR-selected set into the canonical LIR exports retained
    /// by the same atomically validated closure.
    pub fn project_dependency_callables_to_lir(
        &self,
        selected: &SelectedDependencyMirSet,
    ) -> Result<SelectedDependencyLirSet, CrossConeLirSelectionProjectionError> {
        if selected.consumer() != self.current {
            return Err(CrossConeLirSelectionProjectionError::ConsumerMismatch {
                closure: self.current,
                selected: selected.consumer(),
            });
        }

        let mut projected = Vec::with_capacity(selected.len());
        for callable in selected.callables() {
            let provider = callable.provider();
            let declaration = callable.declaration();
            let artifact = self
                .provider(provider)
                .ok_or(CrossConeLirSelectionProjectionError::MissingProvider { provider })?;
            let export = artifact
                .production()
                .lir_cross_cone()
                .export(declaration)
                .ok_or(CrossConeLirSelectionProjectionError::MissingExport {
                    provider,
                    declaration,
                })?;
            if export.target() != callable.implementation()
                || export.abi_signature().signature() != callable.signature()
            {
                return Err(CrossConeLirSelectionProjectionError::BridgeMismatch {
                    provider,
                    declaration,
                });
            }
            let selected = SelectedDependencyLirCallableV1::new(
                provider,
                declaration,
                export.target(),
                export.abi_signature().clone(),
                export.calling_convention(),
                export.root_plan(),
            )
            .map_err(CrossConeLirSelectionProjectionError::Record)?;
            if selected.bridge() != export {
                return Err(CrossConeLirSelectionProjectionError::BridgeMismatch {
                    provider,
                    declaration,
                });
            }
            projected.push(selected);
        }

        SelectedDependencyLirSet::try_from_callables(self.current, projected)
            .map_err(CrossConeLirSelectionProjectionError::Selection)
    }
}

#[derive(Debug)]
pub enum CrossConeMirSelectionProjectionError {
    ConsumerMismatch {
        closure: ConeIdentity,
        selected: ConeIdentity,
    },
    MissingProvider {
        provider: ConeIdentity,
    },
    ProviderCertificateMismatch {
        provider: ConeIdentity,
    },
    MissingExport {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    ImplementationMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    SignatureMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Record(scoop_mir::ParamFreeMirCallableBuildError),
    Selection(scoop_mir::SelectedDependencyMirSetBuildError),
}

impl fmt::Display for CrossConeMirSelectionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsumerMismatch { closure, selected } => write!(
                formatter,
                "dependency HIR selection belongs to consumer {selected}, not closure {closure}"
            ),
            Self::MissingProvider { provider } => write!(
                formatter,
                "dependency HIR selection names provider {provider} outside the committed closure"
            ),
            Self::ProviderCertificateMismatch { provider } => write!(
                formatter,
                "dependency HIR selection carries a stale certificate for provider {provider}"
            ),
            Self::MissingExport {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} has no MIR export for selected callable {declaration:?}"
            ),
            Self::ImplementationMismatch {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} changed the implementation of selected callable {declaration:?} between HIR and MIR"
            ),
            Self::SignatureMismatch {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} changed the signature of selected callable {declaration:?} between HIR and MIR"
            ),
            Self::Record(source) => write!(
                formatter,
                "cannot construct a selected dependency MIR record: {source}"
            ),
            Self::Selection(source) => write!(
                formatter,
                "cannot seal the selected dependency MIR set: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeMirSelectionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(source) => Some(source),
            Self::Selection(source) => Some(source),
            Self::ConsumerMismatch { .. }
            | Self::MissingProvider { .. }
            | Self::ProviderCertificateMismatch { .. }
            | Self::MissingExport { .. }
            | Self::ImplementationMismatch { .. }
            | Self::SignatureMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLirSelectionProjectionError {
    ConsumerMismatch {
        closure: ConeIdentity,
        selected: ConeIdentity,
    },
    MissingProvider {
        provider: ConeIdentity,
    },
    MissingExport {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    BridgeMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Record(scoop_lir::ParamFreeLirCallableBuildError),
    Selection(scoop_lir::SelectedDependencyLirSetBuildError),
}

impl fmt::Display for CrossConeLirSelectionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsumerMismatch { closure, selected } => write!(
                formatter,
                "dependency MIR selection belongs to consumer {selected}, not closure {closure}"
            ),
            Self::MissingProvider { provider } => write!(
                formatter,
                "dependency MIR selection names provider {provider} outside the committed closure"
            ),
            Self::MissingExport {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} has no LIR export for selected callable {declaration:?}"
            ),
            Self::BridgeMismatch {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} changed the bridge contract of selected callable {declaration:?} between MIR and LIR"
            ),
            Self::Record(source) => write!(
                formatter,
                "cannot construct a selected dependency LIR record: {source}"
            ),
            Self::Selection(source) => write!(
                formatter,
                "cannot seal the selected dependency LIR set: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeLirSelectionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(source) => Some(source),
            Self::Selection(source) => Some(source),
            Self::ConsumerMismatch { .. }
            | Self::MissingProvider { .. }
            | Self::MissingExport { .. }
            | Self::BridgeMismatch { .. } => None,
        }
    }
}
