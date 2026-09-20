use super::*;

pub(super) fn nominal(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &NominalSupportNestedInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let nominals = authority.members().nominals;
    let owner = candidate.declaration();
    let declaration = NestedSupportDeclarationV1::NestedNominal(owner);
    query(nominals.table().records().len(), meter)?;
    let source = nominals.nominal_source(owner)?;
    let access = nominals.foundation.nominal_source(owner)?.access();
    compare(
        candidate.declaration_access(),
        access,
        declaration,
        "access",
        meter,
    )?;
    let actual = candidate.payload().source_interface();
    compare(&actual.kind(), &source.kind(), declaration, "kind", meter)?;
    compare(
        &actual.modality(),
        &source.modality(),
        declaration,
        "modality",
        meter,
    )?;
    compare(
        actual.type_parameters(),
        source.type_parameters(),
        declaration,
        "binders",
        meter,
    )?;
    compare(
        actual.supertypes(),
        source.supertypes(),
        declaration,
        "supertypes",
        meter,
    )?;
    compare(
        actual.constructors(),
        source.constructors(),
        declaration,
        "constructors",
        meter,
    )?;
    compare(
        actual.members(),
        source.members(),
        declaration,
        "members",
        meter,
    )?;
    compare(
        actual.children(),
        source.children(),
        declaration,
        "children",
        meter,
    )?;
    compare(
        actual.source_shape(),
        source.source_shape(),
        declaration,
        "shape",
        meter,
    )
}
pub(super) fn leaf(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &NestedSourceSupportV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let declaration = candidate.declaration();
    match candidate {
        NestedSourceSupportV1::Callable(actual) => {
            query(authority.members().callables().records().len(), meter)?;
            let expected = authority.members().callable_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "callable", meter)
        }
        NestedSourceSupportV1::Constructor(actual) => {
            query(authority.constructors().table().records().len(), meter)?;
            let expected = authority
                .constructors()
                .constructor_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "constructor", meter)
        }
        NestedSourceSupportV1::Property(actual) => {
            query(authority.members().properties().records().len(), meter)?;
            let expected = authority.members().property_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "property", meter)
        }
        NestedSourceSupportV1::NestedNominal(actual) => nominal(authority, actual, meter),
    }
}
