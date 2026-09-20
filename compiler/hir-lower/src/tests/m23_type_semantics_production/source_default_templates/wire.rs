use super::*;
use scoop_wire::{Decoder, WireDecode};

// Capture complete field byte ranges using the public typed decoders, without
// reconstructing a candidate default or depending on private source DTO fields.
pub(super) fn fields(value: &Template) -> Vec<Vec<u8>> {
    let bytes = bytes(value);
    let mut shared = meter();
    let mut d = Decoder::new(&bytes, &mut shared).unwrap();
    d.expect_map(12).unwrap();
    let mut result = Vec::new();
    macro_rules! field {
        ($index:literal, $ty:ty) => {{
            let begin = d.position() as usize;
            d.field($index, <$ty>::decode).unwrap();
            result.push(bytes[begin..d.position() as usize].to_vec());
        }};
    }
    field!(1, hir::DecodedProtectedDefaultTemplateKeyV1);
    field!(2, hir::DecodedPersistentLexicalRootV1);
    field!(3, scoop_identity::StructuralDefinitionPath);
    field!(4, hir::DecodedCanonicalTemplateLocalTableV1);
    field!(5, hir::DecodedExportDefaultBodyV1);
    field!(6, scoop_identity::DecodedSignatureTypeKey);
    field!(7, hir::CanonicalBooleanV1);
    field!(8, hir::DecodedCanonicalBinderUseListV1);
    field!(9, hir::DecodedOptionalTemplateReceiverV1);
    field!(10, hir::DecodedCanonicalTemplateValueParametersV1);
    field!(11, hir::DecodedDefaultSourceReferencesV1);
    field!(12, hir::DecodedExportDefinitionSourceV1);
    d.finish().unwrap();
    result
}
pub(super) fn replaced(value: &Template, field: u8, payload: Vec<u8>) -> Decoded {
    let mut fields = fields(value);
    fields[field as usize - 1] = [vec![field], payload].concat();
    let bytes = [vec![0xac], fields.concat()].concat();
    decode_canonical(&bytes, DecodeLimits::default()).unwrap()
}

#[test]
fn source_template_wire_requires_exact_fields_and_known_key_identity() {
    with_hir_source(SOURCE, |output, _| {
        let value = template(output, "SourceBase.callback", 1);
        let bytes = bytes(&value);
        for count in [0xa0, 0xab, 0xad] {
            let mut malformed = bytes.clone();
            malformed[0] = count;
            assert!(decode_canonical::<Decoded>(&malformed, DecodeLimits::default()).is_err());
        }
        let mut fields = fields(&value);
        fields.swap(2, 3);
        assert!(
            decode_canonical::<Decoded>(
                &[vec![0xac], fields.concat()].concat(),
                DecodeLimits::default()
            )
            .is_err()
        );
        let input: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            input.resolve(&mut empty, &mut meter()),
            Err(hir::DefaultSourceTemplateResolutionError::Key(_))
        ));
    });
}
