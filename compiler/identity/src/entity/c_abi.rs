use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use crate::ids::derive_persistent_id;
use crate::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, PersistentExactTypeId,
    PersistentFieldId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignednessV1 {
    Signed,
    Unsigned,
}

impl WireEncodeV1 for SignednessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Signed => 1,
            Self::Unsigned => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntegerBitWidthV1 {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

impl IntegerBitWidthV1 {
    pub const fn get(self) -> u8 {
        match self {
            Self::Bits8 => 8,
            Self::Bits16 => 16,
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

impl WireEncodeV1 for IntegerBitWidthV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.get()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CDataPointeeV1 {
    OpaqueUnit,
    ExactObject(PersistentExactTypeId),
}

impl WireEncodeV1 for CDataPointeeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::OpaqueUnit => encode_empty_sum(encoder, 1),
            Self::ExactObject(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CPointerStorageV1 {
    Direct,
    NullableWrapper(PersistentExactTypeId),
}

impl WireEncodeV1 for CPointerStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Direct => encode_empty_sum(encoder, 1),
            Self::NullableWrapper(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalCStorageTypeV1 {
    Integer {
        exact_type: PersistentExactTypeId,
        signedness: SignednessV1,
        bit_width: IntegerBitWidthV1,
    },
    Boolean {
        exact_type: PersistentExactTypeId,
    },
    DataPointer {
        exact_type: PersistentExactTypeId,
        pointee: CDataPointeeV1,
        storage: CPointerStorageV1,
    },
    CodePointer {
        exact_type: PersistentExactTypeId,
        signature: CanonicalCAbiSignatureFingerprint,
        storage: CPointerStorageV1,
    },
    Struct {
        exact_type: PersistentExactTypeId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
}

impl CanonicalCStorageTypeV1 {
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

impl WireEncodeV1 for CanonicalCStorageTypeV1 {
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
                signature,
                storage,
            } => encode_three_value_sum(encoder, 4, exact_type, signature, storage),
            Self::Struct { exact_type, layout } => {
                encode_two_value_sum(encoder, 5, exact_type, layout)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiParameterV1 {
    source_exact_type: PersistentExactTypeId,
    storage: CanonicalCStorageTypeV1,
}

impl CanonicalCAbiParameterV1 {
    pub fn new(
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageTypeV1,
    ) -> Result<Self, CanonicalCAbiError> {
        require_matching_exact_type(source_exact_type, storage)?;
        Ok(Self {
            source_exact_type,
            storage,
        })
    }

    pub const fn source_exact_type(&self) -> PersistentExactTypeId {
        self.source_exact_type
    }
}

impl WireEncodeV1 for CanonicalCAbiParameterV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source_exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.storage.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalCAbiReturnV1 {
    Void,
    Value {
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageTypeV1,
    },
}

impl CanonicalCAbiReturnV1 {
    pub fn value(
        source_exact_type: PersistentExactTypeId,
        storage: CanonicalCStorageTypeV1,
    ) -> Result<Self, CanonicalCAbiError> {
        require_matching_exact_type(source_exact_type, storage)?;
        Ok(Self::Value {
            source_exact_type,
            storage,
        })
    }
}

impl WireEncodeV1 for CanonicalCAbiReturnV1 {
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
pub enum TargetCallingConventionV1 {
    Cdecl,
}

impl WireEncodeV1 for TargetCallingConventionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiFunctionSignatureV1 {
    calling_convention: TargetCallingConventionV1,
    parameters: Vec<CanonicalCAbiParameterV1>,
    result: CanonicalCAbiReturnV1,
}

impl CanonicalCAbiFunctionSignatureV1 {
    pub fn cdecl(parameters: Vec<CanonicalCAbiParameterV1>, result: CanonicalCAbiReturnV1) -> Self {
        Self {
            calling_convention: TargetCallingConventionV1::Cdecl,
            parameters,
            result,
        }
    }

    pub const fn calling_convention(&self) -> TargetCallingConventionV1 {
        self.calling_convention
    }
}

impl WireEncodeV1 for CanonicalCAbiFunctionSignatureV1 {
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
    pub fn from_signature(signature: &CanonicalCAbiFunctionSignatureV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-c-abi-signature-v1", signature)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCAbiSignatureFingerprintRecordV1 {
    fingerprint: CanonicalCAbiSignatureFingerprint,
    signature: CanonicalCAbiFunctionSignatureV1,
}

impl CanonicalCAbiSignatureFingerprintRecordV1 {
    pub fn new(signature: CanonicalCAbiFunctionSignatureV1) -> Result<Self, HashError> {
        let fingerprint = CanonicalCAbiSignatureFingerprint::from_signature(&signature)?;
        Ok(Self {
            fingerprint,
            signature,
        })
    }
}

impl WireEncodeV1 for CanonicalCAbiSignatureFingerprintRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CLayoutByteAlignmentV1 {
    Bytes1,
    Bytes2,
    Bytes4,
    Bytes8,
    Bytes16,
}

impl CLayoutByteAlignmentV1 {
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

impl WireEncodeV1 for CLayoutByteAlignmentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.get()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CLayoutOverrideV1 {
    Natural,
    Bytes(CLayoutByteAlignmentV1),
}

impl WireEncodeV1 for CLayoutOverrideV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Natural => encode_empty_sum(encoder, 1),
            Self::Bytes(bytes) => encode_value_sum(encoder, 2, bytes),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalCAbiLayoutFieldV1 {
    field: PersistentFieldId,
    offset: u64,
    storage: CanonicalCStorageTypeV1,
}

impl CanonicalCAbiLayoutFieldV1 {
    pub const fn new(
        field: PersistentFieldId,
        offset: u64,
        storage: CanonicalCStorageTypeV1,
    ) -> Self {
        Self {
            field,
            offset,
            storage,
        }
    }
}

impl WireEncodeV1 for CanonicalCAbiLayoutFieldV1 {
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
pub struct CanonicalCAbiLayoutV1 {
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    alignment: NonZeroU64,
    aligned: CLayoutOverrideV1,
    packed: CLayoutOverrideV1,
    fields: Vec<CanonicalCAbiLayoutFieldV1>,
}

impl CanonicalCAbiLayoutV1 {
    pub fn new(
        exact_type: PersistentExactTypeId,
        byte_size: u64,
        alignment: NonZeroU64,
        aligned: CLayoutOverrideV1,
        packed: CLayoutOverrideV1,
        fields: Vec<CanonicalCAbiLayoutFieldV1>,
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
}

impl WireEncodeV1 for CanonicalCAbiLayoutV1 {
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
    pub fn from_layout(layout: &CanonicalCAbiLayoutV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-c-abi-layout-v1", layout)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCAbiLayoutFingerprintRecordV1 {
    fingerprint: CanonicalCAbiLayoutFingerprint,
    layout: CanonicalCAbiLayoutV1,
}

impl CanonicalCAbiLayoutFingerprintRecordV1 {
    pub fn new(layout: CanonicalCAbiLayoutV1) -> Result<Self, HashError> {
        let fingerprint = CanonicalCAbiLayoutFingerprint::from_layout(&layout)?;
        Ok(Self {
            fingerprint,
            layout,
        })
    }
}

impl WireEncodeV1 for CanonicalCAbiLayoutFingerprintRecordV1 {
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
    storage: CanonicalCStorageTypeV1,
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
    value: &impl WireEncodeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncodeV1,
    second: &impl WireEncodeV1,
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
    first: &impl WireEncodeV1,
    second: &impl WireEncodeV1,
    third: &impl WireEncodeV1,
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
        CLayoutOverrideV1, CanonicalCAbiError, CanonicalCAbiFunctionSignatureV1,
        CanonicalCAbiLayoutV1, CanonicalCAbiParameterV1, CanonicalCAbiReturnV1,
        CanonicalCStorageTypeV1,
    };
    use crate::{
        CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint, ConeIdentity,
        PersistentExactTypeId,
    };

    #[test]
    fn parameter_rejects_mismatched_storage_exact_type() {
        let source = PersistentExactTypeId(ConeIdentity::CORE.0);
        let other = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        assert_eq!(
            CanonicalCAbiParameterV1::new(
                source,
                CanonicalCStorageTypeV1::Boolean { exact_type: other },
            ),
            Err(CanonicalCAbiError::StorageExactTypeMismatch)
        );
    }

    #[test]
    fn canonical_c_signature_has_fixed_fingerprint() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let parameter = CanonicalCAbiParameterV1::new(
            exact,
            CanonicalCStorageTypeV1::Boolean { exact_type: exact },
        )
        .unwrap();
        let signature =
            CanonicalCAbiFunctionSignatureV1::cdecl(vec![parameter], CanonicalCAbiReturnV1::Void);
        assert_eq!(
            hex(&encode(&signature).unwrap()),
            "a301010281a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02a200020158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d03a10001"
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
        let layout = CanonicalCAbiLayoutV1::new(
            exact,
            0,
            NonZeroU64::new(1).unwrap(),
            CLayoutOverrideV1::Natural,
            CLayoutOverrideV1::Natural,
            Vec::new(),
        );
        assert_eq!(
            hex(&encode(&layout).unwrap()),
            format!("a6015820{exact}0200030104a1000105a100010680")
        );
        assert_eq!(
            CanonicalCAbiLayoutFingerprint::from_layout(&layout)
                .unwrap()
                .to_string(),
            "d344c6a5bccf5e675d5a2d5516a7f977d7dc928ed3317f539b09c4a76fe1f4c1"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
