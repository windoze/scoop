//! Cloneable selection transactions for ordinary dependency callables.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{CallableTemplateOrigin, ConeIdentity};

use super::{
    DirectImportedTargetBinding, ImportedProviderCertificate, ImportedSemanticWorld, ImportedTarget,
};
use crate::{
    CallableInterfaceRecordV1, CallableSourceInterfaceV1, CoreClosedExactLeafClassifierV1,
    ParamFreeCoreClosedCallableV1,
};

mod error;
mod model;
pub use error::*;
pub use model::*;

static NEXT_PROJECTION: AtomicU64 = AtomicU64::new(1);
static NEXT_SELECTION: AtomicU64 = AtomicU64::new(1);

fn next_id(counter: &AtomicU64, domain: &'static str) -> u64 {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .unwrap_or_else(|_| panic!("the {domain} id space is exhausted"))
}

#[derive(Clone, Debug)]
struct CallableCatalogEntry {
    certificate: ImportedProviderCertificate,
    interface: CallableInterfaceRecordV1,
    source: Option<CallableSourceInterfaceV1>,
    capability: Option<ParamFreeCoreClosedCallableV1>,
}

#[derive(Debug)]
struct DependencyCatalog {
    world_brand: u64,
    consumer: ConeIdentity,
    projection: DependencyProjectionId,
    callables: BTreeMap<CallableTemplateOrigin, CallableCatalogEntry>,
    callable_ids: BTreeMap<CallableTemplateOrigin, ImportedDependencyCallableId>,
}

/// A cloneable, lifetime-free transaction for dependency selections.
///
/// Lowering probes clone this value. Dropping a failed probe drops its
/// selections, while adopting the winning probe preserves both the selected
/// callable and the precise route witnesses used to reach it.
#[derive(Clone, Debug)]
pub struct ImportedDependencySelectionPlan {
    catalog: Arc<DependencyCatalog>,
    selection: DependencySelectionId,
    callables: BTreeMap<ImportedDependencyCallableId, SelectedImportedDependencyCallable>,
}

