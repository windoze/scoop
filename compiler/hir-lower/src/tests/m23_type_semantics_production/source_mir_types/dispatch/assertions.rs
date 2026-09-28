use super::*;
use scoop_mir::{MirClassVtableSchemaV1, MirDispatchImplementationV1 as Implementation};

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
        .entries()
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
    assert_eq!(schemas.records().len(), types.records().len());
    let roots = input.materialization().callable_roots();
    for schema in schemas.records() {
        let ty = types.get(schema.owner()).unwrap();
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
        for table in source.slot_schemas().records() {
            let entries = match table.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => schema.vtable().entries(),
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
            .entries()
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

pub(super) fn dump(
    output: &hir::DependencyHirOutput,
    input: &ConeMirInput,
    types: &CanonicalParamFreeMirTypeExportsV1,
    schemas: &CanonicalMirDispatchSchemasV1,
) -> String {
    let mut names: BTreeMap<_, _> = source_dispatch::owners(output)
        .into_iter()
        .map(|(name, exact)| (exact, name))
        .collect();
    for record in types.records() {
        if let scoop_mir::MirTypeRepresentationV1::Object { backing } = record.representation() {
            names.insert(*backing, format!("{} backing", names[&record.exact()]));
        }
    }
    let targets: BTreeMap<_, _> = input
        .materialization()
        .callable_roots()
        .iter()
        .filter_map(|root| {
            let scoop_mir::CallableSignatureSubject::Strong(owner) = root.subject() else {
                return None;
            };
            Some((
                owner,
                input.module().functions[root.function()].name.as_str(),
            ))
        })
        .collect();
    let mut blocks = Vec::new();
    for schema in schemas.records() {
        let mut text = format!("{}\n", names[&schema.owner()]);
        let tables = match schema.vtable() {
            MirClassVtableSchemaV1::NoClassVtable => Vec::new(),
            MirClassVtableSchemaV1::ClassVtable(entries) => {
                vec![("vtable".to_owned(), entries.as_slice())]
            }
        };
        for (name, entries) in tables
            .into_iter()
            .chain(schema.itables().iter().map(|table| {
                (
                    format!("itable {}", names[&table.interface()]),
                    table.entries(),
                )
            }))
        {
            text.push_str(&format!("  {name}\n"));
            for entry in entries {
                let role = match entry.implementation() {
                    Implementation::AbstractObligation { .. } => "abstract",
                    Implementation::DirectStrongTarget { .. } => "direct",
                    Implementation::InterfaceDefaultTarget { .. } => "default",
                    Implementation::AdjustThunkTarget(_) => "adjust",
                };
                text.push_str(&format!(
                    "    {} {role} {}\n",
                    entry.position().get(),
                    targets[&entry
                        .implementation()
                        .target()
                        .strong_owner()
                        .unwrap()
                        .callable_owner()]
                ));
            }
        }
        blocks.push(text);
    }
    blocks.sort();
    blocks.concat()
}
