//! Untrusted Wire CBOR representation of the HIR identity foundation.

use scoop_identity::{
    DecodedCallableApplicationKey, DecodedCallbackRegistrationKey, DecodedCborIdentityRecord,
    DecodedDefinitionOriginRecord, DecodedDispatchSlotKey, DecodedEnumVariantFieldKey,
    DecodedEnumVariantIdentityKey, DecodedExactTypeKey, DecodedExportBindingKey,
    DecodedFieldIdentityKey, DecodedGeneratedCallableKey, DecodedGeneratedNominalKey,
    DecodedInitializationUnitKey, DecodedLocalBindingKey, DecodedLocalValueKey,
    DecodedOdrMemberKey, DecodedPropertyAccessorKey, DecodedSourceContextKey,
    DecodedSourceDeclarationKey, DecodedSourceNativeExternalContractRecord,
    DecodedSpecializationKey, IdentityLayer, IdentityValidationError, PendingIdentityValidation,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{DecodedNativeBoundaryTypeDefinitionRecord, DecodedSourceRecord};

mod validation;
pub use validation::{
    DefinitionOriginValidationError, HirFoundationValidationError, ValidatedHirFoundation,
};

type DecodedTypeRecord = DecodedCborIdentityRecord<PersistentTypeId, DecodedSourceDeclarationKey>;
type DecodedGenericTypeRecord =
    DecodedCborIdentityRecord<PersistentGenericTypeId, DecodedSourceDeclarationKey>;
type DecodedFunctionRecord =
    DecodedCborIdentityRecord<PersistentFunctionId, DecodedSourceDeclarationKey>;
type DecodedGenericFunctionRecord =
    DecodedCborIdentityRecord<PersistentGenericFunctionId, DecodedSourceDeclarationKey>;
type DecodedConstructorRecord =
    DecodedCborIdentityRecord<PersistentConstructorId, DecodedSourceDeclarationKey>;
type DecodedPropertyRecord =
    DecodedCborIdentityRecord<PersistentPropertyId, DecodedSourceDeclarationKey>;
type DecodedExtensionPropertyRecord =
    DecodedCborIdentityRecord<PersistentExtensionPropertyId, DecodedSourceDeclarationKey>;
type DecodedObjectValueRecord =
    DecodedCborIdentityRecord<PersistentObjectValueId, DecodedSourceDeclarationKey>;
type DecodedTypeAliasRecord =
    DecodedCborIdentityRecord<PersistentTypeAliasId, DecodedSourceDeclarationKey>;
type DecodedPropertyAccessorRecord =
    DecodedCborIdentityRecord<PersistentPropertyAccessorId, DecodedPropertyAccessorKey>;
type DecodedFieldRecord = DecodedCborIdentityRecord<PersistentFieldId, DecodedFieldIdentityKey>;
type DecodedEnumVariantRecord =
    DecodedCborIdentityRecord<PersistentEnumVariantId, DecodedEnumVariantIdentityKey>;
type DecodedEnumVariantFieldRecord =
    DecodedCborIdentityRecord<PersistentEnumVariantFieldId, DecodedEnumVariantFieldKey>;
type DecodedExactTypeRecord = DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>;
type DecodedExportBindingRecord =
    DecodedCborIdentityRecord<PersistentExportBindingId, DecodedExportBindingKey>;
type DecodedCallableApplicationRecord =
    DecodedCborIdentityRecord<PersistentCallableApplicationId, DecodedCallableApplicationKey>;
type DecodedGeneratedCallableRecord =
    DecodedCborIdentityRecord<PersistentGeneratedCallableId, DecodedGeneratedCallableKey>;
type DecodedGeneratedTypeRecord =
    DecodedCborIdentityRecord<PersistentTypeId, DecodedGeneratedNominalKey>;
type DecodedDispatchSlotRecord =
    DecodedCborIdentityRecord<PersistentDispatchSlotId, DecodedDispatchSlotKey>;
type DecodedInitializationUnitRecord =
    DecodedCborIdentityRecord<PersistentInitializationUnitId, DecodedInitializationUnitKey>;
type DecodedSourceContextRecord =
    DecodedCborIdentityRecord<PersistentSourceContextId, DecodedSourceContextKey>;
type DecodedLocalBindingRecord =
    DecodedCborIdentityRecord<PersistentLocalBindingId, DecodedLocalBindingKey>;
type DecodedLocalValueRecord =
    DecodedCborIdentityRecord<PersistentLocalValueId, DecodedLocalValueKey>;
type DecodedCallbackRegistrationRecord =
    DecodedCborIdentityRecord<PersistentCallbackRegistrationId, DecodedCallbackRegistrationKey>;
type DecodedOdrGroupRecord = DecodedCborIdentityRecord<OdrGroupId, DecodedSpecializationKey>;
type DecodedOdrMemberRecord = DecodedCborIdentityRecord<OdrMemberId, DecodedOdrMemberKey>;

/// Raw decoded graph. It stays private so unverified persistent-id bytes
/// cannot escape the public wire boundary.
#[derive(Debug)]
struct DecodedHirFoundationWire {
    sources: Vec<DecodedSourceRecord>,
    types: Vec<DecodedTypeRecord>,
    generic_types: Vec<DecodedGenericTypeRecord>,
    functions: Vec<DecodedFunctionRecord>,
    generic_functions: Vec<DecodedGenericFunctionRecord>,
    constructors: Vec<DecodedConstructorRecord>,
    properties: Vec<DecodedPropertyRecord>,
    extension_properties: Vec<DecodedExtensionPropertyRecord>,
    object_values: Vec<DecodedObjectValueRecord>,
    type_aliases: Vec<DecodedTypeAliasRecord>,
    property_accessors: Vec<DecodedPropertyAccessorRecord>,
    fields: Vec<DecodedFieldRecord>,
    enum_variants: Vec<DecodedEnumVariantRecord>,
    enum_variant_fields: Vec<DecodedEnumVariantFieldRecord>,
    exact_types: Vec<DecodedExactTypeRecord>,
    export_bindings: Vec<DecodedExportBindingRecord>,
    callable_applications: Vec<DecodedCallableApplicationRecord>,
    generated_callables: Vec<DecodedGeneratedCallableRecord>,
    generated_types: Vec<DecodedGeneratedTypeRecord>,
    dispatch_slots: Vec<DecodedDispatchSlotRecord>,
    initialization_units: Vec<DecodedInitializationUnitRecord>,
    source_contexts: Vec<DecodedSourceContextRecord>,
    local_bindings: Vec<DecodedLocalBindingRecord>,
    local_values: Vec<DecodedLocalValueRecord>,
    callback_registrations: Vec<DecodedCallbackRegistrationRecord>,
    source_native_contracts: Vec<DecodedSourceNativeExternalContractRecord>,
    odr_groups: Vec<DecodedOdrGroupRecord>,
    odr_members: Vec<DecodedOdrMemberRecord>,
    definition_origins: Vec<DecodedDefinitionOriginRecord>,
    native_boundary_types: Vec<DecodedNativeBoundaryTypeDefinitionRecord>,
}

/// Canonically decoded HIR foundation wire graph.
///
/// This boundary proves the exact closed Wire CBOR product and canonical byte
/// spelling. Its unverified identity bytes remain private; identity rehash,
/// reference closure, typed remap, and atomic commit are performed when this
/// temporary graph is converted into an imported HIR set.
#[derive(Debug)]
pub struct DecodedHirFoundation {
    decoded: DecodedHirFoundationWire,
}

impl WireEncode for DecodedHirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.decoded.encode(encoder)
    }
}

