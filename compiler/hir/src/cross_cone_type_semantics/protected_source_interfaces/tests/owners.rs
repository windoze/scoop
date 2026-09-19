use super::*;
use scoop_identity::{
    AccessorRole, CanonicalIdentifier, EnumVariantIdentityKey, PersistentEnumVariantId,
    SourceNominalKind,
};
use scoop_wire::{Encoder, WireEncode};

#[test]
fn protected_default_index_cannot_rebind_a_parameter_to_another_callable() {
    let (mut fixture, _, source, keys, _) = fixture(false);
    let other_owner = fixture.class("Other");
    let other = fixture.function(other_owner, "method", false, vec![]);
    let foreign_keys = ProtectedDefaultKeyIndexV1::try_new(vec![
        ProtectedDefaultTemplateKeyV1::try_new(other, 1).unwrap(),
    ])
    .unwrap();
    let bytes = encode(&source.index_templates(&keys, &mut meter()).unwrap()).unwrap();
    let decoded: DecodedProtectedCallableSourceInterfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &foreign_keys, &mut meter()),
        Err(ProtectedSourceResolutionError::Build(
            ProtectedSourceBuildError::TemplateOwner { position: 1 }
        ))
    ));
}

struct OrderedRecords<'a>(Vec<IndexedProtectedCallableSourceInterfaceV1<'a>>);
impl WireEncode for OrderedRecords<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[test]
fn source_reader_rejects_reversed_distinct_owners_without_sorting() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let function = fixture.function(owner, "method", false, vec![]);
    let constructor = CallableTemplateOrigin::Constructor(fixture.constructor(owner));
    let sources = [constructor, function].map(|declaration| {
        ProtectedCallableSourceInterfaceV1::try_new(
            declaration,
            CanonicalProtectedSourceParametersV1::try_new(vec![]).unwrap(),
        )
        .unwrap()
    });
    let keys = ProtectedDefaultKeyIndexV1::default();
    let ordered = OrderedRecords(
        sources
            .iter()
            .map(|source| source.index_templates(&keys, &mut meter()).unwrap())
            .collect(),
    );
    let decoded: DecodedCanonicalProtectedCallableSourceInterfacesV1 =
        decode_canonical(&encode(&ordered).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &keys, &mut meter()),
        Err(ProtectedSourceResolutionError::Build(
            ProtectedSourceBuildError::NonCanonicalOrder
        ))
    ));
    let canonical =
        CanonicalProtectedCallableSourceInterfacesV1::try_new(sources.to_vec()).unwrap();
    assert_eq!(canonical.records()[0].owner(), function);
    let bytes = encode(&canonical.index_templates(&keys, &mut meter()).unwrap()).unwrap();
    let decoded: DecodedCanonicalProtectedCallableSourceInterfacesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture, &keys, &mut meter()).unwrap(),
        canonical
    );
}

#[test]
fn protected_default_keys_preserve_all_source_callable_kinds_and_reject_accessors() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let function = fixture.function(owner, "method", false, vec![]);
    let generic = fixture.function(owner, "generic", true, vec![]);
    let constructor = CallableTemplateOrigin::Constructor(fixture.constructor(owner));
    let enum_owner = fixture.graph.add("Choice", SourceNominalKind::Enum, &[]);
    let variant_key = EnumVariantIdentityKey::source(
        &fixture.graph.keys[&enum_owner.source],
        CanonicalIdentifier::new("Some").unwrap(),
    )
    .unwrap();
    let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
    fixture.variants.insert(
        variant,
        (
            variant_key,
            EnumSourceVariantV1::try_new(variant, EnumSourceVariantStyleV1::Unit, vec![]).unwrap(),
        ),
    );
    for owner in [
        function,
        generic,
        constructor,
        CallableTemplateOrigin::VariantConstructor(variant),
    ] {
        let key = ProtectedDefaultTemplateKeyV1::try_new(owner, 7).unwrap();
        let bytes = encode(&key).unwrap();
        let decoded: DecodedProtectedDefaultTemplateKeyV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.resolve(&mut fixture).unwrap(), key);
    }
    let unit = SignatureTypeKey::Nominal(
        crate::cross_cone_type_semantics::protected_interfaces::tests::support::nominal(
            fixture.unit,
        ),
    );
    let accessor = fixture.accessor(owner, AccessorRole::Getter, unit);
    assert!(matches!(
        ProtectedDefaultTemplateKeyV1::try_new(accessor, 0),
        Err(ProtectedSourceBuildError::AccessorOwner)
    ));
    let bytes = encode(&ExportDefaultTemplateKeyV1::new(accessor, 0)).unwrap();
    let decoded: DecodedProtectedDefaultTemplateKeyV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(ProtectedSourceResolutionError::Build(
            ProtectedSourceBuildError::AccessorOwner
        ))
    ));
}
