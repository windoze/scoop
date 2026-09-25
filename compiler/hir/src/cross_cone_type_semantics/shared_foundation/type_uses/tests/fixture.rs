use super::*;

mod callables;
mod calls;
mod constructors;
mod nominals;
pub(super) use callables::{CallForm, origin};

pub(super) struct Artifact {
    coordinate: ConeCoordinate,
    nominals: Vec<(
        CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
        NominalInterfaceRecordV1,
    )>,
}

pub(super) struct Loaded {
    provider: ConeIdentity,
    pub identities: ValidatedIdentityGraph,
    foundation: OdrFreeHirFoundation,
    public: CrossConeHirInterfaceSectionV1,
}

impl Artifact {
    pub fn new(coordinate: ConeCoordinate) -> Self {
        Self {
            coordinate,
            nominals: Vec::new(),
        }
    }

    pub fn load(self, dependencies: &[&Loaded]) -> Loaded {
        let provider = self.coordinate.identity().unwrap();
        let file = SourceIdentity::new(
            provider,
            NormalizedSourcePath::new("fixture.scoop").unwrap(),
        )
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: file.clone(),
        })
        .unwrap();
        let origin = scoop_identity::DefinitionOrigin::new(
            file.clone(),
            SourceSpan::new(0, 7).unwrap(),
            context.key(),
        )
        .unwrap();
        let mut canonical = CanonicalHirFoundation::empty();
        let mut types = vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ];
        types.extend(self.nominals.iter().map(|(identity, _)| identity.clone()));
        canonical.set_types(types).unwrap();
        canonical
            .set_object_values(
                self.nominals
                    .iter()
                    .filter(|(_, nominal)| nominal.kind() == PublicNominalKindV1::Object)
                    .map(|(identity, _)| {
                        CborIdentityRecord::from_key(identity.key().clone()).unwrap()
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_exact_types(
                self.nominals
                    .iter()
                    .map(|(identity, _)| {
                        CborIdentityRecord::from_key(ExactTypeKey::Nominal(identity.id())).unwrap()
                    })
                    .chain(
                        [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any]
                            .into_iter()
                            .map(|builtin| {
                                CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                                    builtin.identity_record().id(),
                                ))
                                .unwrap()
                            }),
                    )
                    .collect(),
            )
            .unwrap();
        canonical
            .set_sources(vec![
                SourceRecord::from_utf8(file, "fixture", [0, 7]).unwrap(),
            ])
            .unwrap();
        canonical.set_source_contexts(vec![context]).unwrap();
        canonical
            .set_definition_origins(
                self.nominals
                    .iter()
                    .map(|(identity, _)| {
                        DefinitionOriginRecord::new(
                            DefinitionOriginSubject::Type(identity.id()),
                            origin.clone(),
                        )
                    })
                    .collect(),
            )
            .unwrap();
        let decoded: DecodedHirFoundation = scoop_wire::decode_canonical(
            &scoop_wire::encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut pending = PendingIdentityValidation::new();
        for provider in BTreeSet::from([provider, ConeIdentity::CORE]) {
            pending.register_authority(provider).unwrap();
        }
        decoded.register_identities(&mut pending).unwrap();
        for dependency in dependencies {
            pending
                .register_external_graph_authorities(&dependency.identities)
                .unwrap();
        }
        decoded.resolve_identities(&mut pending).unwrap();
        let mut identities = pending.finish().unwrap();
        let foundation = OdrFreeHirFoundation::from_validated(
            decoded
                .validate_with_dependency_sources(&self.coordinate, &mut identities, &mut meter())
                .unwrap(),
        )
        .unwrap();
        let mut public = CrossConeHirInterfaceSectionV1::new(
            Default::default(),
            CanonicalNominalInterfacesV1::try_new(
                self.nominals
                    .into_iter()
                    .map(|(_, nominal)| nominal)
                    .collect(),
            )
            .unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
            CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        let decoded: DecodedCrossConeHirInterfaceSectionV1 = scoop_wire::decode_canonical(
            &scoop_wire::encode(&public.index_for_wire().unwrap()).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let public = decoded
            .resolve_metered(&mut identities, &mut meter())
            .unwrap();
        Loaded {
            provider,
            identities,
            foundation,
            public,
        }
    }
}

impl Loaded {
    pub fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub fn metadata(&self) -> SharedTypeMetadataV1<'_> {
        SharedTypeMetadataV1 {
            provider: self.provider,
            identities: &self.identities,
            foundation: &self.foundation,
            public: &self.public,
        }
    }

    pub fn uses(
        &self,
        dependencies: &[&Loaded],
    ) -> Result<CanonicalSelectedExternalTypeUsesV1, Error> {
        self.metadata().materialized_type_uses(
            &dependencies
                .iter()
                .map(|dependency| dependency.metadata())
                .collect::<Vec<_>>(),
            &mut meter(),
        )
    }

    pub fn validate(
        &self,
        selected: &CanonicalSelectedExternalTypeUsesV1,
        dependencies: &[&Loaded],
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        self.metadata().validate_materialized_type_uses(
            selected,
            &dependencies
                .iter()
                .map(|dependency| dependency.metadata())
                .collect::<Vec<_>>(),
            meter,
        )
    }
}

pub(super) fn coordinate(name: &str) -> ConeCoordinate {
    ConeCoordinate::new("type-use.tests", name, "1.0.0").unwrap()
}

pub(super) fn exact(owner: PersistentTypeId) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap()
}

pub(super) fn selected(
    records: Vec<SelectedExternalTypeUseV1>,
) -> CanonicalSelectedExternalTypeUsesV1 {
    CanonicalSelectedExternalTypeUsesV1::try_new(records).unwrap()
}

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