impl WireDecode for DecodedHirFoundation {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedHirFoundationWire::decode(decoder).map(|decoded| Self { decoded })
    }
}

impl DecodedHirFoundation {
    /// Registers every HIR-owned identity before any layer starts resolution.
    pub fn register_identities(
        &self,
        validation: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        macro_rules! register_tables {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.decoded.$table {
                    validation.register(IdentityLayer::Hir, record)?;
                })+
            };
        }

        register_tables!(
            types,
            generic_types,
            functions,
            generic_functions,
            constructors,
            properties,
            extension_properties,
            object_values,
            type_aliases,
            property_accessors,
            fields,
            enum_variants,
            enum_variant_fields,
            exact_types,
            export_bindings,
            callable_applications,
            generated_callables,
            generated_types,
            dispatch_slots,
            initialization_units,
            source_contexts,
            local_bindings,
            local_values,
            callback_registrations,
            odr_groups,
            odr_members,
        );
        for record in &self.decoded.source_native_contracts {
            validation.register_source_native_contract(IdentityLayer::Hir, record)?;
        }
        Ok(())
    }

    /// Resolves every HIR-owned identity after all three layers registered
    /// their candidates.
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

        // Source declarations need only typed ids. Resolving them first makes
        // their canonical keys available to owner-sensitive HIR identities.
        resolve_tables!(
            types,
            generic_types,
            functions,
            generic_functions,
            constructors,
            properties,
            extension_properties,
            object_values,
            type_aliases,
            property_accessors,
            exact_types,
            callable_applications,
            generated_callables,
            generated_types,
            dispatch_slots,
            initialization_units,
            source_contexts,
            local_values,
            odr_groups,
            odr_members,
        );

        // These keys reconstruct source or generated owners, so their owner
        // families must already have canonical keys.
        resolve_tables!(fields, enum_variants, enum_variant_fields);
        resolve_tables!(export_bindings, local_bindings);
        resolve_tables!(callback_registrations);
        for record in &self.decoded.source_native_contracts {
            validation.resolve_source_native_contract(record)?;
        }
        Ok(())
    }
}

