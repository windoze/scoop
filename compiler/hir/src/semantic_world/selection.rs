//! Cloneable selection transactions for executable ordinary dependency uses.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{CallableTemplateOrigin, ConeIdentity};

use super::{DirectImportedTargetBinding, ImportedTarget};
use crate::DefaultCallableDeclarationV1;

mod catalog;
mod error;
mod model;
mod properties;
mod routes;
pub use error::*;
pub use model::*;

use catalog::DependencyCatalog;

static NEXT_PROJECTION: AtomicU64 = AtomicU64::new(1);
static NEXT_SELECTION: AtomicU64 = AtomicU64::new(1);

fn next_id(counter: &AtomicU64, domain: &'static str) -> u64 {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .unwrap_or_else(|_| panic!("the {domain} id space is exhausted"))
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
    constants: BTreeMap<ImportedDependencyConstantId, SelectedImportedDependencyConstant>,
    type_aliases: BTreeMap<ImportedDependencyTypeAliasId, SelectedImportedDependencyTypeAlias>,
}

impl ImportedDependencySelectionPlan {
    /// Starts an empty transaction for an ordinary core-only request.
    /// No callable candidate can be minted from this plan.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            catalog: Arc::new(DependencyCatalog {
                direct_binding_witnesses: Arc::new(BTreeMap::new()),
                world_brand: 0,
                consumer,
                projection: DependencyProjectionId(next_id(
                    &NEXT_PROJECTION,
                    "dependency projection",
                )),
                callables: BTreeMap::new(),
                callable_ids: BTreeMap::new(),
                properties: BTreeMap::new(),
                constants: BTreeMap::new(),
                constant_ids: BTreeMap::new(),
                type_aliases: BTreeMap::new(),
                type_alias_ids: BTreeMap::new(),
                direct_callable_bindings: BTreeMap::new(),
            }),
            selection: DependencySelectionId(next_id(&NEXT_SELECTION, "dependency selection")),
            callables: BTreeMap::new(),
            constants: BTreeMap::new(),
            type_aliases: BTreeMap::new(),
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
        self.callable_candidate_for_declaration(declaration, binding)
    }

    fn callable_candidate_for_declaration(
        &self,
        declaration: CallableTemplateOrigin,
        binding: &DirectImportedTargetBinding,
    ) -> Result<ImportedDependencyCallableCandidate, ImportedDependencyCandidateError> {
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
            default_templates: entry.default_templates.clone(),
            definition_sources: Arc::clone(&entry.definition_sources),
        })
    }

    /// Resolves a callable referenced by a validated dependency default
    /// template through the direct dependency surface. The template reference
    /// itself supplies definition-side access authority; this lookup supplies
    /// the consumer-side route witness retained by the selected set.
    pub fn default_callable_candidate(
        &self,
        declaration: DefaultCallableDeclarationV1,
    ) -> Result<ImportedDependencyCallableCandidate, ImportedDependencyCandidateError> {
        let declaration = match declaration {
            DefaultCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
            DefaultCallableDeclarationV1::GenericFunction(id) => {
                CallableTemplateOrigin::GenericFunction(id)
            }
            DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                CallableTemplateOrigin::Accessor(id)
            }
            DefaultCallableDeclarationV1::Generated(_) => {
                return Err(ImportedDependencyCandidateError::GeneratedDefaultCallable);
            }
        };
        let binding = self
            .catalog
            .direct_callable_bindings
            .get(&declaration)
            .ok_or(ImportedDependencyCandidateError::MissingDefaultCallableBinding(declaration))?;
        self.callable_candidate_for_declaration(declaration, binding)
    }

    pub fn constant_candidate(
        &self,
        binding: &DirectImportedTargetBinding,
    ) -> Result<ImportedDependencyConstantCandidate, ImportedDependencyCandidateError> {
        let property = match binding.target() {
            ImportedTarget::Property(id) => id.persistent(),
            target => return Err(ImportedDependencyCandidateError::NotConstant(target)),
        };
        if binding
            .sources()
            .any(|source| source.immediate_provider().brand() != self.catalog.world_brand)
        {
            return Err(ImportedDependencyCandidateError::ForeignWorld);
        }
        let entry = self
            .catalog
            .constants
            .get(&property)
            .ok_or(ImportedDependencyCandidateError::MissingConstant(property))?;
        let constant = *self
            .catalog
            .constant_ids
            .get(&property)
            .expect("every dependency constant snapshot has one stable id");
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.certificate.identity()
        }) {
            return Err(
                ImportedDependencyCandidateError::ConstantTerminalProviderMismatch {
                    property,
                    expected: entry.certificate.identity(),
                },
            );
        }
        Ok(ImportedDependencyConstantCandidate {
            projection: self.catalog.projection,
            constant,
            binding: binding.clone(),
            certificate: entry.certificate.clone(),
            record: entry.record.clone(),
            exact_type: entry.exact_type,
            definition_sources: Arc::clone(&entry.definition_sources),
        })
    }

    pub fn type_alias_candidate(
        &self,
        binding: &DirectImportedTargetBinding,
    ) -> Result<ImportedDependencyTypeAliasCandidate, ImportedDependencyCandidateError> {
        let alias = match binding.target() {
            ImportedTarget::TypeAlias(id) => id.persistent(),
            target => return Err(ImportedDependencyCandidateError::NotTypeAlias(target)),
        };
        if binding
            .sources()
            .any(|source| source.immediate_provider().brand() != self.catalog.world_brand)
        {
            return Err(ImportedDependencyCandidateError::ForeignWorld);
        }
        let entry = self
            .catalog
            .type_aliases
            .get(&alias)
            .ok_or(ImportedDependencyCandidateError::MissingTypeAlias(alias))?;
        let selected_id = *self
            .catalog
            .type_alias_ids
            .get(&alias)
            .expect("every dependency type-alias snapshot has one stable id");
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.certificate.identity()
        }) {
            return Err(
                ImportedDependencyCandidateError::TypeAliasTerminalProviderMismatch {
                    alias,
                    expected: entry.certificate.identity(),
                },
            );
        }
        Ok(ImportedDependencyTypeAliasCandidate {
            projection: self.catalog.projection,
            alias: selected_id,
            binding: binding.clone(),
            certificate: entry.certificate.clone(),
            interface: entry.interface.clone(),
            expansion: entry.expansion.clone(),
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

    pub fn select_constant(
        &mut self,
        candidate: ImportedDependencyConstantCandidate,
    ) -> Result<ImportedDependencyConstantRef, ImportedDependencySelectionError> {
        if candidate.projection != self.catalog.projection {
            return Err(ImportedDependencySelectionError::ForeignProjection);
        }
        let Some(exact_type) = candidate.exact_type else {
            return Err(
                ImportedDependencySelectionError::ConstantCapabilityUnavailable {
                    target: candidate.target(),
                },
            );
        };
        let id = candidate.constant;
        if let Some(selected) = self.constants.get_mut(&id) {
            if selected.provider() != candidate.provider()
                || selected.record.property() != candidate.record.property()
                || selected.exact_type != exact_type
            {
                return Err(ImportedDependencySelectionError::ForeignProjection);
            }
            selected
                .binding
                .try_merge(candidate.binding)
                .map_err(ImportedDependencySelectionError::RouteMerge)?;
        } else {
            self.constants.insert(
                id,
                SelectedImportedDependencyConstant {
                    binding: candidate.binding,
                    certificate: candidate.certificate,
                    record: candidate.record,
                    exact_type,
                },
            );
        }
        Ok(ImportedDependencyConstantRef {
            selection: self.selection,
            constant: id,
        })
    }

    pub fn select_type_alias(
        &mut self,
        candidate: ImportedDependencyTypeAliasCandidate,
    ) -> Result<ImportedDependencyTypeAliasRef, ImportedDependencySelectionError> {
        if candidate.projection != self.catalog.projection {
            return Err(ImportedDependencySelectionError::ForeignProjection);
        }
        let id = candidate.alias;
        if let Some(selected) = self.type_aliases.get_mut(&id) {
            if selected.provider() != candidate.provider()
                || selected.interface.alias() != candidate.interface.alias()
                || selected.expansion != candidate.expansion
            {
                return Err(ImportedDependencySelectionError::ForeignProjection);
            }
            selected
                .binding
                .try_merge(candidate.binding)
                .map_err(ImportedDependencySelectionError::RouteMerge)?;
        } else {
            self.type_aliases.insert(
                id,
                SelectedImportedDependencyTypeAlias {
                    binding: candidate.binding,
                    certificate: candidate.certificate,
                    interface: candidate.interface,
                    expansion: candidate.expansion,
                },
            );
        }
        Ok(ImportedDependencyTypeAliasRef {
            selection: self.selection,
            alias: id,
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

    pub fn resolve_type_alias(
        &self,
        reference: ImportedDependencyTypeAliasRef,
    ) -> Option<&SelectedImportedDependencyTypeAlias> {
        (reference.selection == self.selection)
            .then(|| self.type_aliases.get(&reference.alias))
            .flatten()
    }

    pub fn finish(self) -> SelectedImportedDependencySet {
        SelectedImportedDependencySet {
            direct_binding_witnesses: self.catalog.direct_binding_witnesses.clone(),
            consumer: self.catalog.consumer,
            selection: self.selection,
            callables: self.callables,
            constants: self.constants,
            type_aliases: self.type_aliases,
        }
    }

    pub fn selected_callable_count(&self) -> usize {
        self.callables.len()
    }

    pub fn selected_constant_count(&self) -> usize {
        self.constants.len()
    }

    pub fn selected_type_alias_count(&self) -> usize {
        self.type_aliases.len()
    }
}
