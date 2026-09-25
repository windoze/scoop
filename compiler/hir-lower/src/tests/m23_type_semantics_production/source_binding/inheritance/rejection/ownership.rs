use super::*;

#[test]
fn inheritance_sources_cannot_mix_equal_artifacts_with_distinct_bindings() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let other = fixture.bind().unwrap();

        let dispatch = sources.properties.dispatch.bind(&foundation).unwrap();
        let other_dispatch = sources.properties.dispatch.bind(&other).unwrap();
        let slots = dispatch.bind_slot_sources().unwrap();
        let other_slots = other_dispatch.bind_slot_sources().unwrap();
        let properties = sources.properties.bind(&foundation).unwrap();
        let protected = properties
            .bind_protected_callable_sources(&sources.callables)
            .unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(
                &sources.properties.dispatch.inventory,
                &sources.constructors,
            )
            .unwrap();
        let other_constructors = other
            .bind_inheritance_constructor_sources(
                &sources.properties.dispatch.inventory,
                &sources.constructors,
            )
            .unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let other_nominals = other.bind_nominal_sources(&sources.nominals).unwrap();
        for (constructors, nominals, slots) in [
            (&other_constructors, &nominals, &slots),
            (&constructors, &other_nominals, &slots),
            (&constructors, &nominals, &other_slots),
        ] {
            assert!(matches!(
                protected.bind_inheritance_sources(constructors, nominals, slots),
                Err(Error::FoundationMismatch)
            ));
        }
    });
}
