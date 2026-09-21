pub(super) fn mir(
    input: &super::super::CurrentConeMachineHir<'_>,
) -> scoop_mir::SelectedExternalMirSet {
    scoop_mir::SelectedExternalMirSet::try_from_callables(
        input.output.output().export.cone,
        input
            .output
            .imported_dependencies()
            .callables()
            .map(|selected| {
                let capability = selected.capability();
                scoop_mir::SelectedDependencyMirCallableV1::try_new(
                    selected.provider(),
                    capability.declaration(),
                    capability.implementation(),
                    capability.signature().clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

pub(super) fn lir(
    selected: &scoop_mir::SelectedExternalMirSet,
) -> scoop_lir::SelectedExternalLirSet {
    scoop_lir::SelectedExternalLirSet::try_from_callables(
        selected.consumer(),
        selected
            .dependency_callables()
            .map(|callable| {
                scoop_lir::SelectedDependencyLirCallableV1::new(
                    callable.provider(),
                    callable.declaration(),
                    callable.implementation(),
                    scoop_identity::CanonicalScoopAbiFunctionSignature::new(
                        callable.signature().clone(),
                        Vec::new(),
                        scoop_identity::ScoopAbiReturn::unit_void(),
                        scoop_identity::GcEffect::Managed,
                    )
                    .unwrap(),
                    scoop_lir::CallingConvention::Cdecl,
                    scoop_lir::ExternalCallableRootPlan::ManagedStatepoint,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
