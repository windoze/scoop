use super::*;

#[test]
fn protected_production_rejects_missing_source_and_budget_exhaustion() {
    with_hir_source(SOURCE, |output, _| {
        let export = &output.output().export;
        for field in ["origins", "parameters", "defaults"] {
            let mut module = export.clone().into_module();
            match field {
                "origins" => module.export_definition_origins = Default::default(),
                "parameters" => module.source_parameter_interfaces.clear(),
                "defaults" => module.export_default_sources = Default::default(),
                _ => unreachable!(),
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            assert!(
                Production::from_export_hir(&forged, &mut meter()).is_err(),
                "{field}"
            );
        }
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                Production::from_export_hir(export, &mut BudgetMeter::new(limits)).is_err(),
                "{limits:?}"
            );
        }
    });
}

#[test]
fn protected_wrappers_reject_other_declared_visibilities() {
    with_hir_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut rejected = [0; 2];
        for source in sources.constructors.records() {
            if source.declaration_access().declared_visibility()
                != hir::DeclaredVisibilityV1::Protected
            {
                assert!(hir::ProtectedConstructorInterfaceV1::try_from(source.clone()).is_err());
                rejected[0] += 1;
            }
        }
        for source in sources.members.properties.records() {
            if source.declaration_access().declared_visibility()
                != hir::DeclaredVisibilityV1::Protected
            {
                assert!(hir::ProtectedPropertyInterfaceV1::try_from(source.clone()).is_err());
                rejected[1] += 1;
            }
        }
        assert!(rejected.into_iter().all(|count| count > 0));
    });
}

#[test]
fn protected_production_has_an_explicit_empty_surface() {
    with_hir_source("public class Plain {}", |output, _| {
        let produced = Production::from_export_hir(&output.output().export, &mut meter()).unwrap();
        assert!(produced.required().values().is_empty());
        assert!(produced.declarations().records().is_empty());
        assert!(produced.protocols().records().is_empty());
    });
}
