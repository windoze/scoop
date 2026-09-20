use super::*;
use ProtectedDeclarationInterfaceV1 as Declaration;

pub(super) fn validate<'c>(
    authority: &mut BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
    checked: &mut BoundProtectedDeclarationSourcesV1<'c, '_, '_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    for record in checked.table.records() {
        meter.charge_nodes(1, &WirePath::root())?;
        let owner = match record {
            Declaration::Callable(r) => {
                query(authority.members().callables().records().len(), meter)?;
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
                    meter,
                )?;
                compare(&**r.payload(), source.payload(), id, "callable", meter)?;
                if matches!(r.declaration(), CallableTemplateOrigin::Accessor(_)) {
                    continue;
                }
                r.declaration()
            }
            Declaration::Constructor(r) => {
                query(authority.constructors().table().records().len(), meter)?;
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
                    meter,
                )?;
                compare(&**r.payload(), source.payload(), id, "constructor", meter)?;
                CallableTemplateOrigin::Constructor(r.declaration())
            }
            Declaration::Property(r) => {
                query(authority.members().properties().records().len(), meter)?;
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
                    meter,
                )?;
                compare(&**r.payload(), interface, id, "property", meter)?;
                continue;
            }
            Declaration::NestedNominal(r) => {
                let nested = authority.validate_nested_source(
                    r.source_record(),
                    protocols,
                    checked.representations,
                    meter,
                )?;
                for protocol in nested.protocols() {
                    retain(checked, protocol, meter)?;
                }
                continue;
            }
        };
        query(protocols.records().len(), meter)?;
        let candidate = protocols
            .get(owner)
            .ok_or(NominalNestedBindingError::MissingProtocol(owner))?;
        let protocol = authority
            .validate_source_protocol(candidate, meter)
            .map_err(NominalNestedBindingError::from_protocol)?;
        retain(checked, protocol, meter)?;
    }
    Ok(())
}
