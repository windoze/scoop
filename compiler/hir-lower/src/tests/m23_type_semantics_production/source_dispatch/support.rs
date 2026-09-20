use super::*;

pub(super) fn with_source<T>(
    source: &str,
    run: impl FnOnce(&hir::OrdinaryHirOutput<'_>, &scoop_mir::Module) -> T,
) -> T {
    let core = trusted_core();
    let identity = test_source_identity("src/main.scoop");
    let ordinary = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            ast::IdentifiedParsedSource::new(
                identity.clone(),
                scoop_parser::parse(source).unwrap(),
            ),
            Vec::new(),
        ))
        .unwrap(),
        ast::NonEmptyVec::new(
            ast::CurrentSourceText::new(identity.clone(), source.to_owned()),
            Vec::new(),
        ),
        ast::NonEmptyVec::new(
            ast::CurrentSourceDiagnosticContext::new(
                identity,
                std::path::PathBuf::from("src/main.scoop"),
            ),
            Vec::new(),
        ),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinaryCoreOnlySources::try_new(&ordinary, core_inputs).unwrap();
    let output =
        lower_ordinary_core_only(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let mut imported = core.project_selected_callables_to_mir(output.imported_core());
    if !output
        .output()
        .local
        .module()
        .initialization_units
        .is_empty()
    {
        core.project_initialization_cycle_to_mir(&mut imported);
    }
    let dependencies =
        scoop_mir::SelectedDependencyMirSet::empty(output.output().local.module().cone);
    let mir = scoop_mir_lower::lower_ordinary(&output, imported, dependencies).unwrap();
    run(&output, mir.module())
}

pub(super) fn project(
    output: &hir::OrdinaryHirOutput<'_>,
) -> hir::CanonicalSourceInheritanceInventoriesV1 {
    hir::CanonicalSourceInheritanceInventoriesV1::from_ordinary_hir(
        output,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap()
}

pub(super) fn owners(
    output: &hir::OrdinaryHirOutput<'_>,
) -> BTreeMap<String, PersistentExactTypeId> {
    let export = output.output().export.module();
    let mut result = BTreeMap::new();
    let mut add = |name: &str, ty: hir::TypeId| {
        result.insert(
            name.to_owned(),
            export.type_identities[ty].exact().unwrap().id(),
        );
    };
    for id in &export.public_surface.classes {
        add(
            &export.classes[*id].name,
            export.class_applications[export.classes[*id].self_application].canonical_type,
        );
    }
    for id in &export.public_surface.interfaces {
        add(
            &export.interfaces[*id].name,
            export.interface_applications[export.interfaces[*id].self_application].canonical_type,
        );
    }
    for id in &export.public_surface.objects {
        add(
            &export.objects[*id].name,
            export.object_types[export.objects[*id].object_type].canonical_type,
        );
    }
    for id in &export.public_surface.structs {
        add(
            &export.structs[*id].name,
            export.struct_applications[export.structs[*id].self_application].canonical_type,
        );
    }
    for id in &export.public_surface.enums {
        add(
            &export.enums[*id].name,
            export.enum_applications[export.enums[*id].self_application].canonical_type,
        );
    }
    result
}

pub(super) fn owner(output: &hir::OrdinaryHirOutput<'_>, name: &str) -> PersistentExactTypeId {
    owners(output)[name]
}

pub(super) fn render(
    output: &hir::OrdinaryHirOutput<'_>,
    inventory: &hir::CanonicalSourceInheritanceInventoriesV1,
) -> String {
    let export = output.output().export.module();
    let owners = owners(output);
    let reverse = owners
        .iter()
        .map(|(name, id)| (*id, name.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut slots = BTreeMap::<PersistentDispatchSlotId, &str>::new();
    for (_, declaration) in export.functions.iter() {
        let Some(method) = declaration.method else {
            continue;
        };
        match method.dispatch {
            hir::MethodDispatch::Virtual(family) | hir::MethodDispatch::FinalOverride(family) => {
                let root = export
                    .dispatch_slot_identities
                    .virtual_root(family)
                    .unwrap();
                slots.insert(
                    export.dispatch_slot_identities[family].id(),
                    &export.functions[root].name,
                );
            }
            hir::MethodDispatch::Interface(member) => {
                slots.insert(
                    export.dispatch_slot_identities[member].id(),
                    &declaration.name,
                );
            }
            hir::MethodDispatch::Direct => {}
        }
    }
    let mut result = String::new();
    for (name, exact) in &owners {
        result.push_str(&format!("{name}\n"));
        let mut schemas = inventory
            .get(*exact)
            .unwrap()
            .slot_schemas()
            .records()
            .iter()
            .map(|schema| {
                let role = match schema.role() {
                    hir::InheritanceSlotSchemaRoleV1::ClassVtable => "vtable",
                    hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                        reverse[&interface_exact]
                    }
                };
                (role, schema.slots())
            })
            .collect::<Vec<_>>();
        schemas.sort_by_key(|(role, _)| *role);
        for (role, sequence) in schemas {
            let names = sequence
                .iter()
                .map(|slot| slots[slot])
                .collect::<Vec<_>>()
                .join(", ");
            result.push_str(format!("  {role}: {names}").trim_end());
            result.push('\n');
        }
    }
    result
}
