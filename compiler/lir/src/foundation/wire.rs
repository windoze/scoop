//! Untrusted Wire CBOR representation of the LIR identity foundation.

use std::collections::BTreeSet;

use scoop_identity::{
    DecodedCanonicalCAbiLayoutFingerprintRecord, DecodedCanonicalCAbiSignatureFingerprintRecord,
    DecodedCborIdentityRecord, DecodedDispatchTableKey, DecodedExactTypeKey,
    DecodedGeneratedBridgeAtomKey, DecodedGeneratedBridgeUnitKey, DecodedImmortalObjectKey,
    DecodedLayoutKey, DecodedNativeExternalContractRecord, DecodedNativeLinkRequirementKey,
    DecodedObjectDefinitionAtomKey, DecodedObjectDefinitionPlanKey, DecodedOdrMemberKey,
    DecodedPersistentId, DecodedPersistentSymbolRequestTable, DecodedRuntimeIdentityRecord,
    DecodedSafepointSiteKey, DecodedScanKey, DecodedSpecializationKey, DecodedStaticStorageKey,
    IdentityLayer, IdentityValidationError, PendingIdentityValidation,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{
    DecodedCallbackBridgeRecord, DecodedRuntimeTypeMappingRecord, DecodedSafepointMappingRecord,
};

mod validation;
pub use validation::{
    BridgeRelationError, LirFoundationOwnershipError, LirFoundationValidationError,
    NativeContractRelationError, SafepointRelationError, ValidatedLirFoundation,
};

type DecodedExactTypeRecord = DecodedCborIdentityRecord<PersistentExactTypeId, DecodedExactTypeKey>;
type DecodedLayoutRecord = DecodedCborIdentityRecord<PersistentLayoutId, DecodedLayoutKey>;
type DecodedScanRecord = DecodedCborIdentityRecord<PersistentScanId, DecodedScanKey>;
type DecodedDispatchTableRecord =
    DecodedCborIdentityRecord<PersistentDispatchTableId, DecodedDispatchTableKey>;
type DecodedStaticStorageRecord =
    DecodedCborIdentityRecord<PersistentStaticStorageId, DecodedStaticStorageKey>;
type DecodedImmortalObjectRecord =
    DecodedCborIdentityRecord<PersistentImmortalObjectId, DecodedImmortalObjectKey>;
type DecodedOdrGroupRecord = DecodedCborIdentityRecord<OdrGroupId, DecodedSpecializationKey>;
type DecodedOdrMemberRecord = DecodedCborIdentityRecord<OdrMemberId, DecodedOdrMemberKey>;
type DecodedCallableBodyRecord = DecodedRuntimeIdentityRecord<PersistentCallableBodyId>;
type DecodedSafepointSiteRecord =
    DecodedCborIdentityRecord<PersistentSafepointSiteId, DecodedSafepointSiteKey>;
type DecodedBridgeUnitRecord =
    DecodedCborIdentityRecord<GeneratedBridgeUnitId, DecodedGeneratedBridgeUnitKey>;
type DecodedBridgeAtomRecord =
    DecodedCborIdentityRecord<GeneratedBridgeAtomId, DecodedGeneratedBridgeAtomKey>;
type DecodedNativeLinkRequirementRecord =
    DecodedCborIdentityRecord<NativeLinkRequirementId, DecodedNativeLinkRequirementKey>;
type DecodedDefinitionPlanRecord =
    DecodedCborIdentityRecord<ObjectDefinitionPlanId, DecodedObjectDefinitionPlanKey>;
type DecodedDefinitionAtomRecord =
    DecodedCborIdentityRecord<ObjectDefinitionAtomId, DecodedObjectDefinitionAtomKey>;

#[derive(Debug)]
struct DecodedLirFoundationWire {
    exact_types: Vec<DecodedExactTypeRecord>,
    layouts: Vec<DecodedLayoutRecord>,
    scans: Vec<DecodedScanRecord>,
    dispatch_tables: Vec<DecodedDispatchTableRecord>,
    static_storages: Vec<DecodedStaticStorageRecord>,
    immortal_objects: Vec<DecodedImmortalObjectRecord>,
    odr_groups: Vec<DecodedOdrGroupRecord>,
    odr_members: Vec<DecodedOdrMemberRecord>,
    callable_bodies: Vec<DecodedCallableBodyRecord>,
    safepoint_sites: Vec<DecodedSafepointSiteRecord>,
    runtime_types: Vec<DecodedRuntimeTypeMappingRecord>,
    safepoints: Vec<DecodedSafepointMappingRecord>,
    symbol_requests: DecodedPersistentSymbolRequestTable,
    native_contracts: Vec<DecodedNativeExternalContractRecord>,
    c_abi_signatures: Vec<DecodedCanonicalCAbiSignatureFingerprintRecord>,
    c_abi_layouts: Vec<DecodedCanonicalCAbiLayoutFingerprintRecord>,
    bridge_units: Vec<DecodedBridgeUnitRecord>,
    bridge_atoms: Vec<DecodedBridgeAtomRecord>,
    callback_bridges: Vec<DecodedCallbackBridgeRecord>,
    native_link_requirements: Vec<DecodedNativeLinkRequirementRecord>,
    definition_plans: Vec<DecodedDefinitionPlanRecord>,
    definition_atoms: Vec<DecodedDefinitionAtomRecord>,
}

/// Canonically decoded LIR foundation wire graph.
///
/// This boundary proves the exact closed Wire CBOR product and canonical byte
/// spelling. Unverified identity bytes remain private until cross-layer
/// identity validation, typed remap, and atomic commit all succeed.
#[derive(Debug)]
pub struct DecodedLirFoundation {
    decoded: DecodedLirFoundationWire,
}

impl WireEncode for DecodedLirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.decoded.encode(encoder)
    }
}

