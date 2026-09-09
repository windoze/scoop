use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    GeneratedNominalIdentityError, GeneratedNominalKey, NominalDeclarationOwner,
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKind,
};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, PersistentFieldId, PersistentLocalValueId, PersistentPropertyId,
    PersistentTypeId,
};

mod decode;

pub use decode::{
    DecodedFieldIdentityKey, DecodedGeneratedFieldKey, DecodedSourceFieldKey,
    FieldIdentityResolutionError,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceFieldKey(SourceFieldKeyKind);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum SourceFieldKeyKind {
    Declared {
        owner: NominalDeclarationOwner,
        name: CanonicalIdentifier,
    },
    PropertyBacking {
        owner: NominalDeclarationOwner,
        property: PersistentPropertyId,
    },
    PropertyDelegate {
        owner: NominalDeclarationOwner,
        property: PersistentPropertyId,
    },
}

impl WireEncode for SourceFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            SourceFieldKeyKind::Declared { owner, name } => {
                encode_two_value_sum(encoder, 1, owner, name)
            }
            SourceFieldKeyKind::PropertyBacking { owner, property } => {
                encode_two_value_sum(encoder, 2, owner, property)
            }
            SourceFieldKeyKind::PropertyDelegate { owner, property } => {
                encode_two_value_sum(encoder, 3, owner, property)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedFieldKey(GeneratedFieldKeyKind);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum GeneratedFieldKeyKind {
    BoxPayload,
    ClosureCapture(PersistentLocalValueId),
    CallableReferenceReceiver(PersistentLocalValueId),
    CoroutineFrameState,
    CoroutineFrameCompletion,
    CoroutineFrameSaved(PersistentLocalValueId),
    CoroutineFrameFailure,
    CoroutineAdapterFrame,
    CoroutineAdapterState,
    CoroutineAdapterResult,
    CoroutineAdapterFailure,
    FunctionAdapterSource,
    ObjectBackingProperty(PersistentPropertyId),
}

impl WireEncode for GeneratedFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            GeneratedFieldKeyKind::BoxPayload => encode_empty_sum(encoder, 1),
            GeneratedFieldKeyKind::ClosureCapture(value) => encode_value_sum(encoder, 2, &value),
            GeneratedFieldKeyKind::CallableReferenceReceiver(value) => {
                encode_value_sum(encoder, 3, &value)
            }
            GeneratedFieldKeyKind::CoroutineFrameState => encode_empty_sum(encoder, 4),
            GeneratedFieldKeyKind::CoroutineFrameCompletion => encode_empty_sum(encoder, 5),
            GeneratedFieldKeyKind::CoroutineFrameSaved(value) => {
                encode_value_sum(encoder, 6, &value)
            }
            GeneratedFieldKeyKind::CoroutineFrameFailure => encode_empty_sum(encoder, 7),
            GeneratedFieldKeyKind::CoroutineAdapterFrame => encode_empty_sum(encoder, 8),
            GeneratedFieldKeyKind::CoroutineAdapterState => encode_empty_sum(encoder, 9),
            GeneratedFieldKeyKind::CoroutineAdapterResult => encode_empty_sum(encoder, 10),
            GeneratedFieldKeyKind::CoroutineAdapterFailure => encode_empty_sum(encoder, 11),
            GeneratedFieldKeyKind::FunctionAdapterSource => encode_empty_sum(encoder, 12),
            GeneratedFieldKeyKind::ObjectBackingProperty(property) => {
                encode_value_sum(encoder, 13, &property)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldIdentityKey(FieldIdentityKeyKind);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FieldIdentityKeyKind {
    Source(SourceFieldKey),
    Generated {
        owner: PersistentTypeId,
        key: GeneratedFieldKey,
    },
}

impl FieldIdentityKey {
    pub fn source_declared(
        owner: &SourceDeclarationKey,
        name: CanonicalIdentifier,
    ) -> Result<Self, FieldIdentityError> {
        require_source_kind(owner, SourceDeclarationKind::Struct)?;
        Ok(Self(FieldIdentityKeyKind::Source(SourceFieldKey(
            SourceFieldKeyKind::Declared {
                owner: source_nominal_owner(owner)?,
                name,
            },
        ))))
    }

    pub fn source_property_backing(
        owner: &SourceDeclarationKey,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        source_property_field(owner, property, false)
    }

    pub fn source_property_delegate(
        owner: &SourceDeclarationKey,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        source_property_field(owner, property, true)
    }

    pub fn box_payload(owner: &GeneratedNominalKey) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::BoxedValue,
            GeneratedFieldKeyKind::BoxPayload,
        )
    }

    pub fn closure_capture(
        owner: &GeneratedNominalKey,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::ClosureEnvironment,
            GeneratedFieldKeyKind::ClosureCapture(value),
        )
    }

    pub fn callable_reference_receiver(
        owner: &GeneratedNominalKey,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::ClosureEnvironment,
            GeneratedFieldKeyKind::CallableReferenceReceiver(value),
        )
    }

    pub fn coroutine_frame_state(owner: &GeneratedNominalKey) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKind::CoroutineFrameState)
    }

    pub fn coroutine_frame_completion(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKind::CoroutineFrameCompletion)
    }

    pub fn coroutine_frame_saved(
        owner: &GeneratedNominalKey,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKind::CoroutineFrameSaved(value))
    }

    pub fn coroutine_frame_failure(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKind::CoroutineFrameFailure)
    }

    pub fn coroutine_adapter_frame(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKind::CoroutineAdapterFrame)
    }

    pub fn coroutine_adapter_state(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKind::CoroutineAdapterState)
    }

    pub fn coroutine_adapter_result(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKind::CoroutineAdapterResult)
    }

    pub fn coroutine_adapter_failure(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKind::CoroutineAdapterFailure)
    }

    pub fn function_adapter_source(
        owner: &GeneratedNominalKey,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::CallableAdapterEnvironment,
            GeneratedFieldKeyKind::FunctionAdapterSource,
        )
    }

    pub fn object_backing_property(
        owner: &GeneratedNominalKey,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKind::ObjectBackingClass,
            GeneratedFieldKeyKind::ObjectBackingProperty(property),
        )
    }
}

