use super::*;
use crate::{NestedSourceSupportV1, NominalSupportNestedInterfaceV1};

pub(super) fn validate<'a>(
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    record: &NominalSupportNestedInterfaceV1,
    context: &Context<'_>,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    let owner = record.declaration();

    let expected_access = &context.source(owner)?.access;

    let source = metadata
        .public
        .nominal_interfaces()
        .declaration(owner)
        .ok_or(Error::NestedContract(owner))?;
    let actual = record.payload().source_interface();

    let details = source.declaration_details();
    if record.declaration_access() != expected_access
        || actual.kind() != source.kind()
        || actual.modality() != details.modality()
        || actual.type_parameters() != source.type_parameters()
        || actual.supertypes() != source.exact_supertypes()
        || actual.constructors() != details.constructors()
        || actual.members() != details.members()
        || actual.children() != details.children()
        || actual.source_shape() != source.source_shape()
    {
        return Err(Error::NestedContract(owner));
    }
    for child in actual.source_support().records() {
        match child {
            NestedSourceSupportV1::Callable(callable) => contracts::validate_callable(
                metadata,
                callable.declaration(),
                callable.declaration_access(),
                callable.payload(),
            )?,
            NestedSourceSupportV1::Constructor(constructor) => contracts::validate_callable(
                metadata,
                CallableTemplateOrigin::Constructor(constructor.declaration()),
                constructor.declaration_access(),
                constructor.payload(),
            )?,
            NestedSourceSupportV1::Property(property) => properties::support(
                MetadataTypes {
                    current: metadata,
                    dependencies,
                },
                property,
            )?,
            NestedSourceSupportV1::NestedNominal(nominal) => {
                validate(provider, dependencies, nominal, context)?
            }
        }
    }
    Ok(())
}
