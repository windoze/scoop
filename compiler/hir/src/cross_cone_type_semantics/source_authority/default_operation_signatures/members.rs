use super::*;

impl BoundNominalMemberSourcesV1<'_, '_, '_> {
    /// Supplies a nominal member's source signature, including its implicit
    /// receiver. Access, receiver conversion and argument bounds remain separate.
    pub fn default_member_callable_shape(
        &self,
        reference: &DefaultCallableRefV1,

        path: &WirePath,
    ) -> Result<DefaultCallableOperationShapeV1, Error> {
        let declaration = match reference.declaration() {
            DefaultCallableDeclarationV1::Function(id) => CallableTemplateOrigin::Function(id),
            DefaultCallableDeclarationV1::GenericFunction(id) => {
                CallableTemplateOrigin::GenericFunction(id)
            }
            DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                CallableTemplateOrigin::Accessor(id)
            }
            declaration @ DefaultCallableDeclarationV1::Generated(_) => {
                return Err(Error::Declaration(declaration));
            }
        };
        let source = self.callable_source(declaration)?.payload();
        let inferred;
        let owner_type = match reference.owner() {
            OptionalSignatureType::Present(owner) => owner,
            OptionalSignatureType::Absent => match source.owner() {
                SourceNominalId::Concrete(id) => {
                    inferred = SignatureTypeKey::Nominal(id);
                    &inferred
                }
                owner @ SourceNominalId::GenericTemplate(_) => {
                    return Err(Error::MissingOwner(owner));
                }
            },
        };
        let applied = Applied::for_callable(
            self.nominals,
            owner_type,
            source.type_parameters().len_u32(),
            reference.type_arguments(),
            path,
        )?;
        owner_matches(&applied, source.owner())?;
        Ok(DefaultCallableOperationShapeV1::new(
            source.effects().execution(),
            Some(applied.owner_type(path)?),
            Vec::new(),
            applied.sequence(
                source
                    .parameters()
                    .parameters()
                    .iter()
                    .map(SourceParameterShapeV1::value_type),
                path,
            )?,
            applied.field_type(source.result())?,
        ))
    }
}
