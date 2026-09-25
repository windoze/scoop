use super::*;

pub(super) fn validate(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
) -> Result<(), Error> {
    let members = bound.parameters.members();
    let foundation = members.nominals.foundation;
    for record in bound.slots.dispatch.callables.records() {
        let declaration = record.declaration();

        let source = members
            .callable_source(origin(declaration))
            .map_err(NominalNestedBindingError::from)?;
        let id = NestedSupportDeclarationV1::Callable(source.declaration());
        let payload = source.payload();
        compare(
            record.declaration_access(),
            source.declaration_access(),
            id,
            "access",
        )?;
        compare(&record.modality(), &payload.modality(), id, "modality")?;
        compare(
            &record.signature().effects(),
            &payload.effects(),
            id,
            "effects",
        )?;
        let signature = record.signature().exact_signature();
        let SourceNominalId::Concrete(owner) = payload.owner() else {
            return Err(Error::Callable {
                declaration,
                field: "concrete receiver",
            });
        };
        let receiver = signature.receiver().into_option().ok_or(Error::Callable {
            declaration,
            field: "receiver",
        })?;
        let expected = SignatureTypeKey::Nominal(owner);
        NominalRepresentationSupportV1::validate_exact_field_types(
            std::slice::from_ref(&expected),
            &[receiver],
            foundation,
        )
        .map_err(|e| Error::signature(declaration, e))?;
        if !payload.type_parameters().is_empty()
            || payload.parameters().parameters().len() != signature.parameters().len()
        {
            return Err(Error::Callable {
                declaration,
                field: "parameters",
            });
        }
        for (source, exact) in payload
            .parameters()
            .parameters()
            .iter()
            .zip(signature.parameters())
        {
            NominalRepresentationSupportV1::validate_exact_field_types(
                std::slice::from_ref(source.value_type()),
                std::slice::from_ref(exact),
                foundation,
            )
            .map_err(|e| Error::signature(declaration, e))?;
        }
        NominalRepresentationSupportV1::validate_exact_field_types(
            std::slice::from_ref(payload.result()),
            &[signature.result()],
            foundation,
        )
        .map_err(|e| Error::signature(declaration, e))?;
    }
    Ok(())
}
