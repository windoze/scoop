use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash_stream_length};

use crate::ids::derive_persistent_id;
use crate::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, PersistentExactTypeId,
    PersistentFieldId,
};

mod decode;

const C_ABI_SIGNATURE_HASH_DOMAIN: &str = "scoop-c-abi-signature-v1";
const C_ABI_LAYOUT_HASH_DOMAIN: &str = "scoop-c-abi-layout-v1";

pub use decode::{
    CanonicalCAbiResolutionError, DecodedCDataPointee, DecodedCLayoutOverride,
    DecodedCPointerStorage, DecodedCanonicalCAbiFunctionSignature, DecodedCanonicalCAbiLayout,
    DecodedCanonicalCAbiLayoutField, DecodedCanonicalCAbiLayoutFingerprintRecord,
    DecodedCanonicalCAbiParameter, DecodedCanonicalCAbiReturn,
    DecodedCanonicalCAbiSignatureFingerprintRecord, DecodedCanonicalCStorageType,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Signedness {
    Signed,
    Unsigned,
}

impl WireEncode for Signedness {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Signed => 1,
            Self::Unsigned => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntegerBitWidth {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

impl IntegerBitWidth {
    pub const fn get(self) -> u8 {
        match self {
            Self::Bits8 => 8,
            Self::Bits16 => 16,
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

impl WireEncode for IntegerBitWidth {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.get()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CDataPointee {
    OpaqueUnit,
    ExactObject(PersistentExactTypeId),
}

impl WireEncode for CDataPointee {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OpaqueUnit => encode_empty_sum(encoder, 1),
            Self::ExactObject(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CPointerStorage {
    Direct,
    NullableWrapper(PersistentExactTypeId),
}

impl WireEncode for CPointerStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct => encode_empty_sum(encoder, 1),
            Self::NullableWrapper(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalCStorageType {
    Integer {
        exact_type: PersistentExactTypeId,
        signedness: Signedness,
        bit_width: IntegerBitWidth,
    },
    Boolean {
        exact_type: PersistentExactTypeId,
    },
    DataPointer {
        exact_type: PersistentExactTypeId,
        pointee: CDataPointee,
        storage: CPointerStorage,
    },
    CodePointer {
        exact_type: PersistentExactTypeId,
        storage: CPointerStorage,
    },
    Struct {
        exact_type: PersistentExactTypeId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
}

impl CanonicalCStorageType {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Integer { exact_type, .. }
            | Self::Boolean { exact_type }
            | Self::DataPointer { exact_type, .. }
            | Self::CodePointer { exact_type, .. }
            | Self::Struct { exact_type, .. } => exact_type,
        }
    }
}

impl WireEncode for CanonicalCStorageType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Integer {
                exact_type,
                signedness,
                bit_width,
            } => encode_three_value_sum(encoder, 1, exact_type, signedness, bit_width),
            Self::Boolean { exact_type } => encode_value_sum(encoder, 2, exact_type),
            Self::DataPointer {
                exact_type,
                pointee,
                storage,
            } => encode_three_value_sum(encoder, 3, exact_type, pointee, storage),
            Self::CodePointer {
                exact_type,
                storage,
            } => encode_two_value_sum(encoder, 4, exact_type, storage),
            Self::Struct { exact_type, layout } => {
                encode_two_value_sum(encoder, 5, exact_type, layout)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiParameter {
    source_exact_type: PersistentExactTypeId,
    storage: CanonicalCStorageType,
}

impl CanonicalCAbiParameter {
    pub fn new(
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageType,
    ) -> Result<Self, CanonicalCAbiError> {
        require_matching_exact_type(source_exact_type, storage)?;
        Ok(Self {
            source_exact_type,
            storage,
        })
    }

    pub const fn storage(self) -> CanonicalCStorageType {
        self.storage
    }

    pub const fn source_exact_type(&self) -> PersistentExactTypeId {
        self.source_exact_type
    }
}

impl WireEncode for CanonicalCAbiParameter {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source_exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.storage.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalCAbiReturn {
    Void,
    Value {
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageType,
    },
}

impl CanonicalCAbiReturn {
    pub fn value(
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageType,
    ) -> Result<Self, CanonicalCAbiError> {
        require_matching_exact_type(source_exact_type, storage)?;
        Ok(Self::Value {
            source_exact_type,
            storage,
        })
    }
}

impl WireEncode for CanonicalCAbiReturn {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Void => encode_empty_sum(encoder, 1),
            Self::Value {
                source_exact_type,
                storage,
            } => encode_two_value_sum(encoder, 2, source_exact_type, storage),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetCallingConvention {
    Cdecl,
}

impl WireEncode for TargetCallingConvention {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiFunctionSignature {
    calling_convention: TargetCallingConvention,
    parameters: Vec<CanonicalCAbiParameter>,
    result: CanonicalCAbiReturn,
}

impl CanonicalCAbiFunctionSignature {
    pub fn cdecl(parameters: Vec<CanonicalCAbiParameter>, result: CanonicalCAbiReturn) -> Self {
        Self {
            calling_convention: TargetCallingConvention::Cdecl,
            parameters,
            result,
        }
    }

    pub const fn calling_convention(&self) -> TargetCallingConvention {
        self.calling_convention
    }

    pub fn parameters(&self) -> &[CanonicalCAbiParameter] {
        &self.parameters
    }

    pub const fn result(&self) -> CanonicalCAbiReturn {
        self.result
    }
}

impl WireEncode for CanonicalCAbiFunctionSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        encoder.field(3)?;
        self.result.encode(encoder)
    }
}

impl CanonicalCAbiSignatureFingerprint {
    pub fn from_signature(signature: &CanonicalCAbiFunctionSignature) -> Result<Self, HashError> {
        derive_persistent_id(C_ABI_SIGNATURE_HASH_DOMAIN, signature)
    }

    pub fn hash_stream_length(
        signature: &CanonicalCAbiFunctionSignature,
    ) -> Result<u64, HashError> {
        domain_separated_cbor_hash_stream_length(C_ABI_SIGNATURE_HASH_DOMAIN, signature)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCAbiSignatureFingerprintRecord {
    fingerprint: CanonicalCAbiSignatureFingerprint,
    signature: CanonicalCAbiFunctionSignature,
}

impl CanonicalCAbiSignatureFingerprintRecord {
    pub fn new(signature: CanonicalCAbiFunctionSignature) -> Result<Self, HashError> {
        let fingerprint = CanonicalCAbiSignatureFingerprint::from_signature(&signature)?;
        Ok(Self {
            fingerprint,
            signature,
        })
    }

    pub const fn fingerprint(&self) -> CanonicalCAbiSignatureFingerprint {
        self.fingerprint
    }

    pub const fn signature(&self) -> &CanonicalCAbiFunctionSignature {
        &self.signature
    }

    pub(crate) const fn from_verified(
        fingerprint: CanonicalCAbiSignatureFingerprint,
        signature: CanonicalCAbiFunctionSignature,
    ) -> Self {
        Self {
            fingerprint,
            signature,
        }
    }
}

impl WireEncode for CanonicalCAbiSignatureFingerprintRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CLayoutByteAlignment {
    Bytes1,
    Bytes2,
    Bytes4,
    Bytes8,
    Bytes16,
}

impl CLayoutByteAlignment {
    pub const fn get(self) -> u8 {
        match self {
            Self::Bytes1 => 1,
            Self::Bytes2 => 2,
            Self::Bytes4 => 4,
            Self::Bytes8 => 8,
            Self::Bytes16 => 16,
        }
    }
}

impl WireEncode for CLayoutByteAlignment {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.get()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CLayoutOverride {
    Natural,
    Bytes(CLayoutByteAlignment),
}

impl WireEncode for CLayoutOverride {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Natural => encode_empty_sum(encoder, 1),
            Self::Bytes(bytes) => encode_value_sum(encoder, 2, bytes),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiLayoutField {
    field: PersistentFieldId,
    offset: u64,
    storage: CanonicalCStorageType,
}

impl CanonicalCAbiLayoutField {
    pub const fn new(
        field: PersistentFieldId,
        offset: u64,
        storage: CanonicalCStorageType,
    ) -> Self {
        Self {
            field,
            offset,
            storage,
        }
    }

    pub const fn field(self) -> PersistentFieldId {
        self.field
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn storage(self) -> CanonicalCStorageType {
        self.storage
    }
}

impl WireEncode for CanonicalCAbiLayoutField {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.offset)?;
        encoder.field(3)?;
        self.storage.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiLayout {
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    alignment: NonZeroU64,
    aligned: CLayoutOverride,
    packed: CLayoutOverride,
    fields: Vec<CanonicalCAbiLayoutField>,
}

impl CanonicalCAbiLayout {
    pub fn new(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        alignment: NonZeroU64,
        aligned: CLayoutOverride,
        packed: CLayoutOverride,
        fields: Vec<CanonicalCAbiLayoutField>,
    ) -> Self {
        Self {
            exact_type,
            byte_size,
            alignment,
            aligned,
            packed,
            fields,
        }
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    pub const fn alignment(&self) -> NonZeroU64 {
        self.alignment
    }

    pub const fn aligned(&self) -> CLayoutOverride {
        self.aligned
    }

    pub const fn packed(&self) -> CLayoutOverride {
        self.packed
    }

    pub fn fields(&self) -> &[CanonicalCAbiLayoutField] {
        &self.fields
    }
}

impl WireEncode for CanonicalCAbiLayout {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_size)?;
        encoder.field(3)?;
        encoder.unsigned(self.alignment.get())?;
        encoder.field(4)?;
        self.aligned.encode(encoder)?;
        encoder.field(5)?;
        self.packed.encode(encoder)?;
        encoder.field(6)?;
        encoder.array(self.fields.len() as u64)?;
        for field in &self.fields {
            field.encode(encoder)?;
        }
        Ok(())
    }
}

impl CanonicalCAbiLayoutFingerprint {
    pub fn from_layout(layout: &CanonicalCAbiLayout) -> Result<Self, HashError> {
        derive_persistent_id(C_ABI_LAYOUT_HASH_DOMAIN, layout)
    }

    pub fn hash_stream_length(layout: &CanonicalCAbiLayout) -> Result<u64, HashError> {
        domain_separated_cbor_hash_stream_length(C_ABI_LAYOUT_HASH_DOMAIN, layout)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCAbiLayoutFingerprintRecord {
    fingerprint: CanonicalCAbiLayoutFingerprint,
    layout: CanonicalCAbiLayout,
}

impl CanonicalCAbiLayoutFingerprintRecord {
    pub fn new(layout: CanonicalCAbiLayout) -> Result<Self, HashError> {
        let fingerprint = CanonicalCAbiLayoutFingerprint::from_layout(&layout)?;
        Ok(Self {
            fingerprint,
            layout,
        })
    }

    pub const fn fingerprint(&self) -> CanonicalCAbiLayoutFingerprint {
        self.fingerprint
    }

    pub const fn layout(&self) -> &CanonicalCAbiLayout {
        &self.layout
    }

    pub(crate) const fn from_verified(
        fingerprint: CanonicalCAbiLayoutFingerprint,
        layout: CanonicalCAbiLayout,
    ) -> Self {
        Self {
            fingerprint,
            layout,
        }
    }
}

impl WireEncode for CanonicalCAbiLayoutFingerprintRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.layout.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalCAbiError {
    StorageExactTypeMismatch,
}

impl fmt::Display for CanonicalCAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("canonical C storage exact type does not match its source exact type")
    }
}

impl std::error::Error for CanonicalCAbiError {}

fn require_matching_exact_type(
    source_exact_type: PersistentExactTypeId,
    storage: CanonicalCStorageType,
) -> Result<(), CanonicalCAbiError> {
    if storage.exact_type() == source_exact_type {
        Ok(())
    } else {
        Err(CanonicalCAbiError::StorageExactTypeMismatch)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_three_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
    third: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)?;
    encoder.field(3)?;
    third.encode(encoder)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use scoop_wire::encode;

    use super::{
        CLayoutOverride, CPointerStorage, CanonicalCAbiError, CanonicalCAbiFunctionSignature,
        CanonicalCAbiLayout, CanonicalCAbiLayoutField, CanonicalCAbiParameter, CanonicalCAbiReturn,
        CanonicalCStorageType,
    };
    use crate::{
        CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, ConeIdentity,
        PersistentExactTypeId, PersistentFieldId,
    };

    #[test]
    fn parameter_rejects_mismatched_storage_exact_type() {
        let source = PersistentExactTypeId(ConeIdentity::CORE.0);
        let other = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        assert_eq!(
            CanonicalCAbiParameter::new(
                source,
                CanonicalCStorageType::Boolean { exact_type: other },
            ),
            Err(CanonicalCAbiError::StorageExactTypeMismatch)
        );
    }

    #[test]
    fn canonical_c_signature_has_fixed_fingerprint() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let parameter = CanonicalCAbiParameter::new(
            exact,
            CanonicalCStorageType::Boolean { exact_type: exact },
        )
        .unwrap();
        let signature =
            CanonicalCAbiFunctionSignature::cdecl(vec![parameter], CanonicalCAbiReturn::Void);
        let encoded = encode(&signature).unwrap();
        assert_eq!(
            hex(&encoded),
            "a301010281a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02a200020158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d03a10001"
        );
        assert_eq!(
            CanonicalCAbiSignatureFingerprint::hash_stream_length(&signature).unwrap(),
            8 + "scoop-c-abi-signature-v1".len() as u64 + encoded.len() as u64
        );
        assert_eq!(
            CanonicalCAbiSignatureFingerprint::from_signature(&signature)
                .unwrap()
                .to_string(),
            "2d8d9551d5ef127d0ac16956ec75845622d4eb5e7c02cde8c3854f574bb5abec"
        );
    }

    #[test]
    fn canonical_c_layout_has_fixed_fingerprint() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let layout = CanonicalCAbiLayout::new(
            exact,
            0,
            NonZeroU64::new(1).unwrap(),
            CLayoutOverride::Natural,
            CLayoutOverride::Natural,
            Vec::new(),
        );
        let encoded = encode(&layout).unwrap();
        assert_eq!(
            hex(&encoded),
            format!("a6015820{exact}0200030104a1000105a100010680")
        );
        assert_eq!(
            CanonicalCAbiLayoutFingerprint::hash_stream_length(&layout).unwrap(),
            8 + "scoop-c-abi-layout-v1".len() as u64 + encoded.len() as u64
        );
        assert_eq!(
            CanonicalCAbiLayoutFingerprint::from_layout(&layout)
                .unwrap()
                .to_string(),
            "d344c6a5bccf5e675d5a2d5516a7f977d7dc928ed3317f539b09c4a76fe1f4c1"
        );
    }

    #[test]
    fn code_pointer_storage_keeps_layout_and_signature_hashes_acyclic() {
        let struct_type = PersistentExactTypeId([1; 32]);
        let callback_type = PersistentExactTypeId([2; 32]);
        let layout = CanonicalCAbiLayout::new(
            struct_type,
            8,
            NonZeroU64::new(8).unwrap(),
            CLayoutOverride::Natural,
            CLayoutOverride::Natural,
            vec![CanonicalCAbiLayoutField::new(
                PersistentFieldId([3; 32]),
                0,
                CanonicalCStorageType::CodePointer {
                    exact_type: callback_type,
                    storage: CPointerStorage::Direct,
                },
            )],
        );
        let layout_fingerprint = CanonicalCAbiLayoutFingerprint::from_layout(&layout).unwrap();
        let parameter = CanonicalCAbiParameter::new(
            struct_type,
            CanonicalCStorageType::Struct {
                exact_type: struct_type,
                layout: layout_fingerprint,
            },
        )
        .unwrap();
        let signature =
            CanonicalCAbiFunctionSignature::cdecl(vec![parameter], CanonicalCAbiReturn::Void);

        CanonicalCAbiSignatureFingerprint::from_signature(&signature).unwrap();
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
