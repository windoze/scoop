use super::*;

pub(super) fn public_interface(
    output: &hir::DependencyHirOutput,
) -> hir::CrossConeHirInterfaceSectionV1 {
    let export = output.output().export.module();
    let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
    let core = trusted_core();
    let world = core.world(export.cone);
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        &foundation,
        &export.public_export_bindings,
        &world,
    );
    hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(output, &[], &mut authority).unwrap()
}
