//! Cloneable selection transactions for executable ordinary dependency uses.

use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, PersistentTypeAliasId,
};

use super::{DirectImportedTargetBinding, ImportedTarget};
use crate::DefaultCallableDeclarationV1;

mod catalog;
mod error;
mod intrinsics;
mod members;
mod model;
mod nominals;
mod properties;
mod routes;
pub use error::*;
pub use intrinsics::ImportedIntrinsicCallable;
pub use members::*;
pub use model::*;
pub use nominals::ImportedNominalDeclaration;

use catalog::DependencyCatalog;

/// A cloneable, lifetime-free transaction for dependency selections.
///
/// Lowering probes clone this value. Dropping a failed probe drops its
/// selections, while adopting the winning probe preserves both the selected
/// callable and the precise route witnesses used to reach it.
#[derive(Clone, Debug)]
pub struct ImportedDependencySelectionPlan {
    catalog: Arc<DependencyCatalog>,
    callables: BTreeMap<CallableTemplateOrigin, SelectedImportedDependencyCallable>,
    constants: BTreeMap<PersistentPropertyId, SelectedImportedDependencyConstant>,
    type_aliases: BTreeMap<PersistentTypeAliasId, SelectedImportedDependencyTypeAlias>,
}

impl ImportedDependencySelectionPlan {
    /// Looks up compiler semantics on the actual shared callable declarations.
    pub fn has_intrinsic_callable(
        &self,
        kind: crate::IntrinsicFunctionKind,
        gc_effect: scoop_identity::GcEffect,
    ) -> bool {
        self.catalog.callables.values().any(|entry| {
            let effects = entry.interface.effects();
            effects.implementation() == crate::CallableImplementationV1::Intrinsic(kind)
                && effects.gc_effect() == gc_effect
        })
    }

