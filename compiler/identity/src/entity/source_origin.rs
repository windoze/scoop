use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{CallableOwner, NominalDeclarationOwner, PropertyOwner};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentInitializationUnitId, PersistentLocalBindingId,
    PersistentLocalValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentSourceContextId, PersistentSourceNativeExternalContractId, PersistentTypeAliasId,
    PersistentTypeId, SourceIdentity,
};

mod decode;

pub use decode::{
    DecodedConcreteExpressionOrigin, DecodedDefinitionOrigin, DecodedDefinitionOriginRecord,
    DecodedDefinitionOriginSubject, DecodedEvaluationOrigin, DecodedExpressionOrigin,
    DecodedSourceContextKey, DecodedSourceSpan, DefinitionOriginRecordResolutionError,
    DefinitionOriginSubjectResolver, SourceContextResolutionError, SourceContextResolver,
    SourceOriginResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    start_byte: u64,
    end_byte: u64,
}

impl SourceSpan {
    pub fn new(start_byte: u64, end_byte: u64) -> Result<Self, SourceSpanError> {
        if start_byte <= end_byte {
            Ok(Self {
                start_byte,
                end_byte,
            })
        } else {
            Err(SourceSpanError)
        }
    }

    pub const fn start_byte(&self) -> u64 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u64 {
        self.end_byte
    }
}

impl WireEncode for SourceSpan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(self.start_byte)?;
        encoder.field(2)?;
        encoder.unsigned(self.end_byte)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpanError;

impl fmt::Display for SourceSpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("source span start must not exceed its end")
    }
}

impl std::error::Error for SourceSpanError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceContextKey {
    File {
        source: SourceIdentity,
    },
    Nominal {
        source: SourceIdentity,
        owner: NominalDeclarationOwner,
    },
    Callable {
        source: SourceIdentity,
        owner: CallableOwner,
    },
    Property {
        source: SourceIdentity,
        owner: PropertyOwner,
    },
    Initialization {
        source: SourceIdentity,
        unit: PersistentInitializationUnitId,
    },
}

impl SourceContextKey {
    pub fn source(&self) -> &SourceIdentity {
        match self {
            Self::File { source }
            | Self::Nominal { source, .. }
            | Self::Callable { source, .. }
            | Self::Property { source, .. }
            | Self::Initialization { source, .. } => source,
        }
    }
}

impl WireEncode for SourceContextKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::File { source } => encode_context(encoder, 1, source, None),
            Self::Nominal { source, owner } => encode_context(encoder, 2, source, Some(owner)),
            Self::Callable { source, owner } => encode_context(encoder, 3, source, Some(owner)),
            Self::Property { source, owner } => encode_context(encoder, 4, source, Some(owner)),
            Self::Initialization { source, unit } => encode_context(encoder, 5, source, Some(unit)),
        }
    }
}

impl PersistentSourceContextId {
    pub fn from_key(key: &SourceContextKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-source-context-id-v1", key)
    }
}

