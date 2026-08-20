use scoop_hir::DependencyHirOutput;
use scoop_mir::SelectedExternalMirSet;
use scoop_mir_lower::{CurrentConeMirLoweringError, lower_current_cone};

pub(super) fn check(output: &DependencyHirOutput) {
    let missing = lower_current_cone(
        output,
        SelectedExternalMirSet::empty(output.output().export.cone),
        Default::default(),
    )
    .err()
    .expect("a selected HIR dependency must retain its MIR projection");
    assert!(matches!(
        missing,
        CurrentConeMirLoweringError::MissingDependencyMirCallable { index: 0 }
    ));

    let mir = lower_current_cone(output, selected(output), Default::default())
        .expect("local protocols and dependency calls share MIR lowering");
    assert_eq!(mir.selected_callables().len(), 1);
    assert_eq!(mir.module().meta.external_callables.len(), 1);
    for (_, callable) in mir.module().meta.external_callables.iter() {
        let reference = callable.reference();
        let declaration = mir
            .selected_callables()
            .resolve_callable(reference)
            .expect("the output owns the selection that minted each callable use");
        assert_eq!(
            declaration.implementation(),
            output
                .imported_dependencies()
                .callables()
                .next()
                .unwrap()
                .capability()
                .implementation()
        );
    }
}

fn selected(output: &DependencyHirOutput) -> SelectedExternalMirSet {
    SelectedExternalMirSet::try_from_callables(
        output.output().export.cone,
        output
            .imported_dependencies()
            .callables()
            .map(|selected| {
                let capability = selected.capability();
                let declaration = capability.direct_declaration().unwrap();
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
