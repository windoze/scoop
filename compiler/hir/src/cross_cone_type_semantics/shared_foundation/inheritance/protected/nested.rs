use super::*;
use crate::{NestedSourceSupportV1, NominalSupportNestedInterfaceV1};

pub(super) fn validate<'a>(
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    record: &NominalSupportNestedInterfaceV1,
    context: &Context<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_nodes(1, &path)?;
    let metadata = provider.metadata;
    let owner = record.declaration();
    contracts::lookup(context.sources.len(), meter)?;
    let expected_access = &context.source(owner)?.access;
    contracts::charge_compare(record.declaration_access(), expected_access, meter)?;
    contracts::lookup(
        metadata.public.nominal_interfaces().declaration_count(),
        meter,
    )?;
    let source = metadata
        .public
        .nominal_interfaces()
        .declaration(owner)
        .ok_or(Error::NestedContract(owner))?;
    let actual = record.payload().source_interface();
    contracts::charge_compare(source, actual, meter)?;
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
    record
        .validate_concrete_support(graph, provider.section.representation_support(), meter)
        .map_err(|error| Error::NestedSupport(Box::new(error)))?;
    for child in actual.source_support().records() {
        meter.charge_work(1, &path)?;
        match child {
            NestedSourceSupportV1::Callable(callable) => contracts::validate_callable(
                metadata,
                callable.declaration(),
                callable.declaration_access(),
                callable.payload(),
                meter,
            )?,
            NestedSourceSupportV1::Constructor(constructor) => contracts::validate_callable(
                metadata,
                CallableTemplateOrigin::Constructor(constructor.declaration()),
                constructor.declaration_access(),
                constructor.payload(),
                meter,
            )?,
            NestedSourceSupportV1::Property(property) => properties::support(
                MetadataTypes {
                    current: metadata,
                    dependencies,
                },
                property,
                meter,
            )?,
            NestedSourceSupportV1::NestedNominal(nominal) => validate(
                provider,
                dependencies,
                nominal,
                context,
                graph,
                meter,
                depth + 1,
            )?,
        }
    }
    Ok(())
}
