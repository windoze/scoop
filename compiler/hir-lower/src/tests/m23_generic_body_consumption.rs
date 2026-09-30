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
mod constructors;
mod contextual;
mod delegates;
mod equality;
mod interface_members;
mod interfaces;
mod machine;
mod members;
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
mod references;
mod requests;
mod selection;
mod shared_defaults;
mod source_calls;
mod structs;
mod value_layouts;

const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-consumption/provider.scoop"
));
const CONSUMER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-consumption/consumer.scoop"
));
const BAD_KIND: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-body-consumption/bad-kind.scoop"
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
    assert_eq!(export.imported_generic_templates.len(), 8);
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
    let mir = scoop_mir_lower::lower_current_cone(&output, dependencies)
        .expect("dependency templates lower through the ordinary MIR body path");
    let mir_dump = scoop_mir::dump(mir.module());
    let mir_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-body-consumption/consumer.mir.snap");
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&mir_path, &mir_dump).unwrap();
    }
    assert_eq!(std::fs::read_to_string(mir_path).unwrap(), mir_dump);
    let dump = hir::dump(&output.output().export);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-body-consumption/consumer.hir.snap");
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&path, &dump).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), dump);
}

#[test]
fn imported_generic_kind_bound_reports_the_consumer_argument() {
    let errors = lower_consumer(BAD_KIND)
        .err()
        .expect("a class cannot satisfy a value bound");
    let error = errors
        .iter()
        .find(|error| error.message.contains("must satisfy `value`"))
        .expect("the normal kind constraint reports the failure");
    let span = error.span.unwrap();
    assert_eq!(
        &BAD_KIND[span.start as usize..span.end as usize],
        "valueOnly(ReferenceValue())"
    );
}

#[test]
fn imported_generic_overloads_compare_declarations_in_one_type_arena() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-body-consumption/overload.scoop"
    ));
    let output = lower_consumer(source).expect("tuple input selects the tuple declaration");
    let export = output.output().export.module();
    assert_eq!(
        export.imported_generic_templates.len(),
        1,
        "the losing candidate must not commit a template"
    );
    let (_, selected) = export.imported_generic_templates.iter().next().unwrap();
    assert_eq!(selected.type_parameters.len(), 2);
    assert!(matches!(
        export.types[selected.params[0].ty],
        hir::Type::Tuple(_)
    ));
}

#[test]
fn imported_generic_ambiguity_reports_both_declared_signatures() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-body-consumption/bad-overload.scoop"
    ));
    let errors = lower_consumer(source)
        .err()
        .expect("neither declaration dominates");
    let error = errors
        .iter()
        .find(|error| error.message.contains("ambiguous"))
        .unwrap_or_else(|| panic!("missing ambiguity: {errors:?}"));
    assert!(
        error.message.contains("conflict<T>(left: T, right: Int)"),
        "{error:?}"
    );
    assert!(
        error.message.contains("conflict<T>(left: Int, right: T)"),
        "{error:?}"
    );
    let span = error.span.unwrap();
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        "conflict(1, 2)"
    );
    assert_eq!(error.file, 0);
}

#[test]
fn imported_generic_effects_and_pointee_predicates_reach_consumer_calls() {
    for (source, message) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-generic-body-consumption/bad-nogc.scoop"
            )),
            "calling a managed dependency function",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-generic-body-consumption/bad-nogc-argument.scoop"
            )),
            "GC-free",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-generic-body-consumption/bad-pointee.scoop"
            )),
            "GC-free `Ptr` pointee",
        ),
    ] {
        let errors = lower_consumer(source)
            .err()
            .expect("the source violates the imported callable contract");
        let error = errors
            .iter()
            .find(|error| error.message.contains(message))
            .unwrap_or_else(|| panic!("missing {message}: {errors:?}"));
        let span = error.span.unwrap();
        assert!(
            source[span.start as usize..span.end as usize].contains('('),
            "{error:?}"
        );
        assert_eq!(error.file, 0);
    }
}
