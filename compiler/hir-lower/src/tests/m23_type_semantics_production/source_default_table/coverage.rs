use super::*;
#[test]
fn source_coverage_rejects_missing_extra_and_reclassified_default_positions() {
    with_hir_source(SOURCE, |output, _| {
        let production = Production::from_ordinary_hir(output, &mut meter()).unwrap();
        let table = production.templates();
        let missing = table.records()[0].key();
        let subset = Table::try_new(table.records()[1..].to_vec(), &mut meter()).unwrap();
        assert!(
            matches!(subset.validate_parameter_coverage(production.parameters(), &mut meter()), Err(hir::DefaultSourceTemplateCoverageError::Missing(key)) if key == missing)
        );
        let extra = hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
            output,
            function(output.output().export.module(), "topLevel"),
            0,
            &mut meter(),
        )
        .unwrap()
        .into_source_template(&mut meter())
        .unwrap();
        let extra_key = extra.key();
        let mut records = table.records().to_vec();
        records.push(extra);
        let extra = Table::try_new(records, &mut meter()).unwrap();
        assert!(
            matches!(extra.validate_parameter_coverage(production.parameters(), &mut meter()), Err(hir::DefaultSourceTemplateCoverageError::Extra(key)) if key == extra_key)
        );
        let mut protocols = production.parameters().records().to_vec();
        let index = protocols
            .iter()
            .position(|r| r.owner() == missing.owner())
            .unwrap();
        let protocol = &protocols[index];
        let mut parameters = protocol.parameters().to_vec();
        let parameter = &parameters[missing.parameter_position() as usize];
        parameters[missing.parameter_position() as usize] = hir::InheritanceSourceParameterV1::new(
            parameter.shape().clone(),
            hir::ProtectedParameterCallingKindV1::Required,
            parameter.definition_origin().clone(),
        );
        protocols[index] = hir::NominalSourceParameterProtocolV1::try_new(
            protocol.owner(),
            parameters,
            &mut meter(),
        )
        .unwrap();
        let protocols =
            hir::CanonicalNominalSourceParameterProtocolsV1::try_new(protocols, &mut meter())
                .unwrap();
        assert!(
            matches!(table.validate_parameter_coverage(&protocols, &mut meter()), Err(hir::DefaultSourceTemplateCoverageError::Extra(key)) if key == missing)
        );
    });
}