impl WireEncode for FieldIdentityKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            FieldIdentityKeyKind::Source(key) => encode_value_sum(encoder, 1, key),
            FieldIdentityKeyKind::Generated { owner, key } => {
                encode_two_value_sum(encoder, 2, owner, key)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldIdentityError {
    ExpectedSourceStruct,
    ExpectedSourceClass,
    GeneratedOwnerMismatch,
    SourceDeclaration(SourceDeclarationIdentityError),
    GeneratedNominal(GeneratedNominalIdentityError),
    Hash(HashError),
}

impl fmt::Display for FieldIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceStruct => {
                formatter.write_str("declared field owner must be a source struct")
            }
            Self::ExpectedSourceClass => {
                formatter.write_str("property field owner must be a source class")
            }
            Self::GeneratedOwnerMismatch => {
                formatter.write_str("generated field role does not match its nominal owner")
            }
            Self::SourceDeclaration(error) => error.fmt(formatter),
            Self::GeneratedNominal(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FieldIdentityError {}

impl PersistentFieldId {
    pub fn from_key(key: &FieldIdentityKey) -> Result<Self, FieldIdentityError> {
        derive_persistent_id("scoop-field-id-v1", key).map_err(FieldIdentityError::Hash)
    }
}

#[derive(Clone, Copy)]
enum GeneratedOwnerKind {
    BoxedValue,
    ClosureEnvironment,
    CoroutineFrame,
    ContinuationAdapterEnvironment,
    CallableAdapterEnvironment,
    ObjectBackingClass,
}

fn require_source_kind(
    owner: &SourceDeclarationKey,
    expected: SourceDeclarationKind,
) -> Result<(), FieldIdentityError> {
    if owner.declaration_kind() == expected {
        Ok(())
    } else if expected == SourceDeclarationKind::Struct {
        Err(FieldIdentityError::ExpectedSourceStruct)
    } else {
        Err(FieldIdentityError::ExpectedSourceClass)
    }
}

fn source_nominal_owner(
    key: &SourceDeclarationKey,
) -> Result<NominalDeclarationOwner, FieldIdentityError> {
    NominalDeclarationOwner::from_source_declaration(key)
        .map_err(FieldIdentityError::SourceDeclaration)
}

fn source_property_field(
    owner: &SourceDeclarationKey,
    property: PersistentPropertyId,
    delegated: bool,
) -> Result<FieldIdentityKey, FieldIdentityError> {
    require_source_kind(owner, SourceDeclarationKind::Class)?;
    let owner = source_nominal_owner(owner)?;
    let key = if delegated {
        SourceFieldKeyKind::PropertyDelegate { owner, property }
    } else {
        SourceFieldKeyKind::PropertyBacking { owner, property }
    };
    Ok(FieldIdentityKey(FieldIdentityKeyKind::Source(
        SourceFieldKey(key),
    )))
}

fn coroutine_frame_field(
    owner: &GeneratedNominalKey,
    key: GeneratedFieldKeyKind,
) -> Result<FieldIdentityKey, FieldIdentityError> {
    generated_field(owner, GeneratedOwnerKind::CoroutineFrame, key)
}

fn coroutine_adapter_field(
    owner: &GeneratedNominalKey,
    key: GeneratedFieldKeyKind,
) -> Result<FieldIdentityKey, FieldIdentityError> {
    generated_field(
        owner,
        GeneratedOwnerKind::ContinuationAdapterEnvironment,
        key,
    )
}

fn generated_field(
    owner: &GeneratedNominalKey,
    expected: GeneratedOwnerKind,
    key: GeneratedFieldKeyKind,
) -> Result<FieldIdentityKey, FieldIdentityError> {
    if !generated_owner_matches(owner, expected) {
        return Err(FieldIdentityError::GeneratedOwnerMismatch);
    }
    let owner = PersistentTypeId::from_generated_key(owner)
        .map_err(FieldIdentityError::GeneratedNominal)?;
    Ok(FieldIdentityKey(FieldIdentityKeyKind::Generated {
        owner,
        key: GeneratedFieldKey(key),
    }))
}

fn generated_owner_matches(owner: &GeneratedNominalKey, expected: GeneratedOwnerKind) -> bool {
    matches!(
        (owner, expected),
        (
            GeneratedNominalKey::BoxedValue { .. },
            GeneratedOwnerKind::BoxedValue
        ) | (
            GeneratedNominalKey::ClosureEnvironment { .. },
            GeneratedOwnerKind::ClosureEnvironment
        ) | (
            GeneratedNominalKey::CoroutineFrame { .. },
            GeneratedOwnerKind::CoroutineFrame
        ) | (
            GeneratedNominalKey::ContinuationAdapterEnvironment { .. },
            GeneratedOwnerKind::ContinuationAdapterEnvironment
        ) | (
            GeneratedNominalKey::CallableAdapterEnvironment { .. },
            GeneratedOwnerKind::CallableAdapterEnvironment
        ) | (
            GeneratedNominalKey::ObjectBackingClass { .. },
            GeneratedOwnerKind::ObjectBackingClass
        )
    )
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

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{FieldIdentityError, FieldIdentityKey};
    use crate::{
        CanonicalIdentifier, ConeIdentity, GeneratedNominalKey, PersistentExactTypeId,
        PersistentFieldId,
    };

    #[test]
    fn box_payload_field_has_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKey::BoxedValue { payload: exact };
        let key = FieldIdentityKey::box_payload(&owner).unwrap();
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a30002015820b50d260f83c9bf2d7b8b678a332f1092aa8424a5c1bfd24f047f5e93e4ee8f0302a10001"
        );
        assert_eq!(
            PersistentFieldId::from_key(&key).unwrap().to_string(),
            "6fb80b60795479b8c312502067a38b707b6ca7ab6ba805a6e4d110679119e5cf"
        );
    }

    #[test]
    fn generated_field_constructor_enforces_owner_matrix() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKey::CoroutineStep { result: exact };
        assert_eq!(
            FieldIdentityKey::box_payload(&owner),
            Err(FieldIdentityError::GeneratedOwnerMismatch)
        );
    }

    #[test]
    fn source_field_constructor_enforces_owner_matrix() {
        let struct_owner = source_nominal(crate::SourceNominalKind::Struct);
        let class_owner = source_nominal(crate::SourceNominalKind::Class);
        let name = CanonicalIdentifier::new("payload").unwrap();
        assert!(FieldIdentityKey::source_declared(&struct_owner, name.clone()).is_ok());
        assert_eq!(
            FieldIdentityKey::source_declared(&class_owner, name),
            Err(FieldIdentityError::ExpectedSourceStruct)
        );
        let property = crate::PersistentPropertyId(ConeIdentity::CORE.0);
        assert!(FieldIdentityKey::source_property_backing(&class_owner, property).is_ok());
        assert_eq!(
            FieldIdentityKey::source_property_backing(&struct_owner, property),
            Err(FieldIdentityError::ExpectedSourceClass)
        );
    }

    fn source_nominal(kind: crate::SourceNominalKind) -> crate::SourceDeclarationKey {
        crate::SourceDeclarationKey::nominal(
            crate::SourceDeclarationSite::new(
                ConeIdentity::CORE,
                crate::PackagePath::from_segments(vec![CanonicalIdentifier::new("test").unwrap()]),
                crate::DefinitionOwnerChain::top_level(),
                crate::DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Owner").unwrap(),
            kind,
            0,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
