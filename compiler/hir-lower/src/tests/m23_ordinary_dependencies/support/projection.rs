use scoop_identity::ConeCoordinate;

use super::super::super::m23_ordinary_core_only::support::{
    TrustedCoreFixture, parsed_ordinary_at, parsed_ordinary_text_at,
};
use crate::{CurrentConeSources, lower_current_cone};

pub(crate) fn project_dependency(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    default_core_types: &[&str],
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    project_dependency_with_core_roles(core, coordinate, source, default_core_types, true)
}

pub(crate) fn project_dependency_without_default_roles(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    core_types: &[&str],
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    project_dependency_with_core_roles(core, coordinate, source, core_types, false)
}

fn project_dependency_with_core_roles(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: scoop_ast::SourceFile,
    core_types: &[&str],
    include_default_role: bool,
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    let parsed = parsed_ordinary_at(coordinate, source);
    project_parsed_dependency(core, &parsed, core_types, include_default_role)
}

pub(crate) fn project_dependency_text(
    core: &TrustedCoreFixture,
    coordinate: &ConeCoordinate,
    source: &str,
    core_types: &[&str],
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    let parsed = parsed_ordinary_text_at(coordinate, source);
    project_parsed_dependency(core, &parsed, core_types, false)
}

fn project_parsed_dependency(
    core: &TrustedCoreFixture,
    parsed: &scoop_ast::CurrentConeParsedSources,
    core_types: &[&str],
    include_default_role: bool,
) -> (
    scoop_hir::CanonicalHirFoundation,
    scoop_hir::CrossConeHirInterfaceSectionV1,
) {
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let witnesses = core_types
        .iter()
        .flat_map(|name| core_type_witnesses(core, name, include_default_role))
        .collect::<Vec<_>>();
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(parsed, core_inputs, &world).unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .expect("the dependency provider must lower before interface projection");
    let mut foundation =
        scoop_hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
    let mut authority = scoop_hir::CrossConeHirProductionAuthority::new(
        &foundation,
        &output.output().export.module().public_export_bindings,
        &world,
    );
    let interface = scoop_hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
        &output,
        &witnesses,
        &mut authority,
    )
    .unwrap();
    foundation
        .complete_cross_cone_interface_source_points(output.output().export.module(), &interface)
        .unwrap();
    (foundation, interface)
}

fn core_type_witnesses(
    core: &TrustedCoreFixture,
    name: &str,
    include_default_role: bool,
) -> Vec<scoop_hir::ExternalHirBindingWitnessUse> {
    let binding = core.type_binding(name);
    let scoop_hir::ImportedTarget::Type(declaration) = binding.target() else {
        panic!("the {name} prelude binding must target a concrete nominal")
    };
    let target = scoop_hir::ExternalHirTargetV1::Nominal(
        scoop_identity::NominalDeclarationOwner::Concrete(declaration.persistent()),
    );
    let mut witnesses = Vec::new();
    for source in binding.sources() {
        if include_default_role {
            witnesses.push(scoop_hir::ExternalHirBindingWitnessUse::new(
                target,
                scoop_hir::ExternalHirBindingWitnessRole::DefaultDependency,
                source.clone(),
            ));
        }
        witnesses.push(scoop_hir::ExternalHirBindingWitnessUse::new(
            target,
            scoop_hir::ExternalHirBindingWitnessRole::ConcreteSelectedUse,
            source.clone(),
        ));
    }
    witnesses
}
