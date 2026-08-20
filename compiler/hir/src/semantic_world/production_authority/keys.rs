//! Exact canonical-key lookup across the current and imported HIR foundations.

use scoop_identity::{
    EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey, GeneratedCallableKey,
    InitializationUnitKey, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentInitializationUnitId, PersistentObjectValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    PropertyAccessorKey, SourceDeclarationKey,
};

use super::CrossConeHirProductionAuthority;
use crate::CanonicalHirFoundation;

impl CrossConeHirProductionAuthority<'_, '_> {
    fn foundations(&self) -> impl Iterator<Item = &CanonicalHirFoundation> {
        std::iter::once(self.current_foundation).chain(
            self.world
                .providers
                .iter()
                .map(|provider| provider.foundation().canonical_for_semantic_authority()),
        )
    }

    pub(super) fn generated_callable_count(&self) -> usize {
        self.foundations()
            .map(|foundation| foundation.counts().generated_callables)
            .fold(0_usize, usize::saturating_add)
    }

    pub(super) fn generated_type_key(
        &self,
        id: PersistentTypeId,
    ) -> Option<&scoop_identity::GeneratedNominalKey> {
        self.foundations().find_map(|foundation| {
            let records = foundation.type_source_generated_records();
            records
                .binary_search_by_key(&id, |record| record.id())
                .ok()
                .map(|index| records[index].key())
        })
    }

    pub(super) fn source_type_key(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .source_type_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn generic_type_key(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .generic_type_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn function_key(&self, id: PersistentFunctionId) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .function_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn generic_function_key(
        &self,
        id: PersistentGenericFunctionId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .generic_function_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn constructor_key(
        &self,
        id: PersistentConstructorId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .constructor_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn property_key(&self, id: PersistentPropertyId) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .property_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn extension_property_key(
        &self,
        id: PersistentExtensionPropertyId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .extension_property_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn object_value_key(
        &self,
        id: PersistentObjectValueId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .object_value_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn annotation_key(
        &self,
        id: scoop_identity::PersistentAnnotationId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .annotation_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn type_alias_key(
        &self,
        id: PersistentTypeAliasId,
    ) -> Option<&SourceDeclarationKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .type_alias_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Option<&PropertyAccessorKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .property_accessor_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn field_key(&self, id: PersistentFieldId) -> Option<&FieldIdentityKey> {
        self.foundations()
            .find_map(|foundation| foundation.field_by_bytes(id.as_array()).map(|(_, key)| key))
    }

    pub(super) fn variant_key(
        &self,
        id: PersistentEnumVariantId,
    ) -> Option<&EnumVariantIdentityKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .enum_variant_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn variant_field_key(
        &self,
        id: PersistentEnumVariantFieldId,
    ) -> Option<&EnumVariantFieldKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .enum_variant_field_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn generated_callable_key(
        &self,
        id: PersistentGeneratedCallableId,
    ) -> Option<&GeneratedCallableKey> {
        self.foundations().find_map(|foundation| {
            foundation
                .generated_callable_by_bytes(id.as_array())
                .map(|(_, key)| key)
        })
    }

    pub(super) fn initialization_unit_key(
        &self,
        id: PersistentInitializationUnitId,
    ) -> Option<&InitializationUnitKey> {
        self.foundations().find_map(|foundation| {
            let records = foundation.type_source_initialization_records();
            records
                .binary_search_by_key(&id, |record| record.id())
                .ok()
                .map(|index| records[index].key())
        })
    }
}
