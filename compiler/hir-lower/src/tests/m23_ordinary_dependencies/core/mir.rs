use scoop_hir::DependencyHirOutput;
use scoop_mir::{CurrentMirProtocolDeclarations, SelectedDependencyMirSet, SelectedImportedMirSet};
use scoop_mir_lower::{CurrentConeMirLoweringError, lower_current_cone};

pub(super) fn check(output: &DependencyHirOutput, imported: SelectedImportedMirSet<'_>) {
    let wrong_origin = lower_current_cone(output, imported, selected(output))
        .err()
        .expect("a local protocol cannot use an imported protocol selection");
    assert!(matches!(
        wrong_origin,
        CurrentConeMirLoweringError::ProtocolOriginMismatch
    ));
    let missing = lower_current_cone(
        output,
        CurrentMirProtocolDeclarations,
        SelectedDependencyMirSet::empty(output.output().export.cone),
    )
    .err()
    .expect("a selected HIR dependency must retain its MIR projection");
    assert!(matches!(
        missing,
        CurrentConeMirLoweringError::MissingDependencyMirCallable { index: 0 }
    ));

    let mir = lower_current_cone(output, CurrentMirProtocolDeclarations, selected(output))
        .expect("local protocols and dependency calls share MIR lowering");
    assert_eq!(mir.imported_dependencies().len(), 1);
    assert_eq!(mir.module().meta.external_callables.len(), 1);
    for (_, callable) in mir.module().meta.external_callables.iter() {
        let scoop_mir::ExternalCallableSelection::Dependency(reference) = callable.selection()
        else {
            panic!("ordinary dependencies retain their selection role")
        };
        let declaration = mir
            .imported_dependencies()
            .resolve_callable(reference)
            .expect("the output owns the selection that minted each callable use");
        assert_eq!(
            declaration.declaration(),
            output
                .imported_dependencies()
                .callables()
                .next()
                .unwrap()
                .capability()
                .declaration()
        );
    }
    super::assert_core_snapshot("mir", &scoop_mir::dump(mir.module()));
}

fn selected(output: &DependencyHirOutput) -> SelectedDependencyMirSet {
    SelectedDependencyMirSet::try_from_callables(
        output.output().export.cone,
        output
            .imported_dependencies()
            .callables()
            .map(|selected| {
                let capability = selected.capability();
                let declaration = capability.declaration();
                let scoop_identity::DependencyCallableDeclarationId::Function(function) =
                    declaration
                else {
                    panic!("the fixture exports a source function")
                };
                scoop_mir::SelectedDependencyMirCallableV1::try_new(
                    selected.provider(),
                    declaration,
                    scoop_identity::StrongCallableDefinitionOwner::Function(function),
                    capability.signature().clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
