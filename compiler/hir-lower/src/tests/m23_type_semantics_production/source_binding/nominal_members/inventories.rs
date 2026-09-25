use super::*;

#[test]
fn nominal_member_binding_rejects_incomplete_properties_methods_accessors_and_variants() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        for index in 0..sources.properties.records().len() {
            let mut records = sources.properties.records().to_vec();
            records.remove(index);
            let table = Properties::try_new(records).unwrap();
            assert!(matches!(
                nominals.bind_member_sources(&table, &sources.callables, core),
                Err(Error::Inventory("properties"))
            ));
        }
        for index in 0..sources.callables.records().len() {
            let mut records = sources.callables.records().to_vec();
            records.remove(index);
            let table = Callables::try_new(records).unwrap();
            assert!(matches!(
                nominals.bind_member_sources(&sources.properties, &table, core),
                Err(Error::Inventory("callables"))
            ));
        }
    });
}

#[test]
fn nominal_member_binding_requires_artifact_owned_keys_before_source_queries() {
    with_sources(SOURCE, |_, fixture, sources, _| {
        for kind in ["functions", "generic functions", "properties"] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            match kind {
                "functions" => canonical.set_functions(vec![]).unwrap(),
                "generic functions" => canonical.set_generic_functions(vec![]).unwrap(),
                "properties" => canonical.set_properties(vec![]).unwrap(),
                _ => unreachable!(),
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities)
                .unwrap();
            assert!(
                foundation.bind_nominal_sources(&sources.nominals).is_err(),
                "{kind}"
            );
        }
    });
}

#[test]
fn nominal_member_binding_rejects_extra_properties_and_callables() {
    let (property, callable) = with_hir_source(
        "public class Extra { public val extra: Int = 1 }",
        |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            (
                sources.properties.records()[0].clone(),
                sources.callables.records()[0].clone(),
            )
        },
    );
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let mut properties = sources.properties.records().to_vec();
        properties.push(property);
        let properties = Properties::try_new(properties).unwrap();
        assert!(matches!(
            nominals.bind_member_sources(&properties, &sources.callables, core),
            Err(Error::Inventory("properties"))
        ));
        let mut callables = sources.callables.records().to_vec();
        callables.push(callable);
        let callables = Callables::try_new(callables).unwrap();
        assert!(matches!(
            nominals.bind_member_sources(&sources.properties, &callables, core),
            Err(Error::Inventory("callables"))
        ));
    });
}
