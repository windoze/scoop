use super::*;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKind};

impl NominalInterfaceShapeAuthority<Error> for BoundNominalSourceContractsV1<'_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        let key = self
            .foundation
            .identities
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(declaration)
            .map_err(|error| Error::Identity(error.to_string()))?;
        shape(&key)
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        let key = self
            .foundation
            .identities
            .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(declaration)
            .map_err(|error| Error::Identity(error.to_string()))?;
        shape(&key)
    }
}

pub(super) fn shape(key: &SourceDeclarationKey) -> Result<PublicNominalShapeV1, Error> {
    let kind = match key.declaration_kind() {
        SourceDeclarationKind::Class => PublicNominalKindV1::Class,
        SourceDeclarationKind::Struct => PublicNominalKindV1::Struct,
        SourceDeclarationKind::Enum => PublicNominalKindV1::Enum,
        SourceDeclarationKind::Interface => PublicNominalKindV1::Interface,
        SourceDeclarationKind::Object => PublicNominalKindV1::Object,
        _ => return Err(Error::Identity("expected a source nominal key".into())),
    };
    Ok(PublicNominalShapeV1::new(
        kind,
        key.duplicate_signature().type_parameter_count(),
    ))
}

/// The legacy source-shape visitor requests owned keys. Charge each copy before
/// returning it; all recursive signature semantics are metered by the caller.
pub(super) struct ShapeAuthority<'b, 'a, 'f> {
    pub bound: &'b mut BoundNominalSourceContractsV1<'a, 'f>,
    pub meter: &'b mut BudgetMeter,
}

impl NominalInterfaceShapeAuthority<Error> for ShapeAuthority<'_, '_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.bound.concrete_nominal_shape(declaration)
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.bound.generic_nominal_shape(declaration)
    }
}
impl NominalSourceShapeSemanticAuthority<Error> for ShapeAuthority<'_, '_, '_> {
    fn struct_field_key(&mut self, field: PersistentFieldId) -> Result<FieldIdentityKey, Error> {
        copy(self.bound.struct_field_key(field)?, self.meter)
    }
    fn enum_variant_key(
        &mut self,
        variant: PersistentEnumVariantId,
    ) -> Result<EnumVariantIdentityKey, Error> {
        copy(self.bound.enum_variant_key(variant)?, self.meter)
    }
    fn enum_variant_field_key(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<EnumVariantFieldKey, Error> {
        copy(self.bound.enum_variant_field_key(field)?, self.meter)
    }
    fn object_value_key(
        &mut self,
        value: PersistentObjectValueId,
    ) -> Result<SourceDeclarationKey, Error> {
        let key = self.bound.object_value_key(value)?;
        let path = WirePath::root();
        NominalRepresentationSupportV1::charge_source_key_resources(key, self.meter, &path)?;
        self.meter
            .charge_collection_slots(key.owners().owners().len() as u64, &path)?;
        copy(key, self.meter)
    }
}
fn copy<T: Clone + scoop_wire::WireEncode>(key: &T, meter: &mut BudgetMeter) -> Result<T, Error> {
    let path = WirePath::root();
    let bytes =
        scoop_wire::encoded_length(key).map_err(|error| Error::Identity(error.to_string()))?;
    meter.check_semantic_leaf(bytes, &path)?;
    meter.charge_owned_bytes(bytes, &path)?;
    meter.charge_nodes(1, &path)?;
    meter.charge_work(bytes, &path)?;
    Ok(key.clone())
}
