use super::*;

pub(in crate::tests::m23_type_semantics_production) fn with_source<T>(
    source: &str,
    run: impl FnOnce(&hir::DependencyHirOutput, &scoop_mir::Module) -> T,
) -> T {
    with_hir_source(source, |output, core| {
        let mut records = if output
            .output()
            .local
            .module()
            .initialization_units
            .is_empty()
        {
            Vec::new()
        } else {
            vec![core.project_initialization_cycle_to_mir()]
        };
        records.extend(
            output
                .executable_dependency_callables()
                .unwrap()
                .into_iter()
                .map(|use_| {
                    let selected = use_.callable();
                    let callable = selected.capability();
                    scoop_mir::SelectedDependencyMirCallableV1::try_new(
                        selected.provider(),
                        callable.direct_declaration().unwrap(),
                        callable.implementation(),
                        callable.signature().clone(),
                    )
                    .unwrap()
                }),
        );
        let dependencies = scoop_mir::SelectedExternalMirSet::try_from_callables(
            output.output().local.module().cone,
            records,
        )
        .unwrap();
        let mir = scoop_mir_lower::lower_current_cone(output, dependencies).unwrap();
        run(output, mir.module())
    })
}

pub(in crate::tests::m23_type_semantics_production) fn with_hir_source<T>(
    source: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    ) -> T,
) -> T {
    with_hir_source_at(source, "src/main.scoop", run)
}

pub(in crate::tests::m23_type_semantics_production) fn with_hir_source_at<T>(
    source: &str,
    path: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    ) -> T,
) -> T {
    with_hir_sources(&[(path, source)], run)
}

pub(in crate::tests::m23_type_semantics_production) fn with_hir_sources<T>(
    sources: &[(&str, &str)],
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    ) -> T,
) -> T {
    let core = trusted_core();
    let (first, rest) = sources.split_first().expect("nonempty fixture sources");
    let parsed = |(path, source): &(&str, &str)| {
        ast::IdentifiedParsedSource::new(
            test_source_identity(path),
            scoop_parser::parse(source).unwrap(),
        )
    };
    let text = |(path, source): &(&str, &str)| {
        ast::CurrentSourceText::new(test_source_identity(path), (*source).to_owned())
    };
    let context = |(path, _): &(&str, &str)| {
        ast::CurrentSourceDiagnosticContext::new(
            test_source_identity(path),
            std::path::PathBuf::from(path),
        )
    };
    let ordinary = ast::CurrentConeParsedSources::try_new(
        ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
            parsed(first),
            rest.iter().map(parsed).collect(),
        ))
        .unwrap(),
        ast::NonEmptyVec::new(text(first), rest.iter().map(text).collect()),
        ast::NonEmptyVec::new(context(first), rest.iter().map(context).collect()),
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    run(&output, &core)
}

pub(super) fn project(
    output: &hir::DependencyHirOutput,
) -> hir::CanonicalNominalInheritanceInterfacesV1 {
    produce_cross_cone_type_semantics(output, &public_interface(output))
        .unwrap()
        .inheritance()
        .clone()
}

pub(in crate::tests::m23_type_semantics_production) fn owners(
    output: &hir::DependencyHirOutput,
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

pub(super) fn owner(output: &hir::DependencyHirOutput, name: &str) -> PersistentExactTypeId {
    owners(output)[name]
}
