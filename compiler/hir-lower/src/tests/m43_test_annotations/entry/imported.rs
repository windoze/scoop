use super::*;
use crate::tests::m23_ordinary_dependencies::support::{
    alias_expansions, project_parsed_dependency,
};
use crate::tests::m43_test_annotations::support::{marker_core, parsed_sources};

#[test]
fn generated_entry_consumes_the_execution_contract_through_typed_imports() {
    let mut core = marker_core();
    let runtime = parsed_sources(&[], true);
    let coordinate = scoop_identity::ConeCoordinate::new("scoop", "scoop.test", "0.1.0").unwrap();
    let (foundation, interface) = project_parsed_dependency(&core, &runtime, &[], false);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 184);
    let identity = crate::tests::test_source_identity("tests/cases.scoop");
    let sources = scoop_parser::parse_current_cone(scoop_ast::NonEmptyVec::new(
        scoop_parser::CurrentConeSourceInput::new(
            &identity,
            r#"
            import scoop.test.Test
            import scoop.test.BeforeTest
            import scoop.test.AfterTest
            @Test private class Cases(val seed: Int = 1) {
                @BeforeTest private fun prepare() {}
                @Test private fun check() {}
                @AfterTest private fun cleanup() {}
            }
        "#,
            "tests/cases.scoop".as_ref(),
        )
        .with_role(SourceRole::Test),
        Vec::new(),
    ))
    .unwrap();
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
        Vec::new(),
    )
    .unwrap();
    let input = crate::CurrentConeSources::try_new(
        &sources,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap()
    .with_test_dependencies([runtime.cone()])
    .unwrap();
    let output = crate::lower_current_tests(&input).unwrap_or_else(|errors| panic!("{errors:#?}"));
    assert_eq!(output.output().export.lambdas.len(), 4);
    let mut foundation = hir::CanonicalHirFoundation::from_type_semantics_output(&output).unwrap();
    hir::CoreBootstrapInterfaceSectionV1::from_export(&output.output().export).unwrap();
    let public = {
        let mut authority = hir::CrossConeHirProductionAuthority::new(
            &foundation,
            &output.output().export.public_export_bindings,
            &world,
        );
        hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(&output, &[], &mut authority)
            .unwrap()
    };
    foundation
        .complete_cross_cone_interface_source_points(output.output().export.module(), &public)
        .unwrap();
}
