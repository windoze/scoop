//! Read-only imported semantic world used by ordinary cross-Cone HIR lookup.
//!
//! The direct dependency set determines the source package namespace. All
//! providers share the same declaration storage and typed lookup paths.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;

use crate::SourceNominalId;

mod entities;
mod error;
mod namespace;
mod native_boundary;
mod nominal_materialization;
mod nominal_signatures;
mod production_authority;
mod provider;
mod selection;
mod witness;

pub use entities::*;
pub use error::*;
pub use namespace::*;
pub use production_authority::*;
pub use provider::*;
pub use selection::*;
pub use witness::*;

use entities::ImportedEntityIndex;
use namespace::build_direct_package_index;
use provider::ImportedProvider;

/// Immutable semantic projection of one validated dependency closure.
pub struct ImportedSemanticWorld<'input> {
    current: ConeIdentity,
    providers: Vec<ImportedProvider<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    direct: Vec<ConeIdentity>,
    entities: ImportedEntityIndex,
    direct_packages: DirectPackageIndex,
}

impl<'input> ImportedSemanticWorld<'input> {
    /// Builds name and declaration indexes from the artifact reader's complete
    /// dependency data. Direct inputs contribute to the source package index.
    pub fn from_dependencies(
        current: ConeIdentity,
        direct: Vec<ImportedProviderInput<'input>>,
        support: Vec<ImportedProviderInput<'input>>,
    ) -> Result<Self, ImportedSemanticWorldBuildError> {
        let mut providers: Vec<_> = direct
            .into_iter()
            .map(|input| ImportedProvider::new(input, true))
            .chain(
                support
                    .into_iter()
                    .map(|input| ImportedProvider::new(input, false)),
            )
            .collect();
        providers.sort_unstable_by_key(ImportedProvider::identity);
        let mut positions = BTreeMap::new();
        let mut direct_ids = Vec::new();
        for (index, provider) in providers.iter().enumerate() {
            let identity = provider.identity();
            if identity == current {
                return Err(ImportedSemanticWorldBuildError::CurrentUsedAsProvider(
                    current,
                ));
            }
            if positions.insert(identity, index).is_some() {
                return Err(ImportedSemanticWorldBuildError::DuplicateProvider(identity));
            }
            if provider.is_direct() {
                direct_ids.push(identity);
            }
        }
        let entities = ImportedEntityIndex::build(&providers)?;
        for provider in &mut providers {
            provider.build_public_bindings(&entities)?;
        }
        let direct_packages = build_direct_package_index(&providers);

        Ok(Self {
            current,
            providers,
            positions,
            direct: direct_ids,
            entities,
            direct_packages,
        })
    }

    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn direct_provider_count(&self) -> usize {
        self.direct.len()
    }

    pub fn support_provider_count(&self) -> usize {
        self.providers.len() - self.direct.len()
    }

    pub fn direct_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<ImportedProviderView<'_, 'input>> {
        let provider = self.provider(identity)?;
        provider
            .is_direct()
            .then_some(ImportedProviderView { provider })
    }

    pub fn direct_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = ImportedProviderView<'_, 'input>> {
        self.direct.iter().map(|id| ImportedProviderView {
            provider: &self.providers[self.positions[id]],
        })
    }

    pub const fn direct_packages(&self) -> &DirectPackageIndex {
        &self.direct_packages
    }

    pub fn nominal(&self, declaration: SourceNominalId) -> Option<ImportedNominal<'input>> {
        let provider = self
            .entities
            .nominal_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.nominal(declaration)
    }

    pub fn callable(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Option<ImportedCallable<'input>> {
        let provider = self
            .entities
            .callable_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.callable(declaration)
    }

    pub fn property(
        &self,
        declaration: scoop_identity::PropertyOwner,
    ) -> Option<ImportedProperty<'input>> {
        let provider = self
            .entities
            .property_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.property(declaration)
    }

    pub fn type_alias(
        &self,
        alias: scoop_identity::PersistentTypeAliasId,
    ) -> Option<ImportedTypeAlias<'input>> {
        let provider = self
            .entities
            .alias_provider(alias)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.type_alias(alias)
    }

    pub fn object_value(
        &self,
        value: scoop_identity::PersistentObjectValueId,
    ) -> Option<ImportedObjectValue<'input>> {
        let provider = self
            .entities
            .object_value_provider(value)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.object_value(value)
    }

    pub fn enum_variant(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Option<ImportedEnumVariant<'input>> {
        let provider = self
            .entities
            .enum_variant_provider(variant)
            .and_then(|id| self.provider(id))?;
        ImportedProviderView { provider }.enum_variant(variant)
    }

    fn provider(&self, identity: ConeIdentity) -> Option<&ImportedProvider<'input>> {
        self.positions
            .get(&identity)
            .map(|position| &self.providers[*position])
    }
}

#[cfg(test)]
mod tests;
