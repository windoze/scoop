use super::*;
use scoop_identity::{
    DecodedPersistentId, EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentFieldId, PersistentIdResolver, PersistentObjectValueId, SourceDeclarationKey,
};

impl NestedNominalSemanticAuthority<&'static str> for Fixture {
    fn nominal_source_binders(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalBinderListV1, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .type_parameters())
    }
    fn nominal_source_supertypes(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalSignatureTypesV1, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .supertypes())
    }
    fn nominal_source_constructors(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .constructors())
    }
    fn nominal_source_members(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedMemberRefsV1, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .members())
    }
    fn nominal_source_children(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedNominalRefsV1, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .children())
    }
    fn nominal_source_shape(
        &self,
        owner: SourceNominalId,
    ) -> Result<&NominalSourceShapeV1, &'static str> {
        Ok(self
            .nominal_sources
            .get(&owner)
            .ok_or("unknown source")?
            .source_shape())
    }
}
impl NominalSourceShapeSemanticAuthority<&'static str> for Fixture {
    fn struct_field_key(
        &mut self,
        field: PersistentFieldId,
    ) -> Result<FieldIdentityKey, &'static str> {
        self.struct_fields
            .get(&field)
            .cloned()
            .ok_or("unknown struct field")
    }
    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<EnumVariantIdentityKey, &'static str> {
        self.variants
            .get(&variant)
            .map(|value| value.0.clone())
            .ok_or("unknown variant")
    }
    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<EnumVariantFieldKey, &'static str> {
        self.variant_fields
            .get(&field)
            .cloned()
            .ok_or("unknown variant field")
    }
    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<SourceDeclarationKey, &'static str> {
        self.graph
            .keys
            .values()
            .find(|key| PersistentObjectValueId::from_source_object(key).ok() == Some(value))
            .cloned()
            .ok_or("unknown object")
    }
}
impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.graph
            .exacts
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown exact")
    }
}
impl PersistentIdResolver<PersistentFieldId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<PersistentFieldId, Self::Error> {
        self.struct_fields
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown field")
    }
}
impl PersistentIdResolver<PersistentEnumVariantFieldId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantFieldId>,
    ) -> Result<PersistentEnumVariantFieldId, Self::Error> {
        self.variant_fields
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown variant field")
    }
}
impl PersistentIdResolver<PersistentObjectValueId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentObjectValueId>,
    ) -> Result<PersistentObjectValueId, Self::Error> {
        self.graph
            .keys
            .values()
            .find_map(|key| {
                PersistentObjectValueId::from_source_object(key)
                    .ok()
                    .filter(|known| known.as_array() == id.as_array())
            })
            .ok_or("unknown object")
    }
}
