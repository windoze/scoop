use super::*;
use scoop_identity::SourceDeclarationKind;

mod inheritance;

type Error = InheritanceProtectedCallableBindingError;

impl ProtectedCallableSemanticAuthority<Error>
    for BoundInheritanceProtectedCallableSourcesV1<'_, '_>
{
    fn callable_source_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.callable_key(declaration)
    }
    fn property_accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.foundation.accessor_key(accessor).map_err(Into::into)
    }
    fn property_value_type(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&SignatureTypeKey, Error> {
        let record = self
            .properties
            .get(property)
            .ok_or(Error::MissingProperty(property))?;
        match record.payload() {
            NominalSupportPropertyPayloadV1::Runtime { interface } => Ok(interface.value_type()),
            NominalSupportPropertyPayloadV1::Const { .. } => Err(Error::MissingProperty(property)),
        }
    }
    fn unit_type(&self) -> Result<PersistentTypeId, Error> {
        Ok(scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id())
    }
}

impl NominalInterfaceShapeAuthority<Error> for BoundInheritanceProtectedCallableSourcesV1<'_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        let key = self
            .foundation
            .identities
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(declaration)
            .map_err(|e| Error::Identity(e.to_string()))?;
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
            .map_err(|e| Error::Identity(e.to_string()))?;
        shape(&key)
    }
}

fn shape(key: &SourceDeclarationKey) -> Result<PublicNominalShapeV1, Error> {
    let kind = match key.declaration_kind() {
        SourceDeclarationKind::Class => PublicNominalKindV1::Class,
        SourceDeclarationKind::Struct => PublicNominalKindV1::Struct,
        SourceDeclarationKind::Enum => PublicNominalKindV1::Enum,
        SourceDeclarationKind::Interface => PublicNominalKindV1::Interface,
        SourceDeclarationKind::Object => PublicNominalKindV1::Object,
        _ => return Err(Error::NominalKind),
    };
    Ok(PublicNominalShapeV1::new(
        kind,
        key.duplicate_signature().type_parameter_count(),
    ))
}
