//! Untrusted Wire CBOR representation of the MIR identity foundation.

use scoop_identity::{
    DecodedCallbackApplicationKey, DecodedCborIdentityRecord, DecodedEnumVariantFieldKey,
    DecodedEnumVariantIdentityKey, DecodedExactTypeKey, DecodedFieldIdentityKey,
    DecodedGeneratedCallableKey, DecodedGeneratedNominalKey, DecodedLocalValueKey,
    DecodedOdrMemberKey, DecodedSpecializationKey, IdentityLayer, IdentityValidationError,
    PendingIdentityValidation,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{DecodedCallableSignatureRecord, DecodedCallbackApplicationRecord};

mod validation;
pub use validation::{
    CallbackApplicationRelationError, MirFoundationReferenceError, MirFoundationValidationError,
    ValidatedMirFoundation,
};

type DecodedExactTypeRecord = DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>;
type DecodedGeneratedCallableRecord =
    DecodedCborIdentityRecord<PersistentGeneratedCallableId, DecodedGeneratedCallableKey>;
type DecodedGeneratedTypeRecord =
    DecodedCborIdentityRecord<PersistentTypeId, DecodedGeneratedNominalKey>;
type DecodedFieldRecord = DecodedCborIdentityRecord<PersistentFieldId, DecodedFieldIdentityKey>;
type DecodedEnumVariantRecord =
    DecodedCborIdentityRecord<PersistentEnumVariantId, DecodedEnumVariantIdentityKey>;
type DecodedEnumVariantFieldRecord =
    DecodedCborIdentityRecord<PersistentEnumVariantFieldId, DecodedEnumVariantFieldKey>;
type DecodedLocalValueRecord =
    DecodedCborIdentityRecord<PersistentLocalValueId, DecodedLocalValueKey>;
type DecodedCallbackApplicationIdentityRecord =
    DecodedCborIdentityRecord<PersistentCallbackApplicationId, DecodedCallbackApplicationKey>;
type DecodedOdrGroupRecord = DecodedCborIdentityRecord<OdrGroupId, DecodedSpecializationKey>;
type DecodedOdrMemberRecord = DecodedCborIdentityRecord<OdrMemberId, DecodedOdrMemberKey>;

#[derive(Debug)]
struct DecodedMirFoundationWire {
    exact_types: Vec<DecodedExactTypeRecord>,
    generated_callables: Vec<DecodedGeneratedCallableRecord>,
    generated_types: Vec<DecodedGeneratedTypeRecord>,
    fields: Vec<DecodedFieldRecord>,
    enum_variants: Vec<DecodedEnumVariantRecord>,
    enum_variant_fields: Vec<DecodedEnumVariantFieldRecord>,
    callable_signatures: Vec<DecodedCallableSignatureRecord>,
    local_values: Vec<DecodedLocalValueRecord>,
    callback_applications: Vec<DecodedCallbackApplicationIdentityRecord>,
    callback_application_records: Vec<DecodedCallbackApplicationRecord>,
    odr_groups: Vec<DecodedOdrGroupRecord>,
    odr_members: Vec<DecodedOdrMemberRecord>,
}

/// Canonically decoded MIR foundation wire graph.
///
/// This boundary proves the exact closed Wire CBOR product and canonical byte
/// spelling. Unverified identity bytes remain private until the three layers
/// are identity-checked, remapped, and committed atomically.
#[derive(Debug)]
pub struct DecodedMirFoundation {
    decoded: DecodedMirFoundationWire,
}

impl WireEncode for DecodedMirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.decoded.encode(encoder)
    }
}

impl WireDecode for DecodedMirFoundation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedMirFoundationWire::decode(decoder).map(|decoded| Self { decoded })
    }
}

impl DecodedMirFoundation {
    /// Registers every MIR-owned identity before cross-layer resolution.
    pub fn register_identities(
        &self,
        validation: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! register_tables {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.decoded.$table {
                    validation.register(IdentityLayer::Mir, record)?;
                })+
            };
        }

        register_tables!(
            exact_types,
            generated_callables,
            generated_types,
            fields,
            enum_variants,
            enum_variant_fields,
            local_values,
            callback_applications,
            odr_groups,
            odr_members,
        );
        Ok(())
    }

    /// Resolves every MIR-owned identity after all layers registered their
    /// candidates and HIR identities supplied the earlier-layer keys.
    pub fn resolve_identities(
        &self,
        validation: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! resolve_tables {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.decoded.$table {
                    validation.resolve(record)?;
                })+
            };
        }

        resolve_tables!(
            exact_types,
            generated_callables,
            generated_types,
            local_values,
            odr_groups,
            odr_members,
        );
        // Generated member keys reconstruct their nominal owners.
        resolve_tables!(fields, enum_variants, enum_variant_fields);
        // Applications reconstruct the HIR registration key.
        resolve_tables!(callback_applications);
        Ok(())
    }
}

