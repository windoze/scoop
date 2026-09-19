use super::*;
pub(super) use crate::cross_cone_type_semantics::representation::tests::support::Fixture;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn path() -> WirePath {
    WirePath::root().field(2)
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    )
}
pub(super) fn source_key(name: &str, kind: SourceNominalKind) -> SourceDeclarationKey {
    use scoop_identity::{
        DeclarationScope, DefinitionOwnerChain, PackagePath, SourceDeclarationSite,
    };
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    SourceDeclarationKey::nominal(site, CanonicalIdentifier::new(name).unwrap(), kind, 0)
}
pub(super) fn parsed<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
pub(super) fn resource(
    error: MeteredNominalRepresentationResolutionError<&'static str>,
    expected: ResourceKind,
    at: &WirePath,
) {
    let MeteredNominalRepresentationResolutionError::Resource(error) = error else {
        panic!("{error:?}");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
        "{error:?}"
    );
    assert_eq!(error.path(), at);
    assert_eq!(error.byte_offset(), None);
}
pub(super) struct Sequence(pub Vec<NominalRepresentationSupportV1>);
impl WireEncode for Sequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
