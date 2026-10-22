use super::*;
use crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text;

pub(super) fn with_provider(
    provider: &str,
    consumer: &str,
    generator: &mut dyn hir::MethodBodyGenerator,
) -> hir::DependencyHirOutput {
    with_core_provider(trusted_core(), provider, consumer, generator)
}

pub(super) fn with_core_provider(
    mut core: crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
    provider: &str,
    consumer: &str,
    generator: &mut dyn hir::MethodBodyGenerator,
) -> hir::DependencyHirOutput {
    let coordinate = ConeCoordinate::new("test", "query-provider", "1.0.0").unwrap();
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 184);
    let sources = parsed_ordinary_text(consumer);
    let world = hir::ImportedSemanticWorld::from_dependencies(
        sources.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        vec![],
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = crate::CurrentConeSources::try_new(&sources, core_inputs, &world).unwrap();
    crate::lower_current_cone_with_generator(
        scoop_identity::RequestedConeKind::Library,
        &input,
        generator,
    )
    .unwrap()
}