impl WireDecode for DecodedLirFoundation {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedLirFoundationWire::decode(decoder).map(|decoded| Self { decoded })
    }
}

impl DecodedLirFoundation {
    /// Registers every LIR-owned identity before cross-layer resolution.
    pub fn register_identities(
        &self,
        validation: &mut PendingIdentityValidation,
    ) -> Result<(), IdentityValidationError> {
        for record in &self.decoded.c_abi_signatures {
            let fingerprint = record.candidate_fingerprint().map_err(|error| {
                invalid_leaf(
                    record.decoded_fingerprint(),
                    format!("invalid canonical C ABI signature fingerprint: {error}"),
                )
            })?;
            verify_leaf(record.decoded_fingerprint(), fingerprint)?;
            validation.register_verified_leaf(fingerprint)?;
        }
        for record in &self.decoded.c_abi_layouts {
            let fingerprint = record.candidate_fingerprint().map_err(|error| {
                invalid_leaf(
                    record.decoded_fingerprint(),
                    format!("invalid canonical C ABI layout fingerprint: {error}"),
                )
            })?;
            verify_leaf(record.decoded_fingerprint(), fingerprint)?;
            validation.register_verified_leaf(fingerprint)?;
        }
        let mut native_fingerprints = BTreeSet::new();
        for record in &self.decoded.native_contracts {
            let fingerprint = record.candidate_fingerprint().map_err(|error| {
                invalid_leaf(
                    record.decoded_fingerprint(),
                    format!("invalid native external contract fingerprint: {error}"),
                )
            })?;
            verify_leaf(record.decoded_fingerprint(), fingerprint)?;
            native_fingerprints.insert(fingerprint);
        }
        for fingerprint in native_fingerprints {
            validation.register_verified_leaf(fingerprint)?;
        }

        macro_rules! register_tables {
            ($($table:ident),+ $(,)?) => {
                $(for record in &self.decoded.$table {
                    validation.register(IdentityLayer::Lir, record)?;
                })+
            };
        }

        register_tables!(
            exact_types,
            layouts,
            scans,
            dispatch_tables,
            static_storages,
            immortal_objects,
            odr_groups,
            odr_members,
            safepoint_sites,
            bridge_units,
            bridge_atoms,
            native_link_requirements,
            definition_plans,
            definition_atoms,
        );
        for record in &self.decoded.callable_bodies {
            validation.register_callable_body(IdentityLayer::Lir, record)?;
        }
        Ok(())
    }

    /// Resolves every LIR-owned identity after HIR and MIR identities supplied
    /// all earlier-layer keys.
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
            layouts,
            scans,
            dispatch_tables,
            static_storages,
            immortal_objects,
            odr_groups,
            odr_members,
        );
        // ODR callable bodies reconstruct a refined member key.
        for record in &self.decoded.callable_bodies {
            validation.resolve_callable_body(record)?;
        }
        resolve_tables!(
            safepoint_sites,
            bridge_units,
            bridge_atoms,
            native_link_requirements,
        );
        // A strong definition plan may reconstruct a generated bridge atom;
        // definition atoms in turn depend on their plans.
        resolve_tables!(definition_plans, definition_atoms);
        Ok(())
    }
}

fn verify_leaf<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
) -> Result<(), IdentityValidationError> {
    decoded.verify(expected).map(drop).map_err(|mismatch| {
        IdentityValidationError::IdentityMismatch {
            kind: I::KIND,
            expected: *mismatch.expected().as_array(),
            actual: *mismatch.actual(),
        }
    })
}

fn invalid_leaf<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    reason: String,
) -> IdentityValidationError {
    IdentityValidationError::InvalidRecord {
        kind: I::KIND,
        id: *decoded.as_array(),
        reason,
    }
}

