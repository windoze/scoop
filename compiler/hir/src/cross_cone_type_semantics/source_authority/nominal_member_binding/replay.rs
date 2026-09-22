use super::*;
use scoop_identity::{
    PersistentPropertyAccessorId, PersistentTypeId, PropertyAccessorKey, SignatureTypeKey,
};

mod inheritance;
mod nominals;

impl ProtectedCallableSemanticAuthority<Error> for BoundNominalMemberSourcesV1<'_, '_, '_> {
    fn callable_source_key(
        &self,
        declaration: CallableTemplateOrigin,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.callable_key(declaration)
    }
    fn property_accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, Error> {
        self.nominals
            .foundation
            .accessor_key(id)
            .map_err(Into::into)
    }
    fn property_value_type(&self, id: PersistentPropertyId) -> Result<&SignatureTypeKey, Error> {
        match self.property_source(id)?.payload() {
            NominalSupportPropertyPayloadV1::Runtime { interface } => Ok(interface.value_type()),
            NominalSupportPropertyPayloadV1::Const { value } => Ok(value.value_type()),
        }
    }
    fn unit_type(&self) -> Result<PersistentTypeId, Error> {
        Ok(scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id())
    }
}
impl ProtectedPropertySemanticAuthority<Error> for BoundNominalMemberSourcesV1<'_, '_, '_> {
    fn property_source_key(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, Error> {
        self.property_key(id)
    }
    fn property_source_shape(
        &self,
        id: PersistentPropertyId,
    ) -> Result<ProtectedPropertySourceShapeV1, Error> {
        let NominalSupportPropertyPayloadV1::Runtime { interface } =
            self.property_source(id)?.payload()
        else {
            return Err(Error::MissingProperty(id));
        };
        Ok(ProtectedPropertySourceShapeV1 {
            getter: interface.getter(),
            setter: match interface.mutability() {
                ProtectedPropertyMutabilityV1::ReadOnly => None,
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } => Some((*setter, setter_access.declared_visibility())),
            },
            representation: interface.representation(),
        })
    }
}
impl NominalSupportPropertySemanticAuthority<Error> for BoundNominalMemberSourcesV1<'_, '_, '_> {
    fn const_source(
        &self,
        id: PersistentPropertyId,
    ) -> Result<&ConstPropertyDeclarationSourceV1, Error> {
        self.constants.get(&id).ok_or(Error::MissingProperty(id))
    }
    fn canonical_const_value_type(
        &self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, Error> {
        Ok(match kind {
            CanonicalConstValueKindV1::Integer(kind) => self.core.integer(kind).persistent(),
            CanonicalConstValueKindV1::Boolean => self.core.boolean().persistent(),
            CanonicalConstValueKindV1::String => self.core.string().persistent(),
        })
    }
}
