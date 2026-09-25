use super::*;

impl BoundNominalParameterProtocolsV1<'_, '_, '_, '_> {
    /// Replays constructor call parameters. A zero-argument adapter also needs
    /// complete parameter calling kinds; its body and access are checked separately.
    pub fn default_constructor_operation_shape(
        &self,
        reference: &DefaultConstructorRefV1,

        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, Error> {
        let nominals = self.members().nominals;
        let subject = nominals
            .foundation
            .default_constructor_access_subject(reference)?;
        let (declaration, source) = match reference {
            DefaultConstructorRefV1::Variant { declaration, .. } => {
                let declaration = CallableTemplateOrigin::VariantConstructor(*declaration);

                (
                    declaration,
                    self.members().callable_source(declaration)?.payload(),
                )
            }
            DefaultConstructorRefV1::Struct { .. } | DefaultConstructorRefV1::Class { .. } => {
                let DefinitionOriginSubject::Constructor(id) = subject else {
                    return Err(Error::ConstructorSubject(subject));
                };

                (
                    CallableTemplateOrigin::Constructor(id),
                    self.constructors().constructor_source(id)?.payload(),
                )
            }
        };
        let applied = Applied::new(nominals, reference.owner_type(), path)?;
        owner_matches(&applied, source.owner())?;

        let protocol = self.protocol(declaration)?;
        let parameters = if matches!(
            reference,
            DefaultConstructorRefV1::Class {
                declaration: DefaultClassConstructorIdV1::Generated(_),
                ..
            }
        ) {
            let CallableTemplateOrigin::Constructor(constructor) = declaration else {
                return Err(Error::ConstructorSubject(subject));
            };

            for (position, parameter) in protocol.parameters().iter().enumerate() {
                if parameter.calling_kind() == ProtectedParameterCallingKindV1::Required {
                    return Err(Error::AdapterRequiredParameter {
                        constructor,
                        position,
                    });
                }
            }
            Vec::new()
        } else {
            applied.sequence(
                source
                    .parameters()
                    .parameters()
                    .iter()
                    .map(SourceParameterShapeV1::value_type),
                path,
            )?
        };
        Ok(DefaultOperationEntityShapeV1::Constructor {
            owner_type: applied.owner_type(path)?,
            parameters,
        })
    }
}
