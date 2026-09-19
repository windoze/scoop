use super::*;
use scoop_identity::{Effect, GcEffect};

pub(super) fn runtime(
    fixture: &mut Fixture,
    owner: SourceNominalId,
    chain: &[SourceNominalId],
) -> (ProtectedPropertyInterfaceV1, Vec<NestedSourceSupportV1>) {
    let key = SourceDeclarationKey::property(
        site(ConeIdentity::CORE, chain),
        CanonicalIdentifier::new("value").unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&key).unwrap();
    let getter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::Property(property),
        AccessorRole::Getter,
    ))
    .unwrap();
    let setter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::Property(property),
        AccessorRole::Setter,
    ))
    .unwrap();
    let property_origin = origin(ConeIdentity::CORE, 21);
    let getter_origin = origin(ConeIdentity::CORE, 22);
    let setter_origin = origin(ConeIdentity::CORE, 23);
    let interface = NominalSourcePropertyPayloadV1::try_new(
        owner,
        unit(),
        getter,
        ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access: access(
                chain.to_vec(),
                DeclaredVisibilityV1::Private,
                &setter_origin,
            ),
        },
        PropertyRepresentationV1::RuntimeAccessor,
        CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
    )
    .unwrap();
    let runtime = NominalSupportPropertyInterfaceV1::try_new(
        property,
        access(
            chain.to_vec(),
            DeclaredVisibilityV1::Protected,
            &property_origin,
        ),
        NominalSupportPropertyPayloadV1::Runtime { interface },
    )
    .unwrap();
    let protected = ProtectedPropertyInterfaceV1::try_new(
        property,
        access(
            chain.to_vec(),
            DeclaredVisibilityV1::Protected,
            &property_origin,
        ),
        ProtectedPropertyPayloadV1::try_new(
            owner,
            unit(),
            getter,
            ProtectedPropertyMutabilityV1::ReadWrite {
                setter,
                setter_access: access(
                    chain.to_vec(),
                    DeclaredVisibilityV1::Private,
                    &setter_origin,
                ),
            },
            PropertyRepresentationV1::RuntimeAccessor,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut support = vec![NestedSourceSupportV1::Property(Box::new(runtime))];
    fixture.expect(
        UseKey::Nested(owner, support[0].declaration()),
        &property_origin,
    );
    // The same setter-access record occurs in the nested support and top-level property.
    fixture.expect(UseKey::Setter(property), &setter_origin);
    fixture.expect(UseKey::Setter(property), &setter_origin);
    for (id, source, parameters) in [
        (getter, getter_origin, vec![]),
        (
            setter,
            setter_origin,
            vec![SourceParameterShapeV1::new(
                CanonicalIdentifier::new("value").unwrap(),
                unit(),
            )],
        ),
    ] {
        let declaration = CallableTemplateOrigin::Accessor(id);
        let payload = NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner,
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
            unit(),
            CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                CallableSafetyV1::Safe,
                GcEffect::Managed,
                CallableImplementationV1::Scoop,
                CallableOperatorRoleV1::None,
                CallableInfixV1::Ordinary,
            )
            .unwrap(),
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap();
        let record = NestedSourceSupportV1::Callable(Box::new(
            NominalSupportCallableInterfaceV1::try_new(
                declaration,
                access(chain.to_vec(), DeclaredVisibilityV1::Private, &source),
                payload,
            )
            .unwrap(),
        ));
        fixture.expect(UseKey::Nested(owner, record.declaration()), &source);
        support.push(record);
    }
    (protected, support)
}
