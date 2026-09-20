use super::*;

#[test]
fn complete_protocol_inventory_rejects_every_missing_record_and_unrelated_extra() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            for index in 0..sources.protocols.records().len() {
                let mut records = sources.protocols.records().to_vec();
                records.remove(index);
                let forged = Table::try_new(records, &mut meter()).unwrap();
                assert!(matches!(
                    members.bind_parameter_protocols(constructors, &forged, &mut meter()),
                    Err(Error::Inventory)
                ));
            }
            let unused = output
                .output()
                .export
                .function_identities
                .iter()
                .find_map(|(_, identity)| {
                    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(
                        record,
                    )) = identity
                    else {
                        return None;
                    };
                    let owner = CallableTemplateOrigin::Function(record.id());
                    sources.protocols.get(owner).is_none().then_some(owner)
                })
                .unwrap();
            let mut records = sources.protocols.records().to_vec();
            records.push(Record::try_new(unused, vec![], &mut meter()).unwrap());
            let forged = Table::try_new(records, &mut meter()).unwrap();
            assert!(matches!(
                members.bind_parameter_protocols(constructors, &forged, &mut meter()),
                Err(Error::Inventory)
            ));
            let bound = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let Error::MissingProtocol(owner) = bound.protocol(unused).unwrap_err() else {
                panic!("missing protocol");
            };
            assert_eq!(owner, unused);
        });
    });
}

#[test]
fn complete_protocol_binding_rejects_distinct_nominal_proofs_even_with_identical_contents() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        let first = foundation
            .bind_nominal_sources(&sources.members.nominals, &mut meter())
            .unwrap();
        let second = foundation
            .bind_nominal_sources(&sources.members.nominals, &mut meter())
            .unwrap();
        let members = first
            .bind_member_sources(
                &sources.members.properties,
                &sources.members.callables,
                core,
                &mut meter(),
            )
            .unwrap();
        let constructors = second
            .bind_constructor_sources(&sources.constructors, &mut meter())
            .unwrap();
        assert_eq!(members.provider(), constructors.provider());
        assert!(matches!(
            members.bind_parameter_protocols(&constructors, &sources.protocols, &mut meter()),
            Err(Error::NominalSourcesMismatch)
        ));
    });
}
