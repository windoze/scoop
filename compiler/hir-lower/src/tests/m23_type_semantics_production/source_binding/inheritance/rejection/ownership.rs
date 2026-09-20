use super::*;

#[test]
fn inheritance_sources_cannot_mix_equal_artifacts_with_distinct_bindings() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let other = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let core = inputs.protocols().fundamental_types();
        let dispatch = sources
            .properties
            .dispatch
            .bind(&foundation, &mut meter())
            .unwrap();
        let other_dispatch = sources
            .properties
            .dispatch
            .bind(&other, &mut meter())
            .unwrap();
        let slots = dispatch.bind_slot_sources(core, &mut meter()).unwrap();
        let other_slots = other_dispatch
            .bind_slot_sources(core, &mut meter())
            .unwrap();
        let properties = sources.properties.bind(&foundation, &mut meter()).unwrap();
        let protected = properties
            .bind_protected_callable_sources(&sources.callables, core, &mut meter())
            .unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(
                &sources.properties.dispatch.inventory,
                &sources.constructors,
                &mut meter(),
            )
            .unwrap();
        let other_constructors = other
            .bind_inheritance_constructor_sources(
                &sources.properties.dispatch.inventory,
                &sources.constructors,
                &mut meter(),
            )
            .unwrap();
        let nominals = foundation
            .bind_nominal_sources(&sources.nominals, &mut meter())
            .unwrap();
        let other_nominals = other
            .bind_nominal_sources(&sources.nominals, &mut meter())
            .unwrap();
        for (constructors, nominals, slots) in [
            (&other_constructors, &nominals, &slots),
            (&constructors, &other_nominals, &slots),
            (&constructors, &nominals, &other_slots),
        ] {
            assert!(matches!(
                protected.bind_inheritance_sources(constructors, nominals, slots, &mut meter()),
                Err(Error::FoundationMismatch)
            ));
        }
    });
}