impl WireEncode for DecodedLirFoundationWire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(22)?;
        encode_table_field(encoder, 1, &self.exact_types)?;
        encode_table_field(encoder, 2, &self.layouts)?;
        encode_table_field(encoder, 3, &self.scans)?;
        encode_table_field(encoder, 4, &self.dispatch_tables)?;
        encode_table_field(encoder, 5, &self.static_storages)?;
        encode_table_field(encoder, 6, &self.immortal_objects)?;
        encode_table_field(encoder, 7, &self.odr_groups)?;
        encode_table_field(encoder, 8, &self.odr_members)?;
        encode_table_field(encoder, 9, &self.callable_bodies)?;
        encode_table_field(encoder, 10, &self.safepoint_sites)?;
        encode_table_field(encoder, 11, &self.runtime_types)?;
        encode_table_field(encoder, 12, &self.safepoints)?;
        encoder.field(13)?;
        self.symbol_requests.encode(encoder)?;
        encode_table_field(encoder, 14, &self.native_contracts)?;
        encode_table_field(encoder, 15, &self.c_abi_signatures)?;
        encode_table_field(encoder, 16, &self.c_abi_layouts)?;
        encode_table_field(encoder, 17, &self.bridge_units)?;
        encode_table_field(encoder, 18, &self.bridge_atoms)?;
        encode_table_field(encoder, 19, &self.callback_bridges)?;
        encode_table_field(encoder, 20, &self.native_link_requirements)?;
        encode_table_field(encoder, 21, &self.definition_plans)?;
        encode_table_field(encoder, 22, &self.definition_atoms)
    }
}

impl WireDecode for DecodedLirFoundationWire {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(22)?;
        Ok(Self {
            exact_types: decode_table_field(decoder, 1)?,
            layouts: decode_table_field(decoder, 2)?,
            scans: decode_table_field(decoder, 3)?,
            dispatch_tables: decode_table_field(decoder, 4)?,
            static_storages: decode_table_field(decoder, 5)?,
            immortal_objects: decode_table_field(decoder, 6)?,
            odr_groups: decode_table_field(decoder, 7)?,
            odr_members: decode_table_field(decoder, 8)?,
            callable_bodies: decode_table_field(decoder, 9)?,
            safepoint_sites: decode_table_field(decoder, 10)?,
            runtime_types: decode_table_field(decoder, 11)?,
            safepoints: decode_table_field(decoder, 12)?,
            symbol_requests: decoder.field(13, DecodedPersistentSymbolRequestTable::decode)?,
            native_contracts: decode_table_field(decoder, 14)?,
            c_abi_signatures: decode_table_field(decoder, 15)?,
            c_abi_layouts: decode_table_field(decoder, 16)?,
            bridge_units: decode_table_field(decoder, 17)?,
            bridge_atoms: decode_table_field(decoder, 18)?,
            callback_bridges: decode_table_field(decoder, 19)?,
            native_link_requirements: decode_table_field(decoder, 20)?,
            definition_plans: decode_table_field(decoder, 21)?,
            definition_atoms: decode_table_field(decoder, 22)?,
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
        CallableBodyKey, CanonicalCAbiFunctionSignature, CanonicalCAbiReturn,
        CanonicalCAbiSignatureFingerprintRecord, CanonicalIdentifier, CborIdentityRecord,
        ConeIdentity, DeclarationScope, DefinitionOwnerChain, GeneratedBridgeUnitKey,
        NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
        NativeLibraryBinding, PackagePath, PersistentFunctionId,
        PersistentSourceNativeExternalContractId, RuntimeIdentityRecord, SourceDeclarationKey,
        SourceDeclarationSite, SourceNativeExternalContractKey, SourceNativeSymbol,
        StrongCallableDefinitionOwner,
    };
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::*;

    #[test]
    fn decodes_the_exact_empty_foundation_product() {
        let bytes = encode(&CanonicalLirFoundation::empty()).unwrap();
        let validated =
            decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();
        let decoded = validated.decoded;

        assert!(decoded.exact_types.is_empty());
        assert!(decoded.layouts.is_empty());
        assert!(decoded.scans.is_empty());
        assert!(decoded.dispatch_tables.is_empty());
        assert!(decoded.static_storages.is_empty());
        assert!(decoded.immortal_objects.is_empty());
        assert!(decoded.odr_groups.is_empty());
        assert!(decoded.odr_members.is_empty());
        assert!(decoded.callable_bodies.is_empty());
        assert!(decoded.safepoint_sites.is_empty());
        assert!(decoded.runtime_types.is_empty());
        assert!(decoded.safepoints.is_empty());
        assert!(decoded.native_contracts.is_empty());
        assert!(decoded.c_abi_signatures.is_empty());
        assert!(decoded.c_abi_layouts.is_empty());
        assert!(decoded.bridge_units.is_empty());
        assert!(decoded.bridge_atoms.is_empty());
        assert!(decoded.callback_bridges.is_empty());
        assert!(decoded.native_link_requirements.is_empty());
        assert!(decoded.definition_plans.is_empty());
        assert!(decoded.definition_atoms.is_empty());
    }

    #[test]
    fn registers_and_resolves_a_lir_runtime_identity_delta() {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        let record = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(function),
        ))
        .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_callable_bodies(vec![record.clone()]).unwrap();
        let decoded = decode_canonical::<DecodedLirFoundation>(
            &encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut validation = PendingIdentityValidation::new();
        validation.register_authority(function).unwrap();

        decoded.register_identities(&mut validation).unwrap();
        decoded.resolve_identities(&mut validation).unwrap();

        let graph = validation.finish().unwrap();
        assert_eq!(
            graph
                .runtime_records::<PersistentCallableBodyId, CallableBodyKey>(IdentityLayer::Lir)
                .unwrap(),
            vec![record]
        );
    }