fn encode_context(
    encoder: &mut Encoder,
    tag: u64,
    source: &SourceIdentity,
    owner: Option<&dyn WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(if owner.is_some() { 3 } else { 2 })?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    if let Some(owner) = owner {
        encoder.field(2)?;
        owner.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefinitionOrigin {
    source: SourceIdentity,
    span: SourceSpan,
    context: PersistentSourceContextId,
}

impl DefinitionOrigin {
    pub fn new(
        source: SourceIdentity,
        span: SourceSpan,
        context_key: &SourceContextKey,
    ) -> Result<Self, SourceOriginError> {
        origin_from_key(source, span, context_key).map(|fields| Self {
            source: fields.source,
            span: fields.span,
            context: fields.context,
        })
    }

    pub fn source(&self) -> &SourceIdentity {
        &self.source
    }

    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub const fn context(&self) -> PersistentSourceContextId {
        self.context
    }
}

impl WireEncode for DefinitionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvaluationOrigin {
    source: SourceIdentity,
    span: SourceSpan,
    context: PersistentSourceContextId,
}

impl EvaluationOrigin {
    pub fn new(
        source: SourceIdentity,
        span: SourceSpan,
        context_key: &SourceContextKey,
    ) -> Result<Self, SourceOriginError> {
        origin_from_key(source, span, context_key).map(|fields| Self {
            source: fields.source,
            span: fields.span,
            context: fields.context,
        })
    }

    pub fn at_definition(definition: &DefinitionOrigin) -> Self {
        Self {
            source: definition.source.clone(),
            span: definition.span,
            context: definition.context,
        }
    }

    pub fn source(&self) -> &SourceIdentity {
        &self.source
    }

    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub const fn context(&self) -> PersistentSourceContextId {
        self.context
    }
}

impl WireEncode for EvaluationOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

struct OriginFields {
    source: SourceIdentity,
    span: SourceSpan,
    context: PersistentSourceContextId,
}

fn origin_from_key(
    source: SourceIdentity,
    span: SourceSpan,
    context_key: &SourceContextKey,
) -> Result<OriginFields, SourceOriginError> {
    if context_key.source() != &source {
        return Err(SourceOriginError::ContextSourceMismatch);
    }
    let context =
        PersistentSourceContextId::from_key(context_key).map_err(SourceOriginError::Hash)?;
    Ok(OriginFields {
        source,
        span,
        context,
    })
}

fn encode_origin(
    encoder: &mut Encoder,
    source: &SourceIdentity,
    span: SourceSpan,
    context: PersistentSourceContextId,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    span.encode(encoder)?;
    encoder.field(3)?;
    context.encode(encoder)
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConcreteExpressionOrigin {
    definition: DefinitionOrigin,
    evaluation: EvaluationOrigin,
}

impl ConcreteExpressionOrigin {
    pub fn new(definition: DefinitionOrigin, evaluation: EvaluationOrigin) -> Self {
        Self {
            definition,
            evaluation,
        }
    }

    pub fn definition(&self) -> &DefinitionOrigin {
        &self.definition
    }

    pub fn evaluation(&self) -> &EvaluationOrigin {
        &self.evaluation
    }
}

impl WireEncode for ConcreteExpressionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.evaluation.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionOrigin {
    Definition(DefinitionOrigin),
    Concrete(ConcreteExpressionOrigin),
}

impl WireEncode for ExpressionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Definition(_) => 1,
            Self::Concrete(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Definition(origin) => origin.encode(encoder),
            Self::Concrete(origin) => origin.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionOriginSubject {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Property(PersistentPropertyId),
    ExtensionProperty(PersistentExtensionPropertyId),
    PropertyAccessor(PersistentPropertyAccessorId),
    TypeAlias(PersistentTypeAliasId),
    Field(PersistentFieldId),
    EnumVariant(PersistentEnumVariantId),
    EnumVariantField(PersistentEnumVariantFieldId),
    GeneratedCallable(PersistentGeneratedCallableId),
    InitializationUnit(PersistentInitializationUnitId),
    LocalBinding(PersistentLocalBindingId),
    LocalValue(PersistentLocalValueId),
    CallbackRegistration(PersistentCallbackRegistrationId),
    SourceNativeContract(PersistentSourceNativeExternalContractId),
}

impl DefinitionOriginSubject {
    pub const fn kind_tag(self) -> u8 {
        match self {
            Self::Type(_) => 1,
            Self::GenericType(_) => 2,
            Self::Function(_) => 3,
            Self::GenericFunction(_) => 4,
            Self::Constructor(_) => 5,
            Self::Property(_) => 6,
            Self::ExtensionProperty(_) => 7,
            Self::PropertyAccessor(_) => 8,
            Self::TypeAlias(_) => 9,
            Self::Field(_) => 10,
            Self::EnumVariant(_) => 11,
            Self::EnumVariantField(_) => 12,
            Self::GeneratedCallable(_) => 13,
            Self::InitializationUnit(_) => 14,
            Self::LocalBinding(_) => 15,
            Self::LocalValue(_) => 16,
            Self::CallbackRegistration(_) => 17,
            Self::SourceNativeContract(_) => 18,
        }
    }

    pub fn raw_id(self) -> [u8; 32] {
        match self {
            Self::Type(id) => *id.as_array(),
            Self::GenericType(id) => *id.as_array(),
            Self::Function(id) => *id.as_array(),
            Self::GenericFunction(id) => *id.as_array(),
            Self::Constructor(id) => *id.as_array(),
            Self::Property(id) => *id.as_array(),
            Self::ExtensionProperty(id) => *id.as_array(),
            Self::PropertyAccessor(id) => *id.as_array(),
            Self::TypeAlias(id) => *id.as_array(),
            Self::Field(id) => *id.as_array(),
            Self::EnumVariant(id) => *id.as_array(),
            Self::EnumVariantField(id) => *id.as_array(),
            Self::GeneratedCallable(id) => *id.as_array(),
            Self::InitializationUnit(id) => *id.as_array(),
            Self::LocalBinding(id) => *id.as_array(),
            Self::LocalValue(id) => *id.as_array(),
            Self::CallbackRegistration(id) => *id.as_array(),
            Self::SourceNativeContract(id) => *id.as_array(),
        }
    }

    pub fn compare_sort_key(self, other: Self) -> std::cmp::Ordering {
        self.kind_tag()
            .cmp(&other.kind_tag())
            .then_with(|| self.raw_id().cmp(&other.raw_id()))
    }
}

impl WireEncode for DefinitionOriginSubject {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Type(id) => (1, id),
            Self::GenericType(id) => (2, id),
            Self::Function(id) => (3, id),
            Self::GenericFunction(id) => (4, id),
            Self::Constructor(id) => (5, id),
            Self::Property(id) => (6, id),
            Self::ExtensionProperty(id) => (7, id),
            Self::PropertyAccessor(id) => (8, id),
            Self::TypeAlias(id) => (9, id),
            Self::Field(id) => (10, id),
            Self::EnumVariant(id) => (11, id),
            Self::EnumVariantField(id) => (12, id),
            Self::GeneratedCallable(id) => (13, id),
            Self::InitializationUnit(id) => (14, id),
            Self::LocalBinding(id) => (15, id),
            Self::LocalValue(id) => (16, id),
            Self::CallbackRegistration(id) => (17, id),
            Self::SourceNativeContract(id) => (18, id),
        };
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefinitionOriginRecord {
    subject: DefinitionOriginSubject,
    origin: DefinitionOrigin,
}

impl DefinitionOriginRecord {
    pub fn new(subject: DefinitionOriginSubject, origin: DefinitionOrigin) -> Self {
        Self { subject, origin }
    }

    pub const fn subject(&self) -> DefinitionOriginSubject {
        self.subject
    }

    pub fn origin(&self) -> &DefinitionOrigin {
        &self.origin
    }
}

impl WireEncode for DefinitionOriginRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.origin.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOriginError {
    ContextSourceMismatch,
    Hash(HashError),
}

impl fmt::Display for SourceOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextSourceMismatch => {
                formatter.write_str("source origin and source context refer to different sources")
            }
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceOriginError {}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{DefinitionOrigin, EvaluationOrigin, SourceContextKey, SourceSpan};
    use crate::{ConeIdentity, NormalizedSourcePath, PersistentSourceContextId, SourceIdentity};

    fn core_source() -> SourceIdentity {
        SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/String.scoop").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn file_context_has_fixed_wire_and_hash() {
        let key = SourceContextKey::File {
            source: core_source(),
        };
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a2000101a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02707372632f537472696e672e73636f6f70"
        );
        assert_eq!(
            PersistentSourceContextId::from_key(&key)
                .unwrap()
                .to_string(),
            "1ea54fa4c10ec40ff13bf41320fa4d6cb298ebdc129bb7d8f2f846b44c40f2a7"
        );
    }

    #[test]
    fn origin_requires_the_contexts_exact_source() {
        let source = core_source();
        let wrong_key = SourceContextKey::File {
            source: SourceIdentity::single_file(),
        };
        assert!(DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &wrong_key).is_err());
    }

    #[test]
    fn definition_and_evaluation_origins_remain_distinct_types() {
        let source = core_source();
        let key = SourceContextKey::File {
            source: source.clone(),
        };
        let definition =
            DefinitionOrigin::new(source, SourceSpan::new(3, 7).unwrap(), &key).unwrap();
        let evaluation = EvaluationOrigin::at_definition(&definition);
        assert_eq!(encode(&definition).unwrap(), encode(&evaluation).unwrap());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