impl ImportedDependencySelectionPlan {
    /// Starts an empty transaction for an ordinary core-only request.
    /// No callable candidate can be minted from this plan.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            catalog: Arc::new(DependencyCatalog {
                world_brand: 0,
                consumer,
                projection: DependencyProjectionId(next_id(
                    &NEXT_PROJECTION,
                    "dependency projection",
                )),
                callables: BTreeMap::new(),
                callable_ids: BTreeMap::new(),
            }),
            selection: DependencySelectionId(next_id(&NEXT_SELECTION, "dependency selection")),
            callables: BTreeMap::new(),
        }
    }

    pub fn callable_candidate(
        &self,
        binding: &DirectImportedTargetBinding,
    ) -> Result<ImportedDependencyCallableCandidate, ImportedDependencyCandidateError> {
        let declaration = match binding.target() {
            ImportedTarget::Function(id) => CallableTemplateOrigin::Function(id.persistent()),
            ImportedTarget::GenericFunction(id) => {
                CallableTemplateOrigin::GenericFunction(id.persistent())
            }
            target => return Err(ImportedDependencyCandidateError::NotCallable(target)),
        };
        if binding
            .sources()
            .any(|source| source.immediate_provider().brand() != self.catalog.world_brand)
        {
            return Err(ImportedDependencyCandidateError::ForeignWorld);
        }
        let entry = self.catalog.callables.get(&declaration).ok_or(
            ImportedDependencyCandidateError::MissingCallable(declaration),
        )?;
        let callable = *self
            .catalog
            .callable_ids
            .get(&declaration)
            .expect("every dependency callable snapshot has one stable id");
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.certificate.identity()
        }) {
            return Err(ImportedDependencyCandidateError::TerminalProviderMismatch {
                declaration,
                expected: entry.certificate.identity(),
            });
        }
        Ok(ImportedDependencyCallableCandidate {
            projection: self.catalog.projection,
            callable,
            binding: binding.clone(),
            certificate: entry.certificate.clone(),
            interface: entry.interface.clone(),
            source: entry.source.clone(),
            capability: entry.capability.clone(),
        })
    }

    pub fn select_callable(
        &mut self,
        candidate: ImportedDependencyCallableCandidate,
    ) -> Result<ImportedDependencyCallableRef, ImportedDependencySelectionError> {
        if candidate.projection != self.catalog.projection {
            return Err(ImportedDependencySelectionError::ForeignProjection);
        }
        let Some(capability) = candidate.capability.clone() else {
            return Err(ImportedDependencySelectionError::CapabilityUnavailable {
                target: candidate.target(),
            });
        };
        let id = candidate.callable;
        if let Some(selected) = self.callables.get_mut(&id) {
            if selected.provider() != candidate.provider()
                || selected.capability.declaration() != capability.declaration()
            {
                return Err(ImportedDependencySelectionError::ForeignProjection);
            }
            selected
                .binding
                .try_merge(candidate.binding)
                .map_err(ImportedDependencySelectionError::RouteMerge)?;
        } else {
            self.callables.insert(
                id,
                SelectedImportedDependencyCallable {
                    binding: candidate.binding,
                    certificate: candidate.certificate,
                    interface: candidate.interface,
                    source: candidate.source,
                    capability,
                },
            );
        }
        Ok(ImportedDependencyCallableRef {
            selection: self.selection,
            callable: id,
        })
    }

    /// Resolves a reference already committed in this transaction.
    ///
    /// Lowering-side semantic checks consume the selected capability before
    /// the transaction is sealed into its output-owned selected set.
    pub fn resolve_callable(
        &self,
        reference: ImportedDependencyCallableRef,
    ) -> Option<&SelectedImportedDependencyCallable> {
        (reference.selection == self.selection)
            .then(|| self.callables.get(&reference.callable))
            .flatten()
    }

    pub fn finish(self) -> SelectedImportedDependencySet {
        SelectedImportedDependencySet {
            consumer: self.catalog.consumer,
            selection: self.selection,
            callables: self.callables,
        }
    }

    pub fn selected_callable_count(&self) -> usize {
        self.callables.len()
    }
}

impl ImportedSemanticWorld<'_> {
    pub fn dependency_selection_plan(
        &self,
        classifier: &CoreClosedExactLeafClassifierV1,
    ) -> Result<ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError> {
        let projection = DependencyProjectionId(next_id(&NEXT_PROJECTION, "dependency projection"));
        let mut callables = BTreeMap::new();
        for provider in &self.providers {
            if provider.identity() == ConeIdentity::CORE {
                continue;
            }
            for callable in provider.interface().callable_interfaces().records() {
                let declaration = callable.declaration();
                let entry = CallableCatalogEntry {
                    certificate: provider.certificate().clone(),
                    interface: callable.clone(),
                    source: provider
                        .interface()
                        .source_interfaces()
                        .get(declaration)
                        .cloned(),
                    capability: classifier
                        .classify_callable(callable)
                        .map_err(ImportedDependencySelectionPlanBuildError::Classification)?,
                };
                if callables.insert(declaration, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateCallable(declaration),
                    );
                }
            }
        }
        let callable_ids = callables
            .keys()
            .copied()
            .enumerate()
            .map(|(index, declaration)| {
                u32::try_from(index)
                    .map(|index| (declaration, ImportedDependencyCallableId(index)))
                    .map_err(
                        |_| ImportedDependencySelectionPlanBuildError::TooManyCallables {
                            count: callables.len(),
                        },
                    )
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(ImportedDependencySelectionPlan {
            catalog: Arc::new(DependencyCatalog {
                world_brand: self.brand,
                consumer: self.current,
                projection,
                callables,
                callable_ids,
            }),
            selection: DependencySelectionId(next_id(&NEXT_SELECTION, "dependency selection")),
            callables: BTreeMap::new(),
        })
    }
}
