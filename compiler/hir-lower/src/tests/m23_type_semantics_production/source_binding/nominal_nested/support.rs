use super::*;

pub(super) struct Candidates {
    pub records: Vec<Record>,
    pub protocols: hir::CanonicalProtectedCallableSourceInterfacesV1,
}
pub(super) fn with_candidates(
    source: &str,
    run: impl FnOnce(&Fixture, &Sources, &Candidates, &hir::ImportedCoreFundamentalTypeProtocol),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut records = Vec::new();
        for nominal in sources.members.nominals.records() {
            let access = fixture
                .source
                .entries()
                .sources
                .get(nominal.owner())
                .unwrap()
                .access();
            if access.lexical_owners().is_empty() {
                continue;
            }
            let record = build(&fixture, &sources, nominal.owner());
            let bytes = encode(&record).unwrap();
            let decoded: hir::DecodedNominalSupportNestedInterfaceV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let restored = decoded
                .resolve(&mut fixture.identities, &mut meter())
                .unwrap();
            assert_eq!(encode(&restored).unwrap(), bytes);
            records.push(restored);
        }
        assert!(!records.is_empty());
        let protocols = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
            sources
                .protocols
                .records()
                .iter()
                .map(|r| super::super::parameter_candidates::candidate(r.owner(), r.parameters()))
                .collect(),
        )
        .unwrap();
        let keys = hir::ProtectedDefaultKeyIndexV1::try_new(
            protocols
                .records()
                .iter()
                .flat_map(|r| r.parameters().parameters())
                .filter_map(|p| p.calling().template())
                .collect(),
        )
        .unwrap();
        let bytes = encode(&protocols.index_templates(&keys, &mut meter()).unwrap()).unwrap();
        let decoded: hir::DecodedCanonicalProtectedCallableSourceInterfacesV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let protocols = decoded
            .resolve(&mut fixture.identities, &keys, &mut meter())
            .unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(
            &fixture,
            &sources,
            &Candidates { records, protocols },
            inputs.protocols().fundamental_types(),
        );
    });
}
fn build(fixture: &Fixture, sources: &Sources, owner: hir::SourceNominalId) -> Record {
    let nominal = sources.members.nominals.get(owner).unwrap();
    let mut entries = sources
        .constructors
        .records()
        .iter()
        .filter(|r| r.payload().owner() == owner)
        .map(|r| Entry::Constructor(Box::new(r.clone())))
        .collect::<Vec<_>>();
    entries.extend(
        sources
            .members
            .callables
            .records()
            .iter()
            .filter(|r| r.payload().owner() == owner)
            .map(|r| Entry::Callable(Box::new(r.clone()))),
    );
    entries.extend(
        sources
            .members
            .properties
            .records()
            .iter()
            .filter(|r| r.owner() == owner)
            .map(|r| Entry::Property(Box::new(r.clone()))),
    );
    for child in nominal.children().values() {
        entries.push(Entry::NestedNominal(Box::new(build(
            fixture, sources, *child,
        ))));
    }
    let interface = hir::ProtectedNestedSourceInterfaceV1::try_new(
        nominal.kind(),
        nominal.modality(),
        nominal.type_parameters().clone(),
        nominal.supertypes().clone(),
        nominal.constructors().clone(),
        nominal.members().clone(),
        nominal.children().clone(),
        nominal.source_shape().clone(),
        hir::CanonicalNestedSourceSupportV1::try_new(entries).unwrap(),
    )
    .unwrap();
    let support = match owner {
        hir::SourceNominalId::Concrete(id) => hir::NestedNominalSupportV1::ParamFree {
            inheritance_exact: PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id)).unwrap(),
            representation_owner: id,
        },
        hir::SourceNominalId::GenericTemplate(_) => hir::NestedNominalSupportV1::GenericTemplate,
    };
    Record::try_new(
        owner,
        fixture
            .source
            .entries()
            .sources
            .get(owner)
            .unwrap()
            .access()
            .clone(),
        hir::ProtectedNestedNominalPayloadV1::try_new(owner, interface, support).unwrap(),
    )
    .unwrap()
}
pub(super) fn rebuild(record: &Record, interface: hir::ProtectedNestedSourceInterfaceV1) -> Record {
    Record::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::ProtectedNestedNominalPayloadV1::try_new(
            record.declaration(),
            interface,
            record.payload().support(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub(super) fn replacing(record: &Record, entries: Vec<Entry>) -> Record {
    let s = record.payload().source_interface();
    rebuild(
        record,
        hir::ProtectedNestedSourceInterfaceV1::try_new(
            s.kind(),
            s.modality(),
            s.type_parameters().clone(),
            s.supertypes().clone(),
            s.constructors().clone(),
            s.members().clone(),
            s.children().clone(),
            s.source_shape().clone(),
            hir::CanonicalNestedSourceSupportV1::try_new(entries).unwrap(),
        )
        .unwrap(),
    )
}
pub(super) fn collect_protocols(record: &Record, owners: &mut BTreeSet<CallableTemplateOrigin>) {
    for entry in record
        .payload()
        .source_interface()
        .source_support()
        .records()
    {
        match entry {
            Entry::Callable(record) => {
                if !matches!(record.declaration(), CallableTemplateOrigin::Accessor(_)) {
                    owners.insert(record.declaration());
                }
            }
            Entry::Constructor(record) => {
                owners.insert(CallableTemplateOrigin::Constructor(record.declaration()));
            }
            Entry::NestedNominal(record) => collect_protocols(record, owners),
            Entry::Property(_) => {}
        }
    }
}
pub(super) fn outline(fixture: &Fixture, records: &[Record]) -> String {
    let foundation = fixture.bind().unwrap();
    let mut rows = Vec::new();
    for record in records {
        let key = foundation.nominal_key(record.declaration()).unwrap();
        let scoop_identity::DeclarationName::Named(name) = key.name() else {
            panic!("named nominal");
        };
        let mut counts = [0; 4];
        for entry in record
            .payload()
            .source_interface()
            .source_support()
            .records()
        {
            counts[match entry {
                Entry::Callable(_) => 0,
                Entry::Constructor(_) => 1,
                Entry::Property(_) => 2,
                Entry::NestedNominal(_) => 3,
            }] += 1;
        }
        let mut protocols = BTreeSet::new();
        collect_protocols(record, &mut protocols);
        let kind = match record.payload().support() {
            hir::NestedNominalSupportV1::ParamFree { .. } => "ParamFree",
            hir::NestedNominalSupportV1::GenericTemplate => "GenericTemplate",
        };
        rows.push(format!(
            "{name}: {kind}, callable={}, constructor={}, property={}, child={}, protocols={}\n",
            counts[0],
            counts[1],
            counts[2],
            counts[3],
            protocols.len()
        ));
    }
    rows.sort();
    rows.concat()
}
