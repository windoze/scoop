use super::*;

mod variant;

impl Case {
    pub fn method(generic_owner: bool, generic_callable: bool, static_nested: bool) -> Self {
        let mut fixture = Fixture::default();
        let outer = add_owner(&mut fixture, &[], "Owner", u32::from(generic_owner));
        let owner = if static_nested {
            add_owner(&mut fixture, &[outer], "Nested", 0)
        } else {
            outer
        };
        let owners = if static_nested {
            vec![outer, owner]
        } else {
            vec![owner]
        };
        let value = if generic_callable {
            binder(0, 0)
        } else {
            SignatureTypeKey::Nominal(nominal(fixture.unit))
        };
        let key = SourceDeclarationKey::function(
            site(&owners),
            CanonicalIdentifier::new("method").unwrap(),
            u32::from(generic_callable),
            None,
            vec![value.clone(), value.clone()],
        );
        let declaration = if generic_callable {
            CallableTemplateOrigin::GenericFunction(
                PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
            )
        } else {
            CallableTemplateOrigin::Function(
                PersistentFunctionId::from_source_declaration(&key).unwrap(),
            )
        };
        fixture.declarations.insert(declaration, key);
        let origin = fixture.graph.origins[&owner].clone();
        let payload = ProtectedCallablePayloadV1::try_new(
            declaration,
            owner,
            binders(generic_callable),
            parameters(vec![value.clone(), value.clone()]),
            value,
            effects(),
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap();
        let record = Record::Method(
            ProtectedCallableInterfaceV1::try_new(
                declaration,
                DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Protected,
                    owners,
                    origin.clone(),
                )
                .unwrap(),
                payload,
            )
            .unwrap(),
        );
        let nominal_arity = u32::from(generic_owner && !static_nested);
        let expected_receiver = Some(match owner {
            SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
            SourceNominalId::GenericTemplate(id) => SignatureTypeKey::NominalApplication {
                origin: id,
                arguments: NonEmptyVec::from_first(binder(u32::from(generic_callable), 0), []),
            },
        });
        Self::finish(
            fixture,
            record,
            declaration,
            origin,
            DefaultTemplateProviderShapeV1::try_new(nominal_arity, u32::from(generic_callable))
                .unwrap(),
            expected_receiver,
        )
    }
    pub fn constructor() -> Self {
        let mut fixture = Fixture::default();
        let owner = fixture.class("Owner");
        let value = SignatureTypeKey::Nominal(nominal(fixture.unit));
        let key = SourceDeclarationKey::constructor(
            site(&[owner.source]),
            vec![value.clone(), value.clone()],
        );
        let id = PersistentConstructorId::from_source_declaration(&key).unwrap();
        let declaration = CallableTemplateOrigin::Constructor(id);
        fixture.declarations.insert(declaration, key);
        let origin = fixture.graph.origins[&owner.source].clone();
        let payload = fixture.payload(
            owner,
            declaration,
            vec![value.clone(), value],
            SignatureTypeKey::Nominal(nominal(owner)),
        );
        let record = Record::Constructor(
            ProtectedConstructorInterfaceV1::try_new(
                id,
                fixture.access(owner, DeclaredVisibilityV1::Protected),
                payload,
            )
            .unwrap(),
        );
        Self::finish(
            fixture,
            record,
            declaration,
            origin,
            DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap(),
            None,
        )
    }
    fn finish(
        fixture: Fixture,
        record: Record,
        declaration: CallableTemplateOrigin,
        origin: ExportDefinitionSourceV1,
        provider: DefaultTemplateProviderShapeV1,
        expected_receiver: Option<SignatureTypeKey>,
    ) -> Self {
        let count = record.payload().parameters().len_u32();
        let key = ProtectedDefaultTemplateKeyV1::try_new(declaration, count - 1).unwrap();
        let parameters = record
            .payload()
            .parameters()
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                ProtectedSourceParameterV1::new(
                    parameter.name().clone(),
                    parameter.value_type().clone(),
                    if index + 1 == count as usize {
                        ProtectedParameterCallingV1::Default { template: key }
                    } else {
                        ProtectedParameterCallingV1::Required
                    },
                    origin.clone(),
                )
            })
            .collect();
        let source = ProtectedCallableSourceInterfaceV1::try_new(
            declaration,
            CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
        )
        .unwrap();
        Self {
            fixture,
            record,
            source,
            key,
            provider,
            expected_receiver,
            origin,
        }
    }
}
fn add_owner(
    fixture: &mut Fixture,
    owners: &[SourceNominalId],
    name: &str,
    arity: u32,
) -> SourceNominalId {
    let key = SourceDeclarationKey::nominal(
        site(owners),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        arity,
    );
    let id = SourceNominalId::from_source_declaration(&key).unwrap();
    let origin = fixture.graph.origins[&fixture.unit.source].clone();
    fixture.graph.keys.insert(id, key);
    fixture.graph.access.insert(
        id,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Public,
            owners.to_vec(),
            origin.clone(),
        )
        .unwrap(),
    );
    fixture.graph.origins.insert(id, origin);
    id
}
fn parameters(types: Vec<SignatureTypeKey>) -> CanonicalSourceParameterShapesV1 {
    CanonicalSourceParameterShapesV1::try_new(
        types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SourceParameterShapeV1::new(
                    CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
                    ty,
                )
            })
            .collect(),
    )
    .unwrap()
}
fn binders(generic: bool) -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(if generic {
        vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )]
    } else {
        vec![]
    })
    .unwrap()
}
fn effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}
