use super::{validation::Validator, *};
use crate::{
    NominalSourcePropertyPayloadV1, ProtectedDeclarationInterfaceV1, ProtectedPropertyMutabilityV1,
};
use scoop_identity::PersistentPropertyId;

mod nested;

pub(super) fn visit<A: TypeDefinitionSourceSemanticAuthority<E>, E>(
    inputs: TypeDefinitionSourceInputsV1<'_>,
    validator: &mut Validator<'_, A>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    use TypeDefinitionSourceUseV1 as Use;
    meter.check_table_entries(inputs.representations.records().len() as u64, path)?;
    for (index, record) in inputs.representations.records().iter().enumerate() {
        validator.observe(
            record.declaration_access().definition_origin(),
            Use::Representation(record),
            meter,
            &path.clone().field(2).index(index as u64).field(2).field(3),
        )?;
    }
    meter.check_table_entries(inputs.inheritance.records().len() as u64, path)?;
    for (index, record) in inputs.inheritance.records().iter().enumerate() {
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        let at = path.clone().field(3).index(index as u64);
        meter.check_table_entries(record.constructors().records().len() as u64, &at)?;
        for (index, constructor) in record.constructors().records().iter().enumerate() {
            validator.observe(
                constructor
                    .source()
                    .declaration_access()
                    .definition_origin(),
                Use::InheritanceConstructor {
                    owner: record.owner(),
                    constructor,
                },
                meter,
                &at.clone().field(6).index(index as u64).field(2).field(3),
            )?;
        }
        meter.check_table_entries(record.slots().records().len() as u64, &at)?;
        for (index, slot) in record.slots().records().iter().enumerate() {
            let at = at.clone().field(7).index(index as u64);
            validator.observe(
                slot.declaration_access().definition_origin(),
                Use::SlotDeclaration {
                    owner: record.owner(),
                    slot,
                },
                meter,
                &at.clone().field(7).field(3),
            )?;
            if let Some(target) = slot.implementation().target() {
                validator.observe(
                    target.declaration_access().definition_origin(),
                    Use::SlotImplementation {
                        owner: record.owner(),
                        slot,
                        target,
                    },
                    meter,
                    &at.field(6).field(1).field(5).field(3),
                )?;
            }
        }
    }
    meter.check_table_entries(inputs.protected_declarations.records().len() as u64, path)?;
    for (index, record) in inputs.protected_declarations.records().iter().enumerate() {
        let at = path.clone().field(4).index(index as u64);
        validator.observe(
            record.declaration_access().definition_origin(),
            Use::ProtectedDeclaration(record),
            meter,
            &at.clone().field(2).field(3),
        )?;
        match record {
            ProtectedDeclarationInterfaceV1::Callable(_)
            | ProtectedDeclarationInterfaceV1::Constructor(_) => {
                // Their complete source signatures contain no further inline origins.
            }
            ProtectedDeclarationInterfaceV1::Property(property) => setter(
                property.declaration(),
                property.payload(),
                validator,
                meter,
                &at.field(3),
            )?,
            ProtectedDeclarationInterfaceV1::NestedNominal(nominal) => {
                nested::visit(nominal.payload(), validator, meter, at.field(3))?
            }
        }
    }
    Ok(())
}

fn setter<A: TypeDefinitionSourceSemanticAuthority<E>, E>(
    property: PersistentPropertyId,
    interface: &NominalSourcePropertyPayloadV1,
    validator: &mut Validator<'_, A>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    match interface.mutability() {
        ProtectedPropertyMutabilityV1::ReadOnly => Ok(()),
        ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } => validator.observe(
            setter_access.definition_origin(),
            TypeDefinitionSourceUseV1::PropertySetter {
                property,
                interface,
            },
            meter,
            &path.clone().field(4).field(2).field(3),
        ),
    }
}
