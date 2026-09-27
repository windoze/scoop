use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentEnumVariantId, PersistentExportBindingId,
    PersistentId, PersistentObjectValueId, PersistentPropertyId, PersistentTypeAliasId,
    PropertyOwner,
};

use super::super::{import_callable_id, import_nominal_id, import_property_id};
use super::ImportedProvider;
use crate::DependencyBindingWitnessV1;
use crate::{
    CallableSourceInterfaceV1, ExportBindingSourceV1, ExportConstValueV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ImportedBindingConflictKey,
    ImportedCallable, ImportedEnumVariant, ImportedHirId, ImportedNominal, ImportedObjectValue,
    ImportedProperty, ImportedTarget, ImportedTypeAlias, NominalSourceShapeV1, SourceNominalId,
};

/// One public binding with its typed declaration and actual provider.
/// The source records the declaration or re-export path in the artifact.
#[derive(Clone)]
pub struct ImportedPublicBinding<'input> {
    pub(super) provider: ConeIdentity,
    pub(super) identity: ImportedHirId<PersistentExportBindingId>,
    pub(super) key: &'input scoop_identity::ExportBindingKey,
    pub(super) target: ImportedTarget,
    pub(super) conflict: ImportedBindingConflictKey,
    pub(super) source: &'input ExportBindingSourceV1,
    pub(super) lookup_sources: Vec<DependencyBindingWitnessV1>,
}

impl<'input> ImportedPublicBinding<'input> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn identity(&self) -> ImportedHirId<PersistentExportBindingId> {
        self.identity
    }

    pub const fn key(&self) -> &'input scoop_identity::ExportBindingKey {
        self.key
    }

    pub const fn target(&self) -> ImportedTarget {
        self.target
    }

    pub const fn conflict_key(&self) -> &ImportedBindingConflictKey {
        &self.conflict
    }

    pub const fn source(&self) -> &'input ExportBindingSourceV1 {
        self.source
    }

    pub fn lookup_sources(&self) -> &[DependencyBindingWitnessV1] {
        &self.lookup_sources
    }
}

/// Shared name and typed declaration queries for one dependency.
#[derive(Clone, Copy)]
pub struct ImportedProviderView<'world, 'input> {
    pub(in crate::semantic_world) provider: &'world ImportedProvider<'input>,
}

impl<'world, 'input> ImportedProviderView<'world, 'input> {
    pub const fn id(self) -> ConeIdentity {
        self.provider.identity()
    }

    pub fn hir_identity<I: PersistentId + 'static>(self, id: I) -> Option<ImportedHirId<I>> {
        self.provider.foundation().identity(id)
    }

    pub fn nominal(self, declaration: SourceNominalId) -> Option<ImportedNominal<'input>> {
        let record = self
            .provider
            .interface()
            .nominal_interfaces()
            .get(declaration)?;
        Some(ImportedNominal {
            provider: self.id(),
            declaration: import_nominal_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn callable(self, declaration: CallableTemplateOrigin) -> Option<ImportedCallable<'input>> {
        let record = self
            .provider
            .interface()
            .callable_interfaces()
            .get(declaration)?;
        Some(ImportedCallable {
            provider: self.id(),
            declaration: import_callable_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn property(self, declaration: PropertyOwner) -> Option<ImportedProperty<'input>> {
        let record = self
            .provider
            .interface()
            .property_interfaces()
            .get(declaration)?;
        Some(ImportedProperty {
            provider: self.id(),
            declaration: import_property_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn type_alias(self, alias: PersistentTypeAliasId) -> Option<ImportedTypeAlias<'input>> {
        Some(ImportedTypeAlias {
            provider: self.id(),
            identity: self.provider.foundation().identity(alias)?,
            record: self.provider.interface().type_aliases().get(alias)?,
            expansion: self.provider.alias_expansions().get(alias)?,
        })
    }

    pub fn object_value(
        self,
        value: PersistentObjectValueId,
    ) -> Option<ImportedObjectValue<'input>> {
        self.provider
            .interface()
            .nominal_interfaces()
            .records()
            .iter()
            .find_map(|record| {
                let NominalSourceShapeV1::Object(shape) = record.source_shape() else {
                    return None;
                };
                if shape.value() != value {
                    return None;
                }
                Some(ImportedObjectValue {
                    provider: self.id(),
                    identity: self.provider.foundation().identity(value)?,
                    owner: import_nominal_id(self.provider, record.declaration())?,
                    owner_record: record,
                })
            })
    }

    pub fn enum_variant(
        self,
        variant: PersistentEnumVariantId,
    ) -> Option<ImportedEnumVariant<'input>> {
        self.provider
            .interface()
            .nominal_interfaces()
            .records()
            .iter()
            .find_map(|owner_record| {
                let NominalSourceShapeV1::Enum(shape) = owner_record.source_shape() else {
                    return None;
                };
                let record = shape
                    .variants()
                    .iter()
                    .find(|record| record.variant() == variant)?;
                Some(ImportedEnumVariant {
                    provider: self.id(),
                    identity: self.provider.foundation().identity(variant)?,
                    owner: import_nominal_id(self.provider, owner_record.declaration())?,
                    owner_record,
                    record,
                })
            })
    }

    pub fn callable_source(
        self,
        declaration: CallableTemplateOrigin,
    ) -> Option<&'input CallableSourceInterfaceV1> {
        self.provider
            .interface()
            .source_interfaces()
            .get(declaration)
    }

    pub fn default_template(
        self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&'input ExportDefaultTemplateV1> {
        self.provider.interface().default_templates().get(key)
    }

    pub fn constant(self, property: PersistentPropertyId) -> Option<&'input ExportConstValueV1> {
        self.provider.interface().constants().get(property)
    }
    pub fn nominal_interfaces(self) -> &'input crate::CanonicalNominalInterfacesV1 {
        self.provider.interface().nominal_interfaces()
    }

    pub fn public_bindings(self) -> &'world [ImportedPublicBinding<'input>] {
        self.provider.public_bindings()
    }
}