    /// Starts an empty selection set for a request without imported declarations.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            catalog: Arc::new(DependencyCatalog {
                nominals: BTreeMap::new(),
                direct_binding_witnesses: Arc::new(BTreeMap::new()),
                consumer,
                callables: BTreeMap::new(),
                properties: BTreeMap::new(),
                constants: BTreeMap::new(),
                type_aliases: BTreeMap::new(),
                direct_callable_bindings: BTreeMap::new(),
            }),
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
        let entry = self.catalog.callables.get(&declaration).ok_or(
            ImportedDependencyCandidateError::MissingCallable(declaration),
        )?;
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.provider
        }) {
            return Err(ImportedDependencyCandidateError::TerminalProviderMismatch {
                declaration,
                expected: entry.provider,
            });
        }
        Ok(ImportedDependencyCallableCandidate {
            binding: binding.clone(),
            provider: entry.provider,
            interface: entry.interface.clone(),
            source: entry.source.clone(),
            capability: entry.capability.clone(),
            default_templates: entry.default_templates.clone(),
            definition_sources: Arc::clone(&entry.definition_sources),
        })
    }

    /// Resolves a callable referenced by a validated dependency default
    /// template through the direct dependency surface. The template reference
    /// identifies the definition-side target; this lookup retains the
    /// consumer-side import path in the selected set.
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
        let entry = self
            .catalog
            .constants
            .get(&property)
            .ok_or(ImportedDependencyCandidateError::MissingConstant(property))?;
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.provider
        }) {
            return Err(
                ImportedDependencyCandidateError::ConstantTerminalProviderMismatch {
                    property,
                    expected: entry.provider,
                },
            );
        }
        Ok(ImportedDependencyConstantCandidate {
            binding: binding.clone(),
            provider: entry.provider,
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
        let entry = self
            .catalog
            .type_aliases
            .get(&alias)
            .ok_or(ImportedDependencyCandidateError::MissingTypeAlias(alias))?;
        if binding.sources().any(|source| {
            source.witness().terminal_declaration() != binding.target()
                || source.witness().route().terminal().exporter() != entry.provider
        }) {
            return Err(
                ImportedDependencyCandidateError::TypeAliasTerminalProviderMismatch {
                    alias,
                    expected: entry.provider,
                },
            );
        }
        Ok(ImportedDependencyTypeAliasCandidate {
            binding: binding.clone(),
            provider: entry.provider,
            interface: entry.interface.clone(),
            expansion: entry.expansion.clone(),
        })
    }

    pub fn select_callable(
        &mut self,
        candidate: ImportedDependencyCallableCandidate,
    ) -> Result<ImportedDependencyCallableRef, ImportedDependencySelectionError> {
        self.select_callable_declaration(candidate.interface.declaration())
    }

    pub fn select_member_callable(
        &mut self,
        candidate: ImportedMemberCallableCandidate,
    ) -> Result<ImportedDependencyCallableRef, ImportedDependencySelectionError> {
        use crate::ImportedCallableSource;
        self.select_callable_declaration(candidate.interface().declaration())
    }

    fn select_callable_declaration(
        &mut self,
        id: CallableTemplateOrigin,
    ) -> Result<ImportedDependencyCallableRef, ImportedDependencySelectionError> {
        let entry = self
            .catalog
            .callables
            .get(&id)
            .ok_or(ImportedDependencySelectionError::MissingCallable(id))?;
        let Some(capability) = entry.capability.clone() else {
            return Err(ImportedDependencySelectionError::CapabilityUnavailable {
                declaration: id,
            });
        };
        self.callables
            .entry(id)
            .or_insert_with(|| SelectedImportedDependencyCallable {
                provider: entry.provider,
                interface: entry.interface.clone(),
                source: entry.source.clone(),
                initialization_unit: entry.initialization_unit,
                capability,
            });
        Ok(ImportedDependencyCallableRef { callable: id })
    }

    pub fn select_constant(
        &mut self,
        candidate: ImportedDependencyConstantCandidate,
    ) -> Result<ImportedDependencyConstantRef, ImportedDependencySelectionError> {
        let id = candidate.record.property();
        let entry = self
            .catalog
            .constants
            .get(&id)
            .ok_or(ImportedDependencySelectionError::MissingConstant(id))?;
        let Some(exact_type) = entry.exact_type else {
            return Err(
                ImportedDependencySelectionError::ConstantCapabilityUnavailable {
                    target: candidate.target(),
                },
            );
        };
        if let Some(selected) = self.constants.get_mut(&id) {
            selected
                .binding
                .try_merge(candidate.binding)
                .map_err(ImportedDependencySelectionError::RouteMerge)?;
        } else {
            self.constants.insert(
                id,
                SelectedImportedDependencyConstant {
                    binding: candidate.binding,
                    provider: entry.provider,
                    record: entry.record.clone(),
                    exact_type,
                },
            );
        }
        Ok(ImportedDependencyConstantRef { constant: id })
    }

    pub fn select_type_alias(
        &mut self,
        candidate: ImportedDependencyTypeAliasCandidate,
    ) -> Result<ImportedDependencyTypeAliasRef, ImportedDependencySelectionError> {
        let id = candidate.interface.alias();
        let entry = self
            .catalog
            .type_aliases
            .get(&id)
            .ok_or(ImportedDependencySelectionError::MissingTypeAlias(id))?;
        if let Some(selected) = self.type_aliases.get_mut(&id) {
            selected
                .binding
                .try_merge(candidate.binding)
                .map_err(ImportedDependencySelectionError::RouteMerge)?;
        } else {
            self.type_aliases.insert(
                id,
                SelectedImportedDependencyTypeAlias {
                    binding: candidate.binding,
                    provider: entry.provider,
                    interface: entry.interface.clone(),
                    expansion: entry.expansion.clone(),
                },
            );
        }
        Ok(ImportedDependencyTypeAliasRef { alias: id })
    }

    /// Resolves a reference already committed in this transaction.
    ///
    /// Lowering-side semantic checks consume the selected capability before
    /// the transaction is sealed into its output-owned selected set.
    pub fn resolve_callable(
        &self,
        reference: ImportedDependencyCallableRef,
    ) -> Option<&SelectedImportedDependencyCallable> {
        self.callables.get(&reference.callable)
    }

    pub fn resolve_type_alias(
        &self,
        reference: ImportedDependencyTypeAliasRef,
    ) -> Option<&SelectedImportedDependencyTypeAlias> {
        self.type_aliases.get(&reference.alias)
    }

    pub fn finish(self) -> SelectedImportedDependencySet {
        SelectedImportedDependencySet {
            direct_binding_witnesses: self.catalog.direct_binding_witnesses.clone(),
            consumer: self.catalog.consumer,
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
