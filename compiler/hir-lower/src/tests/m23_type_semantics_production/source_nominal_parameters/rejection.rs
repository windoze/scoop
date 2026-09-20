use super::*;

#[test]
fn nominal_variant_parameter_projection_rejects_missing_and_inconsistent_sources() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let required = required(export);
        let index = export
            .source_parameter_interfaces
            .iter()
            .position(|interface| {
                matches!(
                    interface.owner,
                    hir::ExportParameterOwner::VariantConstructor(_)
                ) && interface.parameters.len() == 2
            })
            .unwrap();
        for corruption in ["missing", "duplicate", "arity", "type", "default"] {
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
                _ => unreachable!(),
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            assert!(
                matches!(
                    Table::from_export_hir(&forged, &required, &mut meter()),
                    Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
                ),
                "{corruption}"
            );
        }
    });
}

#[test]
fn nominal_parameter_sources_reject_accessors_and_top_level_functions() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let property = export.properties.iter().next().unwrap().1;
        let accessor = CallableTemplateOrigin::Accessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let function = export
            .functions
            .iter()
            .find(|(_, f)| f.name == "topLevel")
            .unwrap()
            .0;
        let top = declaration(export, hir::ExportParameterOwner::Function(function)).unwrap();
        for id in [accessor, top] {
            assert!(Table::from_export_hir(export, &BTreeSet::from([id]), &mut meter()).is_err());
        }
        assert!(Record::try_new(accessor, vec![], &mut meter()).is_err());
    });
}

#[test]
fn nominal_parameter_protocols_cannot_claim_duplicate_names_or_multiple_varargs() {
    with_source(SOURCE, |output, _| {
        let table = table(&output.output().export);
        let record = table
            .records()
            .iter()
            .find(|r| r.parameters().len() == 2)
            .unwrap();
        let duplicate = vec![record.parameters()[0].clone(); 2];
        let varargs = record
            .parameters()
            .iter()
            .map(|p| {
                hir::InheritanceSourceParameterV1::new(
                    p.shape().clone(),
                    hir::ProtectedParameterCallingKindV1::VarargEmpty,
                    p.definition_origin().clone(),
                )
            })
            .collect();
        for parameters in [duplicate, varargs] {
            assert!(Record::try_new(record.owner(), parameters, &mut meter()).is_err());
        }
    });
}
