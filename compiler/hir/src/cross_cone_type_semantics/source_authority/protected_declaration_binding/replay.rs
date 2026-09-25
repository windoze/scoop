use super::*;
use ProtectedDeclarationInterfaceV1 as Declaration;

pub(super) fn validate<'c>(
    authority: &mut BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
    checked: &mut BoundProtectedDeclarationSourcesV1<'c, '_, '_, '_, '_>,
) -> Result<(), Error> {
    for record in checked.table.records() {
        let owner = match record {
            Declaration::Callable(r) => {
                let source = authority
                    .members()
                    .callable_source(r.declaration())
                    .map_err(NominalNestedBindingError::from)?;
                let id = NestedSupportDeclarationV1::Callable(r.declaration());
                compare(
                    r.declaration_access(),
                    source.declaration_access(),
                    id,
                    "access",
                )?;
                compare(&**r.payload(), source.payload(), id, "callable")?;
                if matches!(r.declaration(), CallableTemplateOrigin::Accessor(_)) {
                    continue;
                }
                r.declaration()
            }
            Declaration::Constructor(r) => {
                let source = authority
                    .constructors()
                    .constructor_source(r.declaration())
                    .map_err(NominalNestedBindingError::from)?;
                let id = NestedSupportDeclarationV1::Constructor(r.declaration());
                compare(
                    r.declaration_access(),
                    source.declaration_access(),
                    id,
                    "access",
                )?;
                compare(&**r.payload(), source.payload(), id, "constructor")?;
                CallableTemplateOrigin::Constructor(r.declaration())
            }
            Declaration::Property(r) => {
                let source = authority
                    .members()
                    .property_source(r.declaration())
                    .map_err(NominalNestedBindingError::from)?;
                let id = NestedSupportDeclarationV1::Property(r.declaration());
                let NominalSupportPropertyPayloadV1::Runtime { interface } = source.payload()
                else {
                    return Err(NominalNestedBindingError::Contract {
                        declaration: id,
                        field: "runtime property",
                    }
                    .into());
                };
                compare(
                    r.declaration_access(),
                    source.declaration_access(),
                    id,
                    "access",
                )?;
                compare(&**r.payload(), interface, id, "property")?;
                continue;
            }
            Declaration::NestedNominal(r) => {
                let nested = authority.validate_nested_source(
                    r.source_record(),
                    protocols,
                    checked.representations,
                )?;
                for protocol in nested.protocols() {
                    retain(checked, protocol)?;
                }
                continue;
            }
        };

        let candidate = protocols
            .get(owner)
            .ok_or(NominalNestedBindingError::MissingProtocol(owner))?;
        let protocol = authority
            .validate_source_protocol(candidate)
            .map_err(NominalNestedBindingError::from_protocol)?;
        retain(checked, protocol)?;
    }
    Ok(())
}
