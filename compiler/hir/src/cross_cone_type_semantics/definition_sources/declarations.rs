use super::*;
use crate::{
    NominalSourcePropertyPayloadV1, ProtectedDeclarationInterfaceV1, ProtectedPropertyMutabilityV1,
};
use scoop_identity::PersistentPropertyId;

mod nested;

pub(super) fn visit<V: SourceVisitor<E>, E>(
    inputs: TypeDefinitionSourceInputsV1<'_>,
    validator: &mut V,

    path: &WirePath,
) -> Result<(), TypeDefinitionSourceClosureError<E>> {
    use TypeDefinitionSourceUseV1 as Use;

    for (index, record) in inputs.representations.records().iter().enumerate() {
        validator.observe(
            record.declaration_access().definition_origin(),
            Use::Representation(record),
            &path.clone().field(2).index(index as u64).field(2).field(3),
        )?;
    }

    for (index, record) in inputs.inheritance.records().iter().enumerate() {
        let at = path.clone().field(3).index(index as u64);

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
                &at.clone().field(6).index(index as u64).field(2).field(3),
            )?;
        }

        for (index, slot) in record.slots().records().iter().enumerate() {
            let at = at.clone().field(7).index(index as u64);
            validator.observe(
                slot.declaration_access().definition_origin(),
                Use::SlotDeclaration {
                    owner: record.owner(),
                    slot,
                },
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
                    &at.field(6).field(1).field(5).field(3),
                )?;
            }
        }
    }

    for (index, record) in inputs.protected_declarations.records().iter().enumerate() {
        let at = path.clone().field(4).index(index as u64);
        validator.observe(
            record.declaration_access().definition_origin(),
            Use::ProtectedDeclaration(record),
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
                &at.field(3),
            )?,
            ProtectedDeclarationInterfaceV1::NestedNominal(nominal) => {
                nested::visit(nominal.payload(), validator, at.field(3))?
            }
        }
    }
    Ok(())
}

fn setter<V: SourceVisitor<E>, E>(
    property: PersistentPropertyId,
    interface: &NominalSourcePropertyPayloadV1,
    validator: &mut V,

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
            &path.clone().field(4).field(2).field(3),
        ),
    }
}