impl WireEncode for DecodedHirFoundationWire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(30)?;
        encode_table_field(encoder, 1, &self.sources)?;
        encode_table_field(encoder, 2, &self.types)?;
        encode_table_field(encoder, 3, &self.generic_types)?;
        encode_table_field(encoder, 4, &self.functions)?;
        encode_table_field(encoder, 5, &self.generic_functions)?;
        encode_table_field(encoder, 6, &self.constructors)?;
        encode_table_field(encoder, 7, &self.properties)?;
        encode_table_field(encoder, 8, &self.extension_properties)?;
        encode_table_field(encoder, 9, &self.object_values)?;
        encode_table_field(encoder, 10, &self.type_aliases)?;
        encode_table_field(encoder, 11, &self.property_accessors)?;
        encode_table_field(encoder, 12, &self.fields)?;
        encode_table_field(encoder, 13, &self.enum_variants)?;
        encode_table_field(encoder, 14, &self.enum_variant_fields)?;
        encode_table_field(encoder, 15, &self.exact_types)?;
        encode_table_field(encoder, 16, &self.export_bindings)?;
        encode_table_field(encoder, 17, &self.callable_applications)?;
        encode_table_field(encoder, 18, &self.generated_callables)?;
        encode_table_field(encoder, 19, &self.generated_types)?;
        encode_table_field(encoder, 20, &self.dispatch_slots)?;
        encode_table_field(encoder, 21, &self.initialization_units)?;
        encode_table_field(encoder, 22, &self.source_contexts)?;
        encode_table_field(encoder, 23, &self.local_bindings)?;
        encode_table_field(encoder, 24, &self.local_values)?;
        encode_table_field(encoder, 25, &self.callback_registrations)?;
        encode_table_field(encoder, 26, &self.source_native_contracts)?;
        encode_table_field(encoder, 27, &self.odr_groups)?;
        encode_table_field(encoder, 28, &self.odr_members)?;
        encode_table_field(encoder, 29, &self.definition_origins)?;
        encode_table_field(encoder, 30, &self.native_boundary_types)
    }
}

impl WireDecode for DecodedHirFoundationWire {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(30)?;
        Ok(Self {
            sources: decode_table_field(decoder, 1)?,
            types: decode_table_field(decoder, 2)?,
            generic_types: decode_table_field(decoder, 3)?,
            functions: decode_table_field(decoder, 4)?,
            generic_functions: decode_table_field(decoder, 5)?,
            constructors: decode_table_field(decoder, 6)?,
            properties: decode_table_field(decoder, 7)?,
            extension_properties: decode_table_field(decoder, 8)?,
            object_values: decode_table_field(decoder, 9)?,
            type_aliases: decode_table_field(decoder, 10)?,
            property_accessors: decode_table_field(decoder, 11)?,
            fields: decode_table_field(decoder, 12)?,
            enum_variants: decode_table_field(decoder, 13)?,
            enum_variant_fields: decode_table_field(decoder, 14)?,
            exact_types: decode_table_field(decoder, 15)?,
            export_bindings: decode_table_field(decoder, 16)?,
            callable_applications: decode_table_field(decoder, 17)?,
            generated_callables: decode_table_field(decoder, 18)?,
            generated_types: decode_table_field(decoder, 19)?,
            dispatch_slots: decode_table_field(decoder, 20)?,
            initialization_units: decode_table_field(decoder, 21)?,
            source_contexts: decode_table_field(decoder, 22)?,
            local_bindings: decode_table_field(decoder, 23)?,
            local_values: decode_table_field(decoder, 24)?,
            callback_registrations: decode_table_field(decoder, 25)?,
            source_native_contracts: decode_table_field(decoder, 26)?,
            odr_groups: decode_table_field(decoder, 27)?,
            odr_members: decode_table_field(decoder, 28)?,
            definition_origins: decode_table_field(decoder, 29)?,
            native_boundary_types: decode_table_field(decoder, 30)?,
        })
    }
}

