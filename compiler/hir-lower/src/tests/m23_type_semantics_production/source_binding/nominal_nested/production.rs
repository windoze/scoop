use super::*;
use hir::NestedNominalSourceProductionV1 as Production;

pub(super) fn class(export: &hir::ExportHirOutput, name: &str) -> hir::SourceNominalId {
    let id = export
        .classes
        .iter()
        .find(|(_, c)| c.name == name)
        .unwrap()
        .0;
    hir::SourceNominalId::from_source_declaration(
        export.nominal_identities[id]
            .source()
            .unwrap()
            .declaration(),
    )
    .unwrap()
}

#[test]
fn nested_production_rejects_top_level_and_absent_roots() {
    with_hir_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let root = class(export, "Envelope");
        assert!(matches!(
            Production::from_export_hir(export, root),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
        ));
        with_hir_source(
            &SOURCE.replace("Envelope", "AnotherEnvelope"),
            |other, _| {
                let absent = class(&other.output().export, "Holder");
                assert!(matches!(
                    Production::from_export_hir(export, absent),
                    Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
                ));
            },
        );
    });
}

#[test]
fn nested_production_rejects_missing_or_inconsistent_independent_sources() {
    with_hir_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let root = class(export, "Holder");
        let index = export
            .source_parameter_interfaces
            .iter()
            .position(|interface| {
                matches!(interface.owner, hir::ExportParameterOwner::Function(id)
                if export.functions[id].name.ends_with(".take"))
            })
            .unwrap();
        for corruption in ["missing", "duplicate", "arity", "type", "default", "origin"] {
            let mut module = export.clone().into_module();
            match corruption {
                "missing" => {
                    module.source_parameter_interfaces.remove(index);
                }
                "duplicate" => module
                    .source_parameter_interfaces
                    .push(module.source_parameter_interfaces[index].clone()),
                "arity" => {
                    module.source_parameter_interfaces[index]
                        .parameters
                        .pop()
                        .unwrap();
                }
                "type" => {
                    module.source_parameter_interfaces[index].parameters[0].calling =
                        hir::ExportParameterCalling::Required {
                            value_type: module.unit,
                        }
                }
                "default" => module.export_default_sources = Default::default(),
                "origin" => module.export_definition_origins = Default::default(),
                _ => unreachable!(),
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            let error = Production::from_export_hir(&forged, root).unwrap_err();
            assert!(
                matches!(
                    error,
                    hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_)
                        | hir::CrossConeTypeSemanticsProductionError::MissingDefinitionOrigin(_)
                ),
                "{corruption}: {error:?}"
            );
        }
    });
}
