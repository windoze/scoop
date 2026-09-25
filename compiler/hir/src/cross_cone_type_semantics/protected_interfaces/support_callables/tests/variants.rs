use super::*;
use scoop_identity::{
    CanonicalIdentifier, EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey,
    PersistentEnumVariantFieldId, PersistentEnumVariantId,
};

fn fixture(style: EnumSourceVariantStyleV1) -> (Fixture, NominalSupportCallableInterfaceV1) {
    let mut fixture = Fixture::default();
    let owner = fixture.graph.add("Choice", SourceNominalKind::Enum, &[]);
    let key = EnumVariantIdentityKey::source(
        &fixture.graph.keys[&owner.source],
        CanonicalIdentifier::new("Some").unwrap(),
    )
    .unwrap();
    let variant = PersistentEnumVariantId::from_key(&key).unwrap();
    let mut fields = vec![];
    let mut types = vec![];
    if style != EnumSourceVariantStyleV1::Unit {
        let selector = if style == EnumSourceVariantStyleV1::Positional {
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            }
        } else {
            EnumVariantFieldSelector::Named(CanonicalIdentifier::new("p0").unwrap())
        };
        let field = EnumVariantFieldKey::new(variant, selector);
        let id = PersistentEnumVariantFieldId::from_key(&field).unwrap();
        let ty = SignatureTypeKey::Nominal(nominal(fixture.unit));
        fields.push(EnumSourceFieldV1::new(id, ty.clone()));
        types.push(ty);
        fixture.variant_fields.insert(id, field);
    }
    fixture.variants.insert(
        variant,
        (
            key,
            EnumSourceVariantV1::try_new(variant, style, fields).unwrap(),
        ),
    );
    let template = fixture.function(owner, "template", false, types.clone());
    let mut payload = fixture
        .payload(
            owner,
            template,
            types,
            SignatureTypeKey::Nominal(nominal(owner)),
        )
        .source_signature;
    payload.source_interface = NominalSupportSourceInterfaceUseV1::VariantConstructor(variant);
    fixture.declarations.remove(&template);
    let record = NominalSupportCallableInterfaceV1::try_new(
        CallableTemplateOrigin::VariantConstructor(variant),
        fixture.access(owner, DeclaredVisibilityV1::Public),
        payload,
    )
    .unwrap();
    (fixture, record)
}

#[test]
fn variant_support_uses_its_own_identity_and_source_protocol_for_all_field_styles() {
    for style in [
        EnumSourceVariantStyleV1::Unit,
        EnumSourceVariantStyleV1::Positional,
        EnumSourceVariantStyleV1::Named,
        EnumSourceVariantStyleV1::Constructor,
    ] {
        let (mut fixture, record) = fixture(style);
        let bytes = encode(&record).unwrap();
        let decoded: DecodedNominalSupportCallableInterfaceV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture).unwrap(), record);
        assert!(decode_canonical::<DecodedProtectedCallableInterfaceV1>(&bytes).is_err());
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate(
            graph_source.records.values(),
            &graph_source,
        )
        .unwrap();
        let checked = record.validate_source(&graph, &mut fixture).unwrap();
        assert!(matches!(
            checked.declaration_access(),
            CheckedNominalSupportAccessSourceV1::Variant(_)
        ));
        assert!(
            checked
                .declaration_access()
                .replay(&graph)
                .unwrap()
                .lookup()
                .domain()
                .is_universal()
        );
    }
}

#[test]
fn variant_source_validation_rejects_wrong_field_role_result_and_origin() {
    let (fixture, record) = fixture(EnumSourceVariantStyleV1::Constructor);
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let mut changed = fixture.clone();
    let field = *changed.variant_fields.keys().next().unwrap();
    let key = changed.variant_fields[&field].clone();
    changed.variant_fields.insert(
        field,
        EnumVariantFieldKey::new(
            key.variant(),
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ),
    );
    assert!(matches!(
        record.validate_source(&graph, &mut changed),
        Err(NominalSupportCallableSemanticError::Variant(
            NominalSupportVariantError::Parameters
        ))
    ));
    let mut bad = record.clone();
    bad.payload.result = SignatureTypeKey::Nominal(nominal(fixture.unit));
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture.clone()),
        Err(NominalSupportCallableSemanticError::Variant(
            NominalSupportVariantError::Result
        ))
    ));
    let mut bad = record;
    bad.declaration_access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Private,
        bad.declaration_access.lexical_owners().to_vec(),
        bad.declaration_access.definition_origin().clone(),
    )
    .unwrap();
    assert!(matches!(
        bad.validate_source(&graph, &mut fixture.clone()),
        Err(NominalSupportCallableSemanticError::Variant(
            NominalSupportVariantError::Access
        ))
    ));
}
