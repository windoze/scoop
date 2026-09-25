use super::*;
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey};

pub(super) struct Shapes<'s, 'a, 'f>(pub &'s BoundNominalSourceContractsV1<'a, 'f>);
impl NominalInterfaceShapeAuthority<Error> for Shapes<'_, '_, '_> {
    fn concrete_nominal_shape(
        &mut self,
        id: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.0
            .signature_nominal_shape(SourceNominalId::Concrete(id))
            .map_err(Error::from)
    }
    fn generic_nominal_shape(
        &mut self,
        id: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, Error> {
        self.0
            .signature_nominal_shape(SourceNominalId::GenericTemplate(id))
            .map_err(Error::from)
    }
}
pub(super) fn validate(
    declaration: PersistentConstructorId,
    scope: &SignatureBinderScopeV1,
    signature: &SignatureTypeKey,
    shapes: &mut Shapes<'_, '_, '_>,
) -> Result<(), Error> {
    scope
        .validate_signature_semantics(signature, shapes)
        .map_err(|error| match error {
            SignatureTypeSemanticError::Allocation(error) => Error::Resource(error),
            error => invalid(declaration, error),
        })
}
