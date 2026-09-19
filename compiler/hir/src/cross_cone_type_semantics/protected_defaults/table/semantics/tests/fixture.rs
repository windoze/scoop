use super::*;

pub(super) struct Fixture {
    pub source: SourceFixture,
    pub protected: CanonicalProtectedDeclarationInterfacesV1,
    pub inheritance: CanonicalNominalInheritanceInterfacesV1,
    pub protocols: CanonicalProtectedCallableSourceInterfacesV1,
    pub owner: SourceNominalId,
    pub key: ProtectedDefaultTemplateKeyV1,
    pub origin: ExportDefinitionSourceV1,
    pub receiver: SignatureTypeKey,
    pub unit: SignatureTypeKey,
    pub direct: Option<PersistentAccessDomainV1>,
    pub lambda: PersistentGeneratedCallableId,
}
impl Fixture {
    pub fn new(case: Case) -> Self {
        let mut bundle = inheritance_interface_fixture();
        let origin = bundle.fixture.graph.origins[&bundle.derived.source].clone();
        let owner = if case.generic() {
            let key = SourceDeclarationKey::nominal(
                site(&[]),
                CanonicalIdentifier::new("GenericDefaultOwner").unwrap(),
                SourceNominalKind::Class,
                1,
            );
            let owner = SourceNominalId::from_source_declaration(&key).unwrap();
            bundle.fixture.graph.keys.insert(owner, key);
            bundle.fixture.graph.access.insert(
                owner,
                DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Public,
                    vec![],
                    origin.clone(),
                )
                .unwrap(),
            );
            bundle.fixture.graph.origins.insert(owner, origin.clone());
            owner
        } else {
            bundle.derived.source
        };
        let unit = SignatureTypeKey::Nominal(nominal(bundle.fixture.unit));
        let key = SourceDeclarationKey::function(
            site(&[owner]),
            CanonicalIdentifier::new("completeDefault").unwrap(),
            0,
            None,
            vec![unit.clone(), unit.clone()],
        );
        let function = PersistentFunctionId::from_source_declaration(&key).unwrap();
        let declaration = CallableTemplateOrigin::Function(function);
        bundle.fixture.declarations.insert(declaration, key);
        let effects = bundle
            .fixture
            .payload(bundle.derived, declaration, vec![], unit.clone())
            .effects();
        let payload = ProtectedCallablePayloadV1::try_new(
            declaration,
            owner,
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(
                (0..2)
                    .map(|index| {
                        SourceParameterShapeV1::new(
                            CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                            unit.clone(),
                        )
                    })
                    .collect(),
            )
            .unwrap(),
            unit.clone(),
            effects,
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap();
        let record = ProtectedDeclarationInterfaceV1::Callable(Box::new(
            ProtectedCallableInterfaceV1::try_new(
                declaration,
                DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Protected,
                    vec![owner],
                    origin.clone(),
                )
                .unwrap(),
                payload,
            )
            .unwrap(),
        ));
        let reference = record.reference();
        let mut protected = bundle.protected.records().to_vec();
        protected.push(record);
        bundle.fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(
            protected
                .iter()
                .map(ProtectedDeclarationInterfaceV1::reference)
                .collect(),
        )
        .unwrap();
        bundle.protected = CanonicalProtectedDeclarationInterfacesV1::try_new(protected).unwrap();
        if !case.generic() {
            let mut members = bundle
                .table
                .get(bundle.derived.exact)
                .unwrap()
                .protected_members()
                .values()
                .to_vec();
            members.push(reference);
            let members = CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap();
            bundle
                .fixture
                .inheritance_interfaces
                .members
                .insert(bundle.derived.exact, members.clone());
            bundle.change(bundle.derived, |record| {
                *record = NominalInheritanceInterfaceV1::try_new(
                    record.edges().clone(),
                    record.domains().clone(),
                    record.constructors().clone(),
                    record.slots().clone(),
                    members,
                    record.slot_schemas().clone(),
                )
                .unwrap();
            });
        }
        let key = ProtectedDefaultTemplateKeyV1::try_new(declaration, 1).unwrap();
        let mut owners = bundle
            .protected
            .records()
            .iter()
            .filter_map(|record| match record {
                ProtectedDeclarationInterfaceV1::Callable(record) => Some(record.declaration()),
                ProtectedDeclarationInterfaceV1::Constructor(record) => {
                    Some(CallableTemplateOrigin::Constructor(record.declaration()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for record in bundle.table.records() {
            owners.extend(
                record
                    .constructors()
                    .records()
                    .iter()
                    .map(|record| CallableTemplateOrigin::Constructor(record.declaration())),
            );
        }
        owners.sort_unstable();
        owners.dedup();
        let protocols = CanonicalProtectedCallableSourceInterfacesV1::try_new(
            owners
                .into_iter()
                .map(|owner| {
                    let parameters = if owner == declaration {
                        (0..2)
                            .map(|index| {
                                ProtectedSourceParameterV1::new(
                                    CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                                    unit.clone(),
                                    if index == 0 {
                                        ProtectedParameterCallingV1::Required
                                    } else {
                                        ProtectedParameterCallingV1::Default { template: key }
                                    },
                                    origin.clone(),
                                )
                            })
                            .collect()
                    } else {
                        vec![]
                    };
                    ProtectedCallableSourceInterfaceV1::try_new(
                        owner,
                        CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let receiver = match owner {
            SourceNominalId::Concrete(owner) => SignatureTypeKey::Nominal(owner),
            SourceNominalId::GenericTemplate(origin) => SignatureTypeKey::NominalApplication {
                origin,
                arguments: NonEmptyVec::from_first(
                    SignatureTypeKey::Binder { depth: 0, index: 0 },
                    [],
                ),
            },
        };
        // This expectation is independent of the candidate witness.
        let direct = (!case.generic()).then(|| {
            PersistentAccessDomainV1::try_from_constraints(vec![
                PersistentAccessConstraintV1::SubclassesOf(bundle.derived.exact),
            ])
            .unwrap()
        });
        let lambda = PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(function),
            role: LexicalCallableRole::LambdaBody,
            path: nested_path(StructuralDefinitionSiteRole::Lambda),
        })
        .unwrap();
        Self {
            source: bundle.fixture,
            protected: bundle.protected,
            inheritance: bundle.table,
            protocols,
            owner,
            key,
            origin,
            receiver,
            unit,
            direct,
            lambda,
        }
    }
    pub fn function_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: vec![],
            result: Box::new(self.unit.clone()),
        }
    }
}
