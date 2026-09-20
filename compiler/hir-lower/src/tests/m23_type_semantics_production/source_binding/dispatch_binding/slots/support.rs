use super::*;

pub(super) fn candidate(
    authority: &hir::BoundInheritanceSlotSourcesV1<'_, '_, '_>,
    selection: hir::InheritanceSourceSlotSelectionRecordV1,
) -> hir::InheritanceSlotContractV1 {
    let key = authority.dispatch_slot_key(selection.slot()).unwrap();
    let declaration = match (key.owner(), key.role()) {
        (DispatchDeclarationOwner::Function(id), _) => Decl::Function(id),
        (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertyGetter) => Decl::Getter(id),
        (DispatchDeclarationOwner::Accessor(id), DispatchRole::PropertySetter) => Decl::Setter(id),
        _ => panic!("slot declaration role"),
    };
    let source = authority.inheritance_callable_source(declaration).unwrap();
    let key = match declaration {
        Decl::Function(id) => authority.function_key(id).unwrap(),
        Decl::Getter(id) | Decl::Setter(id) => {
            let PropertyOwner::Property(id) = authority.accessor_key(id).unwrap().owner() else {
                panic!("ordinary property")
            };
            authority.property_key(id).unwrap()
        }
    };
    let checked = authority
        .graph()
        .check_declaration_source(source.declaration_access, key, authority, &mut meter())
        .unwrap();
    let domains = authority
        .graph()
        .replay_declaration_access(checked, &mut meter())
        .unwrap();
    let implementation = match selection.selection() {
        hir::InheritanceSourceSlotSelectionV1::Abstract => Implementation::Abstract,
        hir::InheritanceSourceSlotSelectionV1::Concrete(declaration) => {
            Implementation::Concrete(target(authority, declaration))
        }
        hir::InheritanceSourceSlotSelectionV1::InterfaceDefault(declaration) => {
            Implementation::InterfaceDefault(target(authority, declaration))
        }
    };
    hir::InheritanceSlotContractV1::try_new(
        selection.slot(),
        source_owner(source.declaration_access),
        declaration,
        source.signature.clone(),
        hir::PersistentSlotContractDomainV1::new(domains.lookup().domain().clone()),
        implementation,
        source.declaration_access.clone(),
    )
    .unwrap()
}

pub(super) fn target(
    authority: &hir::BoundInheritanceSlotSourcesV1<'_, '_, '_>,
    declaration: Decl,
) -> hir::InheritanceSlotTargetV1 {
    let source = authority.inheritance_callable_source(declaration).unwrap();
    hir::InheritanceSlotTargetV1::try_new(
        declaration,
        source_owner(source.declaration_access),
        source.signature.clone(),
        source.modality,
        source.declaration_access.clone(),
    )
    .unwrap()
}
fn source_owner(access: &hir::DeclarationAccessSourceV1) -> PersistentTypeId {
    match access.lexical_owners().last() {
        Some(hir::SourceNominalId::Concrete(id)) => *id,
        _ => panic!("param-free source owner"),
    }
}

pub(super) fn replace(
    record: &hir::InheritanceSlotContractV1,
    signature: hir::InheritanceCallableSignatureV1,
    implementation: Implementation,
    access: hir::DeclarationAccessSourceV1,
) -> hir::InheritanceSlotContractV1 {
    hir::InheritanceSlotContractV1::try_new(
        record.slot(),
        record.declaration_owner(),
        record.declaration(),
        signature,
        record.domain().clone(),
        implementation,
        access,
    )
    .unwrap()
}
