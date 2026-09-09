use std::fmt;
use std::marker::PhantomData;

use scoop_wire::{
    Decoder, Digest256, Encoder, HashError, WireDecodeV1, WireEncodeV1, WireError, WireErrorKind,
    domain_separated_cbor_hash,
};

mod private {
    pub trait Sealed {}
}

/// Shared read-only behavior for a concrete persistent identity kind.
///
/// This trait is sealed and exposes no raw-byte constructor. Each public id
/// remains a separate Rust type and can only be constructed from its specified
/// canonical semantic key.
pub trait PersistentIdV1: private::Sealed + Copy + fmt::Debug + Eq + Ord + std::hash::Hash {
    const KIND: &'static str;

    fn as_array(&self) -> &[u8; 32];
}

pub(crate) trait PersistentIdConstruction: PersistentIdV1 {
    fn from_digest(digest: Digest256) -> Self;
}

macro_rules! persistent_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub(crate) [u8; 32]);

        impl $name {
            pub fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl private::Sealed for $name {}

        impl PersistentIdV1 for $name {
            const KIND: &'static str = $kind;

            fn as_array(&self) -> &[u8; 32] {
                self.as_array()
            }
        }

        impl PersistentIdConstruction for $name {
            fn from_digest(digest: Digest256) -> Self {
                Self(*digest.as_array())
            }
        }

        impl WireEncodeV1 for $name {
            fn encode(
                &self,
                encoder: &mut Encoder,
            ) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(&self.0, formatter)
            }
        }
    };
}

persistent_id!(ConeIdentity, "cone");
persistent_id!(PersistentTypeId, "type");
persistent_id!(PersistentSourceContextId, "source context");
persistent_id!(PersistentGenericTypeId, "generic type");
persistent_id!(PersistentFunctionId, "function");
persistent_id!(PersistentGenericFunctionId, "generic function");
persistent_id!(PersistentCallableApplicationId, "callable application");
persistent_id!(PersistentConstructorId, "constructor");
persistent_id!(PersistentPropertyId, "property");
persistent_id!(PersistentExtensionPropertyId, "extension property");
persistent_id!(PersistentObjectValueId, "object value");
persistent_id!(PersistentTypeAliasId, "type alias");
persistent_id!(PersistentPropertyAccessorId, "property accessor");
persistent_id!(PersistentFieldId, "field");
persistent_id!(PersistentEnumVariantId, "enum variant");
persistent_id!(PersistentEnumVariantFieldId, "enum variant field");
persistent_id!(PersistentGeneratedCallableId, "generated callable");
persistent_id!(PersistentDispatchSlotId, "dispatch slot");
persistent_id!(PersistentLocalBindingId, "local binding");
persistent_id!(PersistentLocalValueId, "local value");
persistent_id!(PersistentCallbackRegistrationId, "callback registration");
persistent_id!(PersistentCallbackApplicationId, "callback application");
persistent_id!(
    PersistentSourceNativeExternalContractId,
    "source native external contract"
);
persistent_id!(PersistentNativeExternalSymbolId, "native external symbol");
persistent_id!(NativeLinkRequirementId, "native link requirement");
persistent_id!(PersistentExportBindingId, "export binding");
persistent_id!(PersistentInitializationUnitId, "initialization unit");
persistent_id!(PersistentLayoutId, "layout");
persistent_id!(PersistentScanId, "scan");
persistent_id!(PersistentDispatchTableId, "dispatch table");
persistent_id!(PersistentStaticStorageId, "static storage");
persistent_id!(PersistentImmortalObjectId, "immortal object");
persistent_id!(OdrGroupId, "ODR group");
persistent_id!(OdrMemberId, "ODR member");
persistent_id!(GeneratedBridgeUnitId, "generated bridge unit");
persistent_id!(GeneratedBridgeAtomId, "generated bridge atom");
persistent_id!(ObjectDefinitionPlanId, "object definition plan");
persistent_id!(ObjectDefinitionAtomId, "object definition atom");
persistent_id!(PersistentExactTypeId, "exact type");
persistent_id!(PersistentCallableBodyId, "callable body");
persistent_id!(PersistentSafepointSiteId, "safepoint site");

/// Typed but not yet semantically trusted bytes read from Wire CBOR v1.
///
/// Validation must call [`Self::verify`] with an id recomputed from the
/// canonical key before the concrete identity can escape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPersistentIdV1<I: PersistentIdV1> {
    bytes: [u8; 32],
    marker: PhantomData<I>,
}

impl<I: PersistentIdV1> DecodedPersistentIdV1<I> {
    pub fn as_array(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn verify(self, expected: I) -> Result<I, PersistentIdMismatch<I>> {
        if self.bytes == *expected.as_array() {
            Ok(expected)
        } else {
            Err(PersistentIdMismatch {
                expected,
                actual: self.bytes,
            })
        }
    }
}

impl<I: PersistentIdV1> WireEncodeV1 for DecodedPersistentIdV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.bytes)
    }
}

impl<I: PersistentIdV1> WireDecodeV1 for DecodedPersistentIdV1<I> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let value = decoder.bytes()?;
        let bytes = <&[u8; 32]>::try_from(value).map_err(|_| {
            WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 32,
                    actual: value.len() as u64,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            )
        })?;
        Ok(Self {
            bytes: *bytes,
            marker: PhantomData,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentIdMismatch<I: PersistentIdV1> {
    expected: I,
    actual: [u8; 32],
}

impl<I: PersistentIdV1> PersistentIdMismatch<I> {
    pub fn expected(&self) -> I {
        self.expected
    }

    pub fn actual(&self) -> &[u8; 32] {
        &self.actual
    }
}

impl<I: PersistentIdV1> fmt::Display for PersistentIdMismatch<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} identity mismatch: expected ", I::KIND)?;
        write_hex(self.expected.as_array(), formatter)?;
        formatter.write_str(", found ")?;
        write_hex(&self.actual, formatter)
    }
}

impl<I: PersistentIdV1> std::error::Error for PersistentIdMismatch<I> {}

pub(crate) fn derive_persistent_id<I: PersistentIdConstruction>(
    domain: &'static str,
    key: &impl WireEncodeV1,
) -> Result<I, HashError> {
    domain_separated_cbor_hash(domain, key).map(I::from_digest)
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimitsV1, decode_canonical, encode};

    use super::{ConeIdentity, DecodedPersistentIdV1};

    #[test]
    fn raw_id_decode_stays_typed_until_verified() {
        let expected = ConeIdentity::SINGLE_FILE;
        let encoded = encode(&expected).unwrap();
        let decoded = decode_canonical::<DecodedPersistentIdV1<ConeIdentity>>(
            &encoded,
            DecodeLimitsV1::default(),
        )
        .unwrap();

        assert_eq!(decoded.as_array(), expected.as_array());
        assert_eq!(decoded.verify(expected), Ok(expected));
    }

    #[test]
    fn id_decoder_rejects_the_wrong_width() {
        let error = decode_canonical::<DecodedPersistentIdV1<ConeIdentity>>(
            b"\x42\0\0",
            DecodeLimitsV1::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &scoop_wire::WireErrorKind::InvalidLength {
                expected: 32,
                actual: 2,
            }
        );
    }

    #[test]
    fn display_is_fixed_lowercase_hex() {
        let display = ConeIdentity::CORE.to_string();
        assert_eq!(display.len(), 64);
        assert_eq!(
            display,
            "5ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d"
        );
    }
}
