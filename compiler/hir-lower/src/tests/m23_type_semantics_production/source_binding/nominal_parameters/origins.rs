use super::*;

#[test]
fn complete_parameter_origins_cannot_borrow_valid_points_from_another_source_file() {
    let other = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/parameters-binding-other.scoop"
    ));
    super::super::super::source_dispatch::with_hir_sources(
        &[("src/main.scoop", SOURCE), ("src/other.scoop", other)],
        |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
            sources.with_bound(
                &foundation,
                inputs.protocols().fundamental_types(),
                |members, constructors| {
                    members
                        .bind_parameter_protocols(constructors, &sources.protocols)
                        .unwrap();
                    for record in sources
                        .protocols
                        .records()
                        .iter()
                        .filter(|r| !r.parameters().is_empty())
                    {
                        let first = &record.parameters()[0];
                        let other = sources
                            .protocols
                            .records()
                            .iter()
                            .flat_map(|r| r.parameters())
                            .map(|p| p.definition_origin())
                            .find(|origin| {
                                origin.origin().source()
                                    != first.definition_origin().origin().source()
                            })
                            .unwrap();
                        let mut parameters = record.parameters().to_vec();
                        parameters[0] = hir::InheritanceSourceParameterV1::new(
                            first.shape().clone(),
                            first.calling_kind(),
                            other.clone(),
                        );
                        let forged =
                            sources.replacing(Record::try_new(record.owner(), parameters).unwrap());
                        let Error::Contract(error) = members
                            .bind_parameter_protocols(constructors, &forged)
                            .unwrap_err()
                        else {
                            panic!("source file rejection");
                        };
                        let ContractError::Origin { owner, position: 0 } = *error else {
                            panic!("origin belongs to another source file");
                        };
                        assert_eq!(owner, record.owner());
                    }
                },
            );
        },
    );
}
