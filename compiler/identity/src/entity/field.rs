use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{
    GeneratedNominalIdentityError, GeneratedNominalKeyV1, NominalDeclarationOwnerV1,
    SourceDeclarationIdentityError, SourceDeclarationKeyV1, SourceDeclarationKindV1,
};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, PersistentFieldId, PersistentLocalValueId, PersistentPropertyId,
    PersistentTypeId,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceFieldKeyV1(SourceFieldKeyKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum SourceFieldKeyKindV1 {
    Declared {
        owner: NominalDeclarationOwnerV1,
        name: CanonicalIdentifier,
    },
    PropertyBacking {
        owner: NominalDeclarationOwnerV1,
        property: PersistentPropertyId,
    },
    PropertyDelegate {
        owner: NominalDeclarationOwnerV1,
        property: PersistentPropertyId,
    },
}

impl WireEncodeV1 for SourceFieldKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            SourceFieldKeyKindV1::Declared { owner, name } => {
                encode_two_value_sum(encoder, 1, owner, name)
            }
            SourceFieldKeyKindV1::PropertyBacking { owner, property } => {
                encode_two_value_sum(encoder, 2, owner, property)
            }
            SourceFieldKeyKindV1::PropertyDelegate { owner, property } => {
                encode_two_value_sum(encoder, 3, owner, property)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedFieldKeyV1(GeneratedFieldKeyKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum GeneratedFieldKeyKindV1 {
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

impl WireEncodeV1 for GeneratedFieldKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.0 {
            GeneratedFieldKeyKindV1::BoxPayload => encode_empty_sum(encoder, 1),
            GeneratedFieldKeyKindV1::ClosureCapture(value) => encode_value_sum(encoder, 2, &value),
            GeneratedFieldKeyKindV1::CallableReferenceReceiver(value) => {
                encode_value_sum(encoder, 3, &value)
            }
            GeneratedFieldKeyKindV1::CoroutineFrameState => encode_empty_sum(encoder, 4),
            GeneratedFieldKeyKindV1::CoroutineFrameCompletion => encode_empty_sum(encoder, 5),
            GeneratedFieldKeyKindV1::CoroutineFrameSaved(value) => {
                encode_value_sum(encoder, 6, &value)
            }
            GeneratedFieldKeyKindV1::CoroutineFrameFailure => encode_empty_sum(encoder, 7),
            GeneratedFieldKeyKindV1::CoroutineAdapterFrame => encode_empty_sum(encoder, 8),
            GeneratedFieldKeyKindV1::CoroutineAdapterState => encode_empty_sum(encoder, 9),
            GeneratedFieldKeyKindV1::CoroutineAdapterResult => encode_empty_sum(encoder, 10),
            GeneratedFieldKeyKindV1::CoroutineAdapterFailure => encode_empty_sum(encoder, 11),
            GeneratedFieldKeyKindV1::FunctionAdapterSource => encode_empty_sum(encoder, 12),
            GeneratedFieldKeyKindV1::ObjectBackingProperty(property) => {
                encode_value_sum(encoder, 13, &property)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldIdentityKeyV1(FieldIdentityKeyKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FieldIdentityKeyKindV1 {
    Source(SourceFieldKeyV1),
    Generated {
        owner: PersistentTypeId,
        key: GeneratedFieldKeyV1,
    },
}

impl FieldIdentityKeyV1 {
    pub fn source_declared(
        owner: &SourceDeclarationKeyV1,
        name: CanonicalIdentifier,
    ) -> Result<Self, FieldIdentityError> {
        require_source_kind(owner, SourceDeclarationKindV1::Struct)?;
        Ok(Self(FieldIdentityKeyKindV1::Source(SourceFieldKeyV1(
            SourceFieldKeyKindV1::Declared {
                owner: source_nominal_owner(owner)?,
                name,
            },
        ))))
    }

    pub fn source_property_backing(
        owner: &SourceDeclarationKeyV1,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        source_property_field(owner, property, false)
    }

    pub fn source_property_delegate(
        owner: &SourceDeclarationKeyV1,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        source_property_field(owner, property, true)
    }

    pub fn box_payload(owner: &GeneratedNominalKeyV1) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKindV1::BoxedValue,
            GeneratedFieldKeyKindV1::BoxPayload,
        )
    }

    pub fn closure_capture(
        owner: &GeneratedNominalKeyV1,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKindV1::ClosureEnvironment,
            GeneratedFieldKeyKindV1::ClosureCapture(value),
        )
    }

    pub fn callable_reference_receiver(
        owner: &GeneratedNominalKeyV1,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKindV1::ClosureEnvironment,
            GeneratedFieldKeyKindV1::CallableReferenceReceiver(value),
        )
    }

    pub fn coroutine_frame_state(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKindV1::CoroutineFrameState)
    }

    pub fn coroutine_frame_completion(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKindV1::CoroutineFrameCompletion)
    }

    pub fn coroutine_frame_saved(
        owner: &GeneratedNominalKeyV1,
        value: PersistentLocalValueId,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKindV1::CoroutineFrameSaved(value))
    }

    pub fn coroutine_frame_failure(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_frame_field(owner, GeneratedFieldKeyKindV1::CoroutineFrameFailure)
    }

    pub fn coroutine_adapter_frame(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKindV1::CoroutineAdapterFrame)
    }

    pub fn coroutine_adapter_state(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKindV1::CoroutineAdapterState)
    }

    pub fn coroutine_adapter_result(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKindV1::CoroutineAdapterResult)
    }

    pub fn coroutine_adapter_failure(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        coroutine_adapter_field(owner, GeneratedFieldKeyKindV1::CoroutineAdapterFailure)
    }

    pub fn function_adapter_source(
        owner: &GeneratedNominalKeyV1,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKindV1::CallableAdapterEnvironment,
            GeneratedFieldKeyKindV1::FunctionAdapterSource,
        )
    }

    pub fn object_backing_property(
        owner: &GeneratedNominalKeyV1,
        property: PersistentPropertyId,
    ) -> Result<Self, FieldIdentityError> {
        generated_field(
            owner,
            GeneratedOwnerKindV1::ObjectBackingClass,
            GeneratedFieldKeyKindV1::ObjectBackingProperty(property),
        )
    }
}

