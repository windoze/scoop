use scoop_hir as hir;
use scoop_identity::ConeCoordinate;

use super::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use super::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use crate::{CurrentConeSources, lower_current_cone};

mod abstracts;
mod arrays;
mod bound_properties;
mod bounds;
mod callable_signatures;
mod classes;
mod concrete_calls;
mod constructor_applications;
mod constructor_requests;
mod constructors;
mod contextual;
mod delegates;
mod equality;
mod host_properties;
mod interface_members;
mod interfaces;
mod local_calls;
mod machine;
mod metadata;
mod method_calls;
mod native_calls;
mod nominal_conditions;
mod nominals;
mod options;
mod parents;
mod pointer_construction;
mod pointers;
mod qualified_types;
mod reference_targets;
mod references;
mod requests;
mod selection;
mod shared_closures;
mod shared_defaults;
mod shared_singletons;
mod source_calls;
mod structs;
mod value_layouts;
mod wire_equivalence;

const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-consumption/provider.scoop"
));
const CONSUMER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-consumption/consumer.scoop"
));

fn lower_consumer(source: &str) -> Result<hir::DependencyHirOutput, Vec<crate::Diagnostic>> {
    with_consumer(source, |output, _, _, _, _| output)
}

fn with_consumer<T>(
    source: &str,
    verify: impl FnOnce(
        hir::DependencyHirOutput,
        &hir::ImportedSemanticWorld,
        &hir::CanonicalHirFoundation,
        &hir::CrossConeHirInterfaceSectionV1,
        &super::m23_ordinary_core_only::support::TrustedCoreFixture,
    ) -> T,
) -> Result<T, Vec<crate::Diagnostic>> {
    with_provider_consumer(PROVIDER, source, verify)
}

fn with_provider_consumer<T>(
    provider: &str,
    source: &str,
    verify: impl FnOnce(
        hir::DependencyHirOutput,
        &hir::ImportedSemanticWorld,
        &hir::CanonicalHirFoundation,
        &hir::CrossConeHirInterfaceSectionV1,
        &super::m23_ordinary_core_only::support::TrustedCoreFixture,
    ) -> T,
) -> Result<T, Vec<crate::Diagnostic>> {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
    let (foundation, interface) =
        project_dependency_text(&core, &coordinate, provider, &["Boolean"]);
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 73);
    let aliases = alias_expansions(interface.type_aliases());
    let consumer = parsed_ordinary_text(source);
    let world = hir::ImportedSemanticWorld::from_dependencies(
        consumer.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &consumer,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)?;
    Ok(verify(output, &world, &foundation, &interface, &core))
}

#[test]
fn imported_generic_bodies_infer_and_materialize_provider_templates() {
    let output =
        lower_consumer(CONSUMER).expect("source-selected dependency templates must instantiate");
    let export = output.output().export.module();
    let local = output.output().local.module();
    assert!(export.functions.iter().all(|(_, function)| {
        !["identity", "choose", "helper", "echo"].contains(&function.name.as_str())
    }));
    assert_eq!(
        export
            .imported_generic_templates
            .iter()
            .filter(|(_, template)| {
                export.source_files[template.origin.file as usize]
                    .identity
                    .cone()
                    != scoop_identity::ConeIdentity::CORE
            })
            .count(),
        8
    );
    for name in [
        "identity",
        "choose",
        "helper",
        "echo",
        "defaulted",
        "nativeIdentity",
        "recursive",
        "pointerIdentity",
    ] {
        let functions = local
            .functions
            .iter()
            .filter(|(_, function)| function.name == name)
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        assert_eq!(
            functions.len(),
            if name == "identity" || name == "defaulted" {
                2
            } else {
                1
            },
            "{name}"
        );
        assert!(
            functions
                .iter()
                .all(|function| matches!(function.kind, hir::concrete::FunctionKind::User(_)))
        );
    }
    let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output)
        .expect("generic applications retain their actual foundation identities");
    let _ = scoop_wire::encode(&foundation).unwrap();
    let dependencies =
        scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new()).unwrap();
    scoop_mir_lower::lower_current_cone(&output, dependencies)
        .expect("dependency templates lower through the ordinary MIR body path");
}

#[test]
fn imported_generic_overloads_compare_declarations_in_one_type_arena() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-body-consumption/overload.scoop"
    ));
    let output = lower_consumer(source).expect("tuple input selects the tuple declaration");
    let export = output.output().export.module();
    let selected = export
        .imported_generic_templates
        .iter()
        .filter(|(_, template)| {
            export.source_files[template.origin.file as usize]
                .identity
                .cone()
                != scoop_identity::ConeIdentity::CORE
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected.len(),
        1,
        "the losing candidate must not commit a template"
    );
    let (_, selected) = selected[0];
    assert_eq!(selected.type_parameters.len(), 2);
    assert!(matches!(
        export.types[selected.params[0].ty],
        hir::Type::Tuple(_)
    ));
}
