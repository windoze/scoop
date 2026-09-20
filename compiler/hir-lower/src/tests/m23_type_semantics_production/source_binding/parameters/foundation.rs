use super::*;

#[test]
fn generic_parameter_source_points_survive_public_interface_completion() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let protocols = Table::from_ordinary_hir(output, &mut meter()).unwrap();
        let generic = protocols
            .records()
            .iter()
            .find(|record| matches!(record.owner(), CallableTemplateOrigin::GenericFunction(_)))
            .unwrap();
        let origin = generic.parameters()[0].definition_origin().origin();
        let legacy = hir::CanonicalHirFoundation::from_ordinary_output(output).unwrap();
        let legacy = hir::OdrFreeHirFoundation::try_new(legacy).unwrap();
        assert!(
            legacy
                .source_record(origin.source())
                .unwrap()
                .require_points([origin.span().start_byte(), origin.span().end_byte(),])
                .is_err()
        );
        let mut completed =
            hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
        completed
            .complete_cross_cone_source_points(
                export,
                &hir::CanonicalExportDefinitionSourcesV1::default(),
            )
            .unwrap();
        let completed = hir::OdrFreeHirFoundation::try_new(completed).unwrap();
        for parameter in protocols
            .records()
            .iter()
            .flat_map(|record| record.parameters())
        {
            let origin = parameter.definition_origin().origin();
            completed
                .source_record(origin.source())
                .unwrap()
                .require_points([origin.span().start_byte(), origin.span().end_byte()])
                .unwrap();
        }
    });
}