impl WireEncodeV1 for FieldIdentityKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            FieldIdentityKeyKindV1::Source(key) => encode_value_sum(encoder, 1, key),
            FieldIdentityKeyKindV1::Generated { owner, key } => {
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
    pub fn from_key(key: &FieldIdentityKeyV1) -> Result<Self, FieldIdentityError> {
        derive_persistent_id("scoop-field-id-v1", key).map_err(FieldIdentityError::Hash)
    }
}

#[derive(Clone, Copy)]
enum GeneratedOwnerKindV1 {
    BoxedValue,
    ClosureEnvironment,
    CoroutineFrame,
    ContinuationAdapterEnvironment,
    CallableAdapterEnvironment,
    ObjectBackingClass,
}

fn require_source_kind(
    owner: &SourceDeclarationKeyV1,
    expected: SourceDeclarationKindV1,
) -> Result<(), FieldIdentityError> {
    if owner.declaration_kind() == expected {
        Ok(())
    } else if expected == SourceDeclarationKindV1::Struct {
        Err(FieldIdentityError::ExpectedSourceStruct)
    } else {
        Err(FieldIdentityError::ExpectedSourceClass)
    }
}

fn source_nominal_owner(
    key: &SourceDeclarationKeyV1,
) -> Result<NominalDeclarationOwnerV1, FieldIdentityError> {
    NominalDeclarationOwnerV1::from_source_declaration(key)
        .map_err(FieldIdentityError::SourceDeclaration)
}

fn source_property_field(
    owner: &SourceDeclarationKeyV1,
    property: PersistentPropertyId,
    delegated: bool,
) -> Result<FieldIdentityKeyV1, FieldIdentityError> {
    require_source_kind(owner, SourceDeclarationKindV1::Class)?;
    let owner = source_nominal_owner(owner)?;
    let key = if delegated {
        SourceFieldKeyKindV1::PropertyDelegate { owner, property }
    } else {
        SourceFieldKeyKindV1::PropertyBacking { owner, property }
    };
    Ok(FieldIdentityKeyV1(FieldIdentityKeyKindV1::Source(
        SourceFieldKeyV1(key),
    )))
}

fn coroutine_frame_field(
    owner: &GeneratedNominalKeyV1,
    key: GeneratedFieldKeyKindV1,
) -> Result<FieldIdentityKeyV1, FieldIdentityError> {
    generated_field(owner, GeneratedOwnerKindV1::CoroutineFrame, key)
}

