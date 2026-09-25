use super::*;

#[test]
fn complete_constructor_binding_rejects_missing_and_extra_sources() {
    let extra = with_source(
        "public class Unrelated public constructor()",
        |output, _| {
            let mut fixture = Fixture::from_output(output);
            Sources::from_output(output, &mut fixture)
                .constructors
                .records()[0]
                .clone()
        },
    );
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let mut missing = sources.constructors.records().to_vec();
        missing.pop().unwrap();
        let mut surplus = sources.constructors.records().to_vec();
        surplus.push(extra.clone());
        for records in [vec![], missing, surplus] {
            let forged = Table::try_new(records).unwrap();
            assert!(matches!(
                nominals.bind_constructor_sources(&forged),
                Err(Error::Inventory)
            ));
        }
    });
}

#[test]
fn complete_constructor_binding_cannot_use_dependency_only_keys() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_constructors(vec![]).unwrap();
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities)
            .unwrap();
        // The prerequisite nominal inventory already refuses to publish a
        // proof when its constructor keys only exist in the dependency graph.
        assert!(matches!(
            foundation.bind_nominal_sources(&sources.nominals),
            Err(hir::NominalSourceBindingError::Inventory(
                "nominal constructors"
            ))
        ));
    });
}

#[test]
fn complete_constructor_binding_rejects_owner_and_lexical_chain_changes() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let record = sources.named(&fixture, "Box", &["value"]);
        let payload = record.payload();
        let other = sources
            .named(&fixture, "Pair", &["value"])
            .payload()
            .owner();
        let mut owners = record.declaration_access().lexical_owners().to_vec();
        owners.remove(0);
        let wrong_chain = Record::try_new(
            record.declaration(),
            hir::DeclarationAccessSourceV1::try_new(
                record.declaration_access().declared_visibility(),
                owners,
                record.declaration_access().definition_origin().clone(),
            )
            .unwrap(),
            payload.clone(),
        )
        .unwrap();
        let wrong_owner = with_payload(
            record,
            other,
            payload.parameters().clone(),
            payload.result().clone(),
            payload.effects(),
        );
        for record in [wrong_owner, wrong_chain] {
            let mut forged = sources.clone();
            forged.replace(record);
            assert!(matches!(
                nominals.bind_constructor_sources(&forged.constructors),
                Err(Error::Contract { .. })
            ));
        }
    });
}
