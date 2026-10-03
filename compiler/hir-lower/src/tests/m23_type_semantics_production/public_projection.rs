use super::*;

pub(super) fn public_interface(
    output: &hir::DependencyHirOutput,
) -> hir::CrossConeHirInterfaceSectionV1 {
    public_interface_with_core(output, &trusted_core())
}

pub(super) fn public_interface_with_core(
    output: &hir::DependencyHirOutput,
    core: &super::super::m23_ordinary_core_only::support::TrustedCoreFixture,
) -> hir::CrossConeHirInterfaceSectionV1 {
    let export = output.output().export.module();
    let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
    let world = core.world(export.cone);
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        &foundation,
        &export.public_export_bindings,
        &world,
    );
    hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(output, &[], &mut authority).unwrap()
}
