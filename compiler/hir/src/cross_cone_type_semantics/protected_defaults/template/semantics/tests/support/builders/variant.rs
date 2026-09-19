use super::*;

impl Case {
    pub fn variant() -> Self {
        let mut fixture = Fixture::default();
        let owner = fixture.graph.add("Choice", SourceNominalKind::Enum, &[]);
        let key = EnumVariantIdentityKey::source(
            &fixture.graph.keys[&owner.source],
            CanonicalIdentifier::new("Some").unwrap(),
        )
        .unwrap();
        let variant = PersistentEnumVariantId::from_key(&key).unwrap();
        let field = EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Named(CanonicalIdentifier::new("p0").unwrap()),
        );
        let field_id = PersistentEnumVariantFieldId::from_key(&field).unwrap();
        let value = SignatureTypeKey::Nominal(nominal(fixture.unit));
        fixture.variant_fields.insert(field_id, field);
        fixture.variants.insert(
            variant,
            (
                key,
                EnumSourceVariantV1::try_new(
                    variant,
                    EnumSourceVariantStyleV1::Constructor,
                    vec![EnumSourceFieldV1::new(field_id, value.clone())],
                )
                .unwrap(),
            ),
        );
        let declaration = CallableTemplateOrigin::VariantConstructor(variant);
        let payload = NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner.source,
            binders(false),
            parameters(vec![value]),
            SignatureTypeKey::Nominal(nominal(owner)),
            effects(),
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap();
        let origin = fixture.graph.origins[&owner.source].clone();
        let record = Record::Support(
            NominalSupportCallableInterfaceV1::try_new(
                declaration,
                fixture.access(owner, DeclaredVisibilityV1::Public),
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
}