impl WireEncode for DecodedMirFoundationWire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_table_field(encoder, 1, &self.exact_types)?;
        encode_table_field(encoder, 2, &self.generated_callables)?;
        encode_table_field(encoder, 3, &self.generated_types)?;
        encode_table_field(encoder, 4, &self.fields)?;
        encode_table_field(encoder, 5, &self.enum_variants)?;
        encode_table_field(encoder, 6, &self.enum_variant_fields)?;
        encode_table_field(encoder, 7, &self.callable_signatures)?;
        encode_table_field(encoder, 8, &self.local_values)?;
        encode_table_field(encoder, 9, &self.callback_applications)?;
        encode_table_field(encoder, 10, &self.callback_application_records)?;
        encode_table_field(encoder, 11, &self.odr_groups)?;
        encode_table_field(encoder, 12, &self.odr_members)
    }
}

impl WireDecode for DecodedMirFoundationWire {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            exact_types: decode_table_field(decoder, 1)?,
            generated_callables: decode_table_field(decoder, 2)?,
            generated_types: decode_table_field(decoder, 3)?,
            fields: decode_table_field(decoder, 4)?,
            enum_variants: decode_table_field(decoder, 5)?,
            enum_variant_fields: decode_table_field(decoder, 6)?,
            callable_signatures: decode_table_field(decoder, 7)?,
            local_values: decode_table_field(decoder, 8)?,
            callback_applications: decode_table_field(decoder, 9)?,
            callback_application_records: decode_table_field(decoder, 10)?,
            odr_groups: decode_table_field(decoder, 11)?,
            odr_members: decode_table_field(decoder, 12)?,
        })
    }
}

fn decode_table_field<T: WireDecode>(
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Vec<T>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| T::decode(decoder))
    })
}

fn encode_table_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_identity::{CborIdentityRecord, CoreBuiltinNominal, ExactTypeKey};
    use scoop_wire::{WireErrorKind, WirePath, decode_canonical, encode};

    use super::*;

    #[test]
    fn decodes_the_exact_empty_foundation_product() {
        let bytes = encode(&CanonicalMirFoundation::empty()).unwrap();
        let validated = decode_canonical::<DecodedMirFoundation>(&bytes).unwrap();
        let decoded = validated.decoded;

        assert!(decoded.exact_types.is_empty());
        assert!(decoded.generated_callables.is_empty());
        assert!(decoded.generated_types.is_empty());
        assert!(decoded.fields.is_empty());
        assert!(decoded.enum_variants.is_empty());
        assert!(decoded.enum_variant_fields.is_empty());
        assert!(decoded.callable_signatures.is_empty());
        assert!(decoded.local_values.is_empty());
        assert!(decoded.callback_applications.is_empty());
        assert!(decoded.callback_application_records.is_empty());
        assert!(decoded.odr_groups.is_empty());
        assert!(decoded.odr_members.is_empty());
    }

    #[test]
    fn registers_and_resolves_a_mir_identity_delta() {
        let nominal = CoreBuiltinNominal::Unit.identity_record().id();
        let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
        let mut canonical = CanonicalMirFoundation::empty();
        canonical.set_exact_types(vec![record.clone()]).unwrap();
        let decoded =
            decode_canonical::<DecodedMirFoundation>(&encode(&canonical).unwrap()).unwrap();
        let mut validation = PendingIdentityValidation::new();
        validation.register_authority(nominal).unwrap();

        decoded.register_identities(&mut validation).unwrap();
        decoded.resolve_identities(&mut validation).unwrap();

        let graph = validation.finish().unwrap();

        assert_eq!(
            graph
                .records::<PersistentExactTypeId, ExactTypeKey>(
                    IdentityLayer::Mir,
                    &WirePath::root(),
                )
                .unwrap(),
            vec![record]
        );
    }

    #[test]
    fn rejects_a_different_closed_product_length() {
        let mut bytes = encode(&CanonicalMirFoundation::empty()).unwrap();
        assert_eq!(bytes[0], 0xac);
        bytes[0] = 0xab;

        let error = decode_canonical::<DecodedMirFoundation>(&bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 12,
                actual: 11,
            }
        );
    }

    #[test]
    fn rejects_a_missing_or_reordered_table_field() {
        let mut bytes = encode(&CanonicalMirFoundation::empty()).unwrap();
        let second_field = bytes
            .windows(4)
            .position(|window| window == [1, 0x80, 2, 0x80])
            .unwrap()
            + 2;
        bytes[second_field] = 3;

        let error = decode_canonical::<DecodedMirFoundation>(&bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::UnexpectedField {
                expected: 2,
                actual: 3,
            }
        );
    }
}