    #[test]
    fn verified_signature_fingerprint_is_available_to_bridge_identity_resolution() {
        let signature =
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void);
        let signature = CanonicalCAbiSignatureFingerprintRecord::new(signature).unwrap();
        let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: signature.fingerprint(),
            context_index: scoop_identity::CallbackParameterIndex::new(0),
        })
        .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_c_abi_signatures(vec![signature]).unwrap();
        canonical.set_bridge_units(vec![unit.clone()]).unwrap();
        let decoded = decode_canonical::<DecodedLirFoundation>(
            &encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut validation = PendingIdentityValidation::new();

        decoded.register_identities(&mut validation).unwrap();
        decoded.resolve_identities(&mut validation).unwrap();

        let graph = validation.finish().unwrap();
        assert_eq!(
            graph
                .records::<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>(IdentityLayer::Lir)
                .unwrap(),
            vec![unit]
        );
    }

    #[test]
    fn verified_native_contract_fingerprint_is_available_to_bridge_identity_resolution() {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("native").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let source = PersistentSourceNativeExternalContractId::from_key(
            &SourceNativeExternalContractKey::function(&declaration).unwrap(),
        )
        .unwrap();
        let signature =
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void);
        let contract = NativeExternalContractRecord::new(
            source,
            NativeExternalSymbolKey::darwin_macho_external(
                &SourceNativeSymbol::new("native").unwrap(),
            )
            .unwrap(),
            NativeExternalContract::c_function(
                NativeLibraryBinding::DefaultNativeNamespace,
                signature,
            ),
        )
        .unwrap();
        let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::OutboundFunction(
            contract.fingerprint(),
        ))
        .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_native_contracts(vec![contract]).unwrap();
        canonical.set_bridge_units(vec![unit.clone()]).unwrap();
        let decoded = decode_canonical::<DecodedLirFoundation>(
            &encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut validation = PendingIdentityValidation::new();

        decoded.register_identities(&mut validation).unwrap();
        decoded.resolve_identities(&mut validation).unwrap();

        assert_eq!(
            validation
                .finish()
                .unwrap()
                .records::<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>(IdentityLayer::Lir)
                .unwrap(),
            vec![unit]
        );
    }

    #[test]
    fn rejects_an_unverified_signature_fingerprint_leaf() {
        let signature = CanonicalCAbiSignatureFingerprintRecord::new(
            CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
        )
        .unwrap();
        let fingerprint = *signature.fingerprint().as_array();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_c_abi_signatures(vec![signature]).unwrap();
        let mut bytes = encode(&canonical).unwrap();
        let offset = bytes
            .windows(fingerprint.len())
            .position(|window| window == fingerprint)
            .unwrap();
        bytes[offset] ^= 1;
        let decoded =
            decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap();

        assert!(matches!(
            decoded.register_identities(&mut PendingIdentityValidation::new()),
            Err(IdentityValidationError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn rejects_a_different_closed_product_length() {
        let mut bytes = encode(&CanonicalLirFoundation::empty()).unwrap();
        assert_eq!(&bytes[..2], &[0xb6, 1]);
        bytes[0] = 0xb5;

        let error =
            decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 22,
                actual: 21,
            }
        );
    }

    #[test]
    fn rejects_a_missing_or_reordered_table_field() {
        let mut bytes = encode(&CanonicalLirFoundation::empty()).unwrap();
        let second_field = bytes
            .windows(4)
            .position(|window| window == [1, 0x80, 2, 0x80])
            .unwrap()
            + 2;
        bytes[second_field] = 3;

        let error =
            decode_canonical::<DecodedLirFoundation>(&bytes, DecodeLimits::default()).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::UnexpectedField {
                expected: 2,
                actual: 3,
            }
        );
    }
}
