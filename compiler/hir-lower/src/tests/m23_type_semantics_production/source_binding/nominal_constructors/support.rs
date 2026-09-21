use super::*;

#[derive(Clone)]
pub(super) struct Sources {
    pub nominals: hir::CanonicalNominalSourceContractsV1,
    pub constructors: Table,
}
impl Sources {
    pub(super) fn from_output(output: &hir::DependencyHirOutput, fixture: &mut Fixture) -> Self {
        let source = hir::CanonicalNominalSourceContractsV1::from_export_hir(
            &output.output().export,
            &fixture.source.entries().source_roots,
            &mut meter(),
        )
        .unwrap();
        let decoded: hir::DecodedCanonicalNominalSourceContractsV1 =
            decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
        let nominals = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(nominals, source);
        let required = nominals
            .records()
            .iter()
            .flat_map(|n| n.constructors().values().iter().copied())
            .collect();
        let source =
            Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        let bytes = encode(&source).unwrap();
        let decoded: hir::DecodedCanonicalNominalSourceConstructorsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let constructors = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(encode(&constructors).unwrap(), bytes);
        Self {
            nominals,
            constructors,
        }
    }
    pub(super) fn named(&self, fixture: &Fixture, owner: &str, parameters: &[&str]) -> &Record {
        let foundation = fixture.bind().unwrap();
        self.constructors.records().iter().find(|r| {
            matches!(foundation.nominal_key(r.payload().owner()).unwrap().name(), DeclarationName::Named(name) if name.as_str() == owner)
                && r.payload().parameters().parameters().iter().map(|p| p.name().as_str()).eq(parameters.iter().copied())
        }).unwrap()
    }
    pub(super) fn replace(&mut self, record: Record) {
        self.constructors = Table::try_new(
            self.constructors
                .records()
                .iter()
                .map(|r| {
                    if r.declaration() == record.declaration() {
                        record.clone()
                    } else {
                        r.clone()
                    }
                })
                .collect(),
            &mut meter(),
        )
        .unwrap();
    }
}
pub(super) fn with_payload(
    record: &Record,
    owner: hir::SourceNominalId,
    parameters: hir::CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
    effects: hir::CallableSourceEffectsV1,
) -> Record {
    let payload = record.payload();
    let mut owners = record.declaration_access().lexical_owners().to_vec();
    *owners.last_mut().unwrap() = owner;
    Record::try_new(
        record.declaration(),
        hir::DeclarationAccessSourceV1::try_new(
            record.declaration_access().declared_visibility(),
            owners,
            record.declaration_access().definition_origin().clone(),
        )
        .unwrap(),
        hir::NominalSourceCallablePayloadV1::try_new(
            CallableTemplateOrigin::Constructor(record.declaration()),
            owner,
            payload.type_parameters().clone(),
            parameters,
            result,
            effects,
            payload.modality(),
            payload.slot_relations().clone(),
        )
        .unwrap(),
    )
    .unwrap()
}
