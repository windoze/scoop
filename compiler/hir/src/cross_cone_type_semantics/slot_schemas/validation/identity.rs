use scoop_identity::{
    AccessorRole, DefinitionOwnerAtom, DispatchDeclarationOwner, DispatchRole,
    PersistentDispatchSlotId, PersistentFunctionId, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyOwner, SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{InheritanceSlotSchemaSemanticAuthority, InheritanceSlotSchemaSemanticError};
use crate::{CheckedNominalInheritanceGraphV1, SourceNominalId};

pub(super) fn source_owner<A: InheritanceSlotSchemaSemanticAuthority<E>, E>(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    slot: PersistentDispatchSlotId,
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<scoop_identity::PersistentExactTypeId, InheritanceSlotSchemaSemanticError<E>> {
    let path = WirePath::root();
    meter
        .charge_work(1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
    let key = authority
        .dispatch_slot_key(slot)
        .map_err(InheritanceSlotSchemaSemanticError::Foundation)?;
    if PersistentDispatchSlotId::from_key(key).ok() != Some(slot) {
        return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
    }
    let declaration = match key.owner() {
        DispatchDeclarationOwner::Function(function) => {
            let declaration = authority
                .function_key(function)
                .map_err(InheritanceSlotSchemaSemanticError::Foundation)?;
            if PersistentFunctionId::from_source_declaration(declaration).ok() != Some(function)
                || !matches!(
                    key.role(),
                    DispatchRole::VirtualMethod | DispatchRole::InterfaceMethod
                )
            {
                return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
            }
            declaration
        }
        DispatchDeclarationOwner::Accessor(accessor) => {
            let accessor_key = authority
                .accessor_key(accessor)
                .map_err(InheritanceSlotSchemaSemanticError::Foundation)?;
            let expected = match accessor_key.role() {
                AccessorRole::Getter => DispatchRole::PropertyGetter,
                AccessorRole::Setter => DispatchRole::PropertySetter,
            };
            if PersistentPropertyAccessorId::from_key(accessor_key).ok() != Some(accessor)
                || key.role() != expected
            {
                return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
            }
            let PropertyOwner::Property(property) = accessor_key.owner() else {
                return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
            };
            let declaration = authority
                .property_key(property)
                .map_err(InheritanceSlotSchemaSemanticError::Foundation)?;
            if PersistentPropertyId::from_source_declaration(declaration).ok() != Some(property) {
                return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
            }
            declaration
        }
    };
    if declaration.duplicate_signature().type_parameter_count() != 0
        || declaration.duplicate_signature().receiver_is_present()
    {
        return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
    }
    let Some((DefinitionOwnerAtom::Type(owner), outer)) =
        declaration.owners().owners().split_last()
    else {
        return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
    };
    let source = graph
        .source(SourceNominalId::Concrete(*owner))
        .ok_or(InheritanceSlotSchemaSemanticError::SlotIdentity(slot))?;
    if source.key.origin() != declaration.origin()
        || source.key.package() != declaration.package()
        || source.key.owners().owners() != outer
    {
        return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
    }
    let kind = source.key.declaration_kind();
    let matches = match key.role() {
        DispatchRole::VirtualMethod => kind == SourceDeclarationKind::Class,
        DispatchRole::InterfaceMethod => kind == SourceDeclarationKind::Interface,
        DispatchRole::PropertyGetter | DispatchRole::PropertySetter => matches!(
            kind,
            SourceDeclarationKind::Class | SourceDeclarationKind::Interface
        ),
    };
    if !matches {
        return Err(InheritanceSlotSchemaSemanticError::SlotIdentity(slot));
    }
    graph
        .source_exact(SourceNominalId::Concrete(*owner))
        .map_err(InheritanceSlotSchemaSemanticError::Inheritance)
}
