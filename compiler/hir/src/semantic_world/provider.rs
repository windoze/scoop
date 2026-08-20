use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentExportBindingId};

use super::witness::import_binding_routes;
use super::{ImportedEntityIndex, ImportedSemanticWorldBuildError};
use crate::{
    CanonicalTypeAliasExpansionsV1, CrossConeHirInterfaceSectionV1, ImportedHirFoundation,
};

mod views;
pub use views::*;

/// Shared declarations already read and checked at the artifact boundary.
pub struct ImportedProviderInput<'input> {
    pub foundation: &'input ImportedHirFoundation,
    pub interface: &'input CrossConeHirInterfaceSectionV1,
    pub alias_expansions: &'input CanonicalTypeAliasExpansionsV1,
}

pub(super) struct ImportedProvider<'input> {
    is_direct: bool,
    foundation: &'input ImportedHirFoundation,
    interface: &'input CrossConeHirInterfaceSectionV1,
    alias_expansions: &'input CanonicalTypeAliasExpansionsV1,
    public_bindings: Vec<ImportedPublicBinding<'input>>,
    binding_positions: BTreeMap<PersistentExportBindingId, usize>,
    nested_bindings: BTreeSet<PersistentExportBindingId>,
}

impl<'input> ImportedProvider<'input> {
    pub(super) fn new(input: ImportedProviderInput<'input>, is_direct: bool) -> Self {
        let ImportedProviderInput {
            foundation,
            interface,
            alias_expansions,
        } = input;
        let nested_bindings = interface
            .nominal_interfaces()
            .records()
            .iter()
            .flat_map(|record| record.nested_bindings().values().iter().copied())
            .collect();
        Self {
            is_direct,
            foundation,
            interface,
            alias_expansions,
            public_bindings: Vec::new(),
            binding_positions: BTreeMap::new(),
            nested_bindings,
        }
    }

    pub(super) fn build_public_bindings(
        &mut self,
        entities: &ImportedEntityIndex,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        let provider = self.identity();
        let mut bindings = Vec::with_capacity(self.interface.public_bindings().records().len());
        let mut positions = BTreeMap::new();
        for record in self.interface.public_bindings().records() {
            let binding = record.binding();
            let identity = self.foundation.identity(binding).ok_or(
                ImportedSemanticWorldBuildError::MissingBindingIdentity { provider, binding },
            )?;
            let key = self
                .foundation
                .semantic_world_export_binding_key(binding)
                .ok_or(ImportedSemanticWorldBuildError::MissingBindingKey { provider, binding })?;
            if key.exporter() != provider {
                return Err(ImportedSemanticWorldBuildError::BindingExporterMismatch {
                    provider,
                    binding,
                    actual: key.exporter(),
                });
            }
            let (target, conflict) = entities.import_target_with_conflict(key.target()).ok_or(
                ImportedSemanticWorldBuildError::MissingBindingTarget {
                    provider,
                    binding,
                    target: key.target(),
                },
            )?;
            positions.insert(binding, bindings.len());
            bindings.push(ImportedPublicBinding {
                provider,
                identity,
                key,
                target,
                conflict: conflict.clone(),
                source: record.source(),
                lookup_sources: import_binding_routes(provider, binding, record.source()).map_err(
                    |error| ImportedSemanticWorldBuildError::InvalidLookupRoute {
                        provider,
                        binding,
                        error: Box::new(error),
                    },
                )?,
            });
        }
        if let Some(binding) = self
            .nested_bindings
            .iter()
            .find(|binding| !positions.contains_key(binding))
        {
            return Err(ImportedSemanticWorldBuildError::MissingNestedBinding {
                provider,
                binding: *binding,
            });
        }
        self.public_bindings = bindings;
        self.binding_positions = positions;
        Ok(())
    }

    pub(super) const fn identity(&self) -> ConeIdentity {
        self.foundation.origin()
    }

    pub(super) const fn foundation(&self) -> &'input ImportedHirFoundation {
        self.foundation
    }

    pub(super) const fn interface(&self) -> &'input CrossConeHirInterfaceSectionV1 {
        self.interface
    }

    pub(super) const fn alias_expansions(&self) -> &'input CanonicalTypeAliasExpansionsV1 {
        self.alias_expansions
    }

    pub(super) const fn is_direct(&self) -> bool {
        self.is_direct
    }

    pub(super) fn is_package_binding(&self, binding: PersistentExportBindingId) -> bool {
        !self.nested_bindings.contains(&binding)
    }

    pub(super) fn public_bindings(&self) -> &[ImportedPublicBinding<'input>] {
        &self.public_bindings
    }

    pub(super) fn binding(&self, index: usize) -> Option<&ImportedPublicBinding<'input>> {
        self.public_bindings.get(index)
    }

    pub(super) fn binding_by_id(
        &self,
        binding: PersistentExportBindingId,
    ) -> Option<&ImportedPublicBinding<'input>> {
        self.binding_positions
            .get(&binding)
            .and_then(|index| self.public_bindings.get(*index))
    }
}
