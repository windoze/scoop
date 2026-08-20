use super::*;
use scoop_mir::MirDispatchImplementationV1 as Implementation;

pub(super) fn object_overrides(
    output: &hir::DependencyHirOutput,
    input: &ConeMirInput,
    types: &CanonicalParamFreeMirTypeExportsV1,
    authority: MirDispatchSchemaAuthority<'_>,
    schemas: &CanonicalMirDispatchSchemasV1,
) {
    let owners = source_dispatch::owners(output);
    let owner = owners["Singleton"];
    let scoop_mir::MirTypeRepresentationV1::Object { backing } =
        *types.get(owner).unwrap().representation()
    else {
        unreachable!()
    };
    assert_eq!(
        authority.canonical_receiver_path(backing, owner).unwrap(),
        [backing, owner]
    );
    assert!(matches!(
        authority.canonical_receiver_path(backing, owners["OtherSingleton"]),
        Err(scoop_mir::MirDispatchSchemaError::MissingReceiverPath { .. })
    ));
    let targets: Vec<_> = schemas
        .get(owner)
        .unwrap()
        .vtable()
        .iter()
        .map(|entry| {
            let root = input
                .materialization()
                .callable_roots()
                .iter()
                .find(|root| {
                    root.subject()
                        == scoop_mir::CallableSignatureSubject::Strong(
                            entry
                                .implementation()
                                .target()
                                .strong_owner()
                                .unwrap()
                                .callable_owner(),
                        )
                })
                .unwrap();
            input.module().functions[root.function()].name.as_str()
        })
        .collect();
    assert_eq!(targets, ["Singleton.run", "Singleton.$get$token"]);
}

pub(super) fn actual(
    input: &ConeMirInput,
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    types: &CanonicalParamFreeMirTypeExportsV1,
    authority: MirDispatchSchemaAuthority<'_>,
    schemas: &CanonicalMirDispatchSchemasV1,
) {
    for record in types.records().iter().filter(|record| {
        !matches!(
            record.origin(),
            scoop_mir::MirTypeOriginV1::NominalApplication(_)
        )
    }) {
        assert!(schemas.get(record.exact()).is_some());
    }
    let roots = input.materialization().callable_roots();
    for schema in schemas.records() {
        let ty = types.get(schema.owner()).unwrap();
        if matches!(
            ty.origin(),
            scoop_mir::MirTypeOriginV1::NominalApplication(_)
        ) {
            continue;
        }
        let source_owner = types
            .records()
            .iter()
            .find_map(|record| match record.representation() {
                scoop_mir::MirTypeRepresentationV1::Object { backing }
                    if *backing == schema.owner() =>
                {
                    Some(record.exact())
                }
                _ => None,
            })
            .unwrap_or(schema.owner());
        let source = hir.inheritance().get(source_owner).unwrap();
        if let Some(slots) = schema.interface_slots() {
            assert!(schema.itables().is_empty());
            let table = source
                .slot_schemas()
                .get(hir::InheritanceSlotSchemaRoleV1::Interface {
                    interface_exact: schema.owner(),
                })
                .unwrap();
            assert!(
                slots
                    .iter()
                    .map(|slot| slot.slot())
                    .eq(table.slots().iter().copied())
            );
            for slot in slots {
                assert_eq!(
                    slot.signature().exact().receiver().into_option(),
                    Some(schema.owner())
                );
            }
            continue;
        }
        for table in source.slot_schemas().records() {
            let entries = match table.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => schema.vtable(),
                hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                    schema.interface_table(interface_exact).unwrap().entries()
                }
            };
            assert!(
                entries
                    .iter()
                    .map(|entry| entry.slot())
                    .eq(table.slots().iter().copied())
            );
        }
        if matches!(
            ty.representation(),
            scoop_mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            assert_eq!(schema.vtable(), schemas.get(source_owner).unwrap().vtable());
            assert_eq!(
                schema.itables(),
                schemas.get(source_owner).unwrap().itables()
            );
        }
        for entry in schema
            .vtable()
            .iter()
            .chain(schema.itables().iter().flat_map(|table| table.entries()))
        {
            let target = entry.implementation().target();
            let root = roots
                .iter()
                .find(|root| {
                    root.subject()
                        == scoop_mir::CallableSignatureSubject::Strong(
                            target.strong_owner().unwrap().callable_owner(),
                        )
                })
                .unwrap();
            let binding = authority.callables.get(target).unwrap();
            assert_eq!(
                binding.lowered_signature().gc_effect(),
                input.module().functions[root.function()].gc_effect
            );
            if let Implementation::AbstractObligation { .. } = entry.implementation() {
                let body = &input.module().functions[root.function()].body;
                assert!(matches!(
                    body.blocks[body.entry].terminator,
                    scoop_mir::Terminator::Trap { .. }
                ));
            }
        }
    }
}
