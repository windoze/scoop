use super::*;
use scoop_wire::{Encoder, WireEncode};

struct RepeatedRecord<'a>(&'a IndexedProtectedCallableSourceInterfaceV1<'a>);
impl WireEncode for RepeatedRecord<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(2)?;
        self.0.encode(encoder)?;
        self.0.encode(encoder)
    }
}
#[test]
fn source_reader_rejects_duplicate_owner_and_unknown_calling_convention() {
    let (mut fixture, _, source, keys, _) = fixture(true);
    let indexed = source.index_templates(&keys).unwrap();
    let bytes = encode(&RepeatedRecord(&indexed)).unwrap();
    let decoded: DecodedCanonicalProtectedCallableSourceInterfacesV1 =
        decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &keys),
        Err(ProtectedSourceResolutionError::Build(
            ProtectedSourceBuildError::DuplicateOwner
        ))
    ));
    assert!(decode_canonical::<DecodedProtectedParameterCallingV1>(&[0xa1, 0, 5]).is_err());
}
#[test]
fn source_parameter_builder_rejects_duplicate_names_and_multiple_varargs() {
    let (_, _, source, _, _) = fixture(false);
    let vararg = source.parameters().parameters()[2].clone();
    assert!(matches!(
        CanonicalProtectedSourceParametersV1::try_new(vec![vararg.clone(), vararg.clone()]),
        Err(ProtectedSourceBuildError::DuplicateName { position: 1 })
    ));
    let second = ProtectedSourceParameterV1::new(
        scoop_identity::CanonicalIdentifier::new("second").unwrap(),
        vararg.value_type().clone(),
        vararg.calling().clone(),
        vararg.definition_origin().clone(),
    );
    assert!(matches!(
        CanonicalProtectedSourceParametersV1::try_new(vec![vararg, second]),
        Err(ProtectedSourceBuildError::MultipleVarargs)
    ));
}