fn coroutine_adapter_field(
    owner: &GeneratedNominalKeyV1,
    key: GeneratedFieldKeyKindV1,
) -> Result<FieldIdentityKeyV1, FieldIdentityError> {
    generated_field(
        owner,
        GeneratedOwnerKindV1::ContinuationAdapterEnvironment,
        key,
    )
}

fn generated_field(
    owner: &GeneratedNominalKeyV1,
    expected: GeneratedOwnerKindV1,
    key: GeneratedFieldKeyKindV1,
) -> Result<FieldIdentityKeyV1, FieldIdentityError> {
    if !generated_owner_matches(owner, expected) {
        return Err(FieldIdentityError::GeneratedOwnerMismatch);
    }
    let owner = PersistentTypeId::from_generated_key(owner)
        .map_err(FieldIdentityError::GeneratedNominal)?;
    Ok(FieldIdentityKeyV1(FieldIdentityKeyKindV1::Generated {
        owner,
        key: GeneratedFieldKeyV1(key),
    }))
}

fn generated_owner_matches(owner: &GeneratedNominalKeyV1, expected: GeneratedOwnerKindV1) -> bool {
    matches!(
        (owner, expected),
        (
            GeneratedNominalKeyV1::BoxedValue { .. },
            GeneratedOwnerKindV1::BoxedValue
        ) | (
            GeneratedNominalKeyV1::ClosureEnvironment { .. },
            GeneratedOwnerKindV1::ClosureEnvironment
        ) | (
            GeneratedNominalKeyV1::CoroutineFrame { .. },
            GeneratedOwnerKindV1::CoroutineFrame
        ) | (
            GeneratedNominalKeyV1::ContinuationAdapterEnvironment { .. },
            GeneratedOwnerKindV1::ContinuationAdapterEnvironment
        ) | (
            GeneratedNominalKeyV1::CallableAdapterEnvironment { .. },
            GeneratedOwnerKindV1::CallableAdapterEnvironment
        ) | (
            GeneratedNominalKeyV1::ObjectBackingClass { .. },
            GeneratedOwnerKindV1::ObjectBackingClass
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

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{FieldIdentityError, FieldIdentityKeyV1};
    use crate::{
        CanonicalIdentifier, ConeIdentity, GeneratedNominalKeyV1, PersistentExactTypeId,
        PersistentFieldId,
    };

    #[test]
    fn box_payload_field_has_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKeyV1::BoxedValue { payload: exact };
        let key = FieldIdentityKeyV1::box_payload(&owner).unwrap();
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
        let owner = GeneratedNominalKeyV1::CoroutineStep { result: exact };
        assert_eq!(
            FieldIdentityKeyV1::box_payload(&owner),
            Err(FieldIdentityError::GeneratedOwnerMismatch)
        );
    }

    #[test]
    fn source_field_constructor_enforces_owner_matrix() {
        let struct_owner = source_nominal(crate::SourceNominalKindV1::Struct);
        let class_owner = source_nominal(crate::SourceNominalKindV1::Class);
        let name = CanonicalIdentifier::new("payload").unwrap();
        assert!(FieldIdentityKeyV1::source_declared(&struct_owner, name.clone()).is_ok());
        assert_eq!(
            FieldIdentityKeyV1::source_declared(&class_owner, name),
            Err(FieldIdentityError::ExpectedSourceStruct)
        );
        let property = crate::PersistentPropertyId(ConeIdentity::CORE.0);
        assert!(FieldIdentityKeyV1::source_property_backing(&class_owner, property).is_ok());
        assert_eq!(
            FieldIdentityKeyV1::source_property_backing(&struct_owner, property),
            Err(FieldIdentityError::ExpectedSourceClass)
        );
    }

    fn source_nominal(kind: crate::SourceNominalKindV1) -> crate::SourceDeclarationKeyV1 {
        crate::SourceDeclarationKeyV1::nominal(
            crate::SourceDeclarationSiteV1::new(
                ConeIdentity::CORE,
                crate::PackagePath::from_segments(vec![CanonicalIdentifier::new("test").unwrap()]),
                crate::DefinitionOwnerChainV1::top_level(),
                crate::DeclarationScopeV1::ConeWide,
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
