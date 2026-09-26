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
        let mut protocols = BTreeMap::new();
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
            let production = hir::NestedNominalSourceProductionV1::from_export_hir(
                &output.output().export,
                nominal.owner(),
            )
            .unwrap();
            let mut expected = BTreeSet::new();
            collect_protocols(production.record(), &mut expected);
            assert_eq!(
                production
                    .protocols()
                    .records()
                    .iter()
                    .map(|r| r.owner())
                    .collect::<BTreeSet<_>>(),
                expected
            );
            let repeated = hir::NestedNominalSourceProductionV1::from_export_hir(
                &output.output().export,
                nominal.owner(),
            )
            .unwrap();
            assert_eq!(production.record(), repeated.record());
            assert_eq!(production.protocols(), repeated.protocols());
            for protocol in production.protocols().records() {
                if let Some(previous) = protocols.insert(protocol.owner(), protocol.clone()) {
                    assert_eq!(&previous, protocol);
                }
            }
            let (record, _) = production.into_parts();
            let bytes = encode(&record).unwrap();
            let decoded: hir::DecodedNominalSupportNestedInterfaceV1 =
                decode_canonical(&bytes).unwrap();
            let restored = decoded.resolve(&mut fixture.identities).unwrap();
            assert_eq!(encode(&restored).unwrap(), bytes);
            records.push(restored);
        }
        assert!(!records.is_empty());
        let protocols = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
            protocols.into_values().collect(),
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
        let bytes = encode(&protocols.index_templates(&keys).unwrap()).unwrap();
        let decoded: hir::DecodedCanonicalProtectedCallableSourceInterfacesV1 =
            decode_canonical(&bytes).unwrap();
        let protocols = decoded.resolve(&mut fixture.identities, &keys).unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        run(
            &fixture,
            &sources,
            &Candidates { records, protocols },
            inputs.protocols().fundamental_types(),
        );
    });
}
pub(super) fn rebuild(record: &Record, interface: hir::ProtectedNestedSourceInterfaceV1) -> Record {
    Record::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::ProtectedNestedNominalPayloadV1::try_new(record.declaration(), interface).unwrap(),
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
        let kind = match record.declaration() {
            hir::SourceNominalId::Concrete(_) => "Concrete",
            hir::SourceNominalId::GenericTemplate(_) => "GenericTemplate",
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
