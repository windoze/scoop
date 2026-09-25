use super::*;

pub(super) fn nominal(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &NominalSupportNestedInterfaceV1,
) -> Result<(), Error> {
    let nominals = authority.members().nominals;
    let owner = candidate.declaration();
    let declaration = NestedSupportDeclarationV1::NestedNominal(owner);

    let source = nominals.nominal_source(owner)?;
    let access = nominals.foundation.nominal_source(owner)?.access();
    compare(
        candidate.declaration_access(),
        access,
        declaration,
        "access",
    )?;
    let actual = candidate.payload().source_interface();
    compare(&actual.kind(), &source.kind(), declaration, "kind")?;
    compare(
        &actual.modality(),
        &source.modality(),
        declaration,
        "modality",
    )?;
    compare(
        actual.type_parameters(),
        source.type_parameters(),
        declaration,
        "binders",
    )?;
    compare(
        actual.supertypes(),
        source.supertypes(),
        declaration,
        "supertypes",
    )?;
    compare(
        actual.constructors(),
        source.constructors(),
        declaration,
        "constructors",
    )?;
    compare(actual.members(), source.members(), declaration, "members")?;
    compare(
        actual.children(),
        source.children(),
        declaration,
        "children",
    )?;
    compare(
        actual.source_shape(),
        source.source_shape(),
        declaration,
        "shape",
    )
}
pub(super) fn leaf(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    candidate: &NestedSourceSupportV1,
) -> Result<(), Error> {
    let declaration = candidate.declaration();
    match candidate {
        NestedSourceSupportV1::Callable(actual) => {
            let expected = authority.members().callable_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "callable")
        }
        NestedSourceSupportV1::Constructor(actual) => {
            let expected = authority
                .constructors()
                .constructor_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "constructor")
        }
        NestedSourceSupportV1::Property(actual) => {
            let expected = authority.members().property_source(actual.declaration())?;
            compare(actual.as_ref(), expected, declaration, "property")
        }
        NestedSourceSupportV1::NestedNominal(actual) => nominal(authority, actual),
    }
}