fn decode_table_field<T: WireDecode>(
    decoder: &mut Decoder<'_, '_>,
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
    use scoop_identity::{
        CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, PackagePath, SourceDeclarationKey, SourceDeclarationSite,
        SourceNominalKind,
    };
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::*;

    #[test]
    fn decodes_the_exact_empty_foundation_product() {
        let bytes = encode(&CanonicalHirFoundation::empty()).unwrap();
        let validated =
            decode_canonical::<DecodedHirFoundation>(&bytes, DecodeLimits::default()).unwrap();
        let decoded = validated.decoded;

        assert!(decoded.sources.is_empty());
        assert!(decoded.types.is_empty());
        assert!(decoded.generic_types.is_empty());
        assert!(decoded.functions.is_empty());
        assert!(decoded.generic_functions.is_empty());
        assert!(decoded.constructors.is_empty());
        assert!(decoded.properties.is_empty());
        assert!(decoded.extension_properties.is_empty());
        assert!(decoded.object_values.is_empty());
        assert!(decoded.type_aliases.is_empty());
        assert!(decoded.property_accessors.is_empty());
        assert!(decoded.fields.is_empty());
        assert!(decoded.enum_variants.is_empty());
        assert!(decoded.enum_variant_fields.is_empty());
        assert!(decoded.exact_types.is_empty());
        assert!(decoded.export_bindings.is_empty());
        assert!(decoded.callable_applications.is_empty());
        assert!(decoded.generated_callables.is_empty());
        assert!(decoded.generated_types.is_empty());
        assert!(decoded.dispatch_slots.is_empty());
        assert!(decoded.initialization_units.is_empty());
        assert!(decoded.source_contexts.is_empty());
        assert!(decoded.local_bindings.is_empty());
        assert!(decoded.local_values.is_empty());
        assert!(decoded.callback_registrations.is_empty());
        assert!(decoded.source_native_contracts.is_empty());
        assert!(decoded.odr_groups.is_empty());
        assert!(decoded.odr_members.is_empty());
        assert!(decoded.definition_origins.is_empty());
        assert!(decoded.native_boundary_types.is_empty());
    }

    #[test]
    fn registers_and_resolves_a_hir_identity_delta() {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Widget").unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let record = CborIdentityRecord::from_key(declaration).unwrap();
        let mut canonical = CanonicalHirFoundation::empty();
        canonical.set_types(vec![record.clone()]).unwrap();
        let decoded = decode_canonical::<DecodedHirFoundation>(
            &encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut validation = PendingIdentityValidation::new();
        validation.register_authority(ConeIdentity::CORE).unwrap();

        decoded.register_identities(&mut validation).unwrap();
        decoded.resolve_identities(&mut validation).unwrap();

        let graph = validation.finish().unwrap();
        assert_eq!(
            graph
                .records::<PersistentTypeId, SourceDeclarationKey>(IdentityLayer::Hir)
                .unwrap(),
            vec![record]
        );
    }

    #[test]
    fn rejects_a_different_closed_product_length() {
        let mut bytes = encode(&CanonicalHirFoundation::empty()).unwrap();
        assert_eq!(&bytes[..2], &[0xb8, 30]);
        bytes[1] = 29;

        let error =
            decode_canonical::<DecodedHirFoundation>(&bytes, DecodeLimits::default()).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 30,
                actual: 29,
            }
        );
    }

    #[test]
    fn rejects_a_missing_or_reordered_table_field() {
        let mut bytes = encode(&CanonicalHirFoundation::empty()).unwrap();
        let second_field = bytes
            .windows(4)
            .position(|window| window == [1, 0x80, 2, 0x80])
            .unwrap()
            + 2;
        bytes[second_field] = 3;

        let error =
            decode_canonical::<DecodedHirFoundation>(&bytes, DecodeLimits::default()).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::UnexpectedField {
                expected: 2,
                actual: 3,
            }
        );
    }
}
