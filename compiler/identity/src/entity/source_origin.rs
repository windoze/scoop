use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{CallableOwnerV1, NominalDeclarationOwnerV1, PropertyOwnerV1};
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpanV1 {
    start_byte: u64,
    end_byte: u64,
}

impl SourceSpanV1 {
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

impl WireEncodeV1 for SourceSpanV1 {
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
pub enum SourceContextKeyV1 {
    File {
        source: SourceIdentity,
    },
    Nominal {
        source: SourceIdentity,
        owner: NominalDeclarationOwnerV1,
    },
    Callable {
        source: SourceIdentity,
        owner: CallableOwnerV1,
    },
    Property {
        source: SourceIdentity,
        owner: PropertyOwnerV1,
    },
    Initialization {
        source: SourceIdentity,
        unit: PersistentInitializationUnitId,
    },
}

impl SourceContextKeyV1 {
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

impl WireEncodeV1 for SourceContextKeyV1 {
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
    pub fn from_key(key: &SourceContextKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-source-context-id-v1", key)
    }
}

fn encode_context(
    encoder: &mut Encoder,
    tag: u64,
    source: &SourceIdentity,
    owner: Option<&dyn WireEncodeV1>,
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
pub struct DefinitionOriginV1 {
    source: SourceIdentity,
    span: SourceSpanV1,
    context: PersistentSourceContextId,
}

impl DefinitionOriginV1 {
    pub fn new(
        source: SourceIdentity,
        span: SourceSpanV1,
        context_key: &SourceContextKeyV1,
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

    pub const fn span(&self) -> SourceSpanV1 {
        self.span
    }

    pub const fn context(&self) -> PersistentSourceContextId {
        self.context
    }
}

impl WireEncodeV1 for DefinitionOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EvaluationOriginV1 {
    source: SourceIdentity,
    span: SourceSpanV1,
    context: PersistentSourceContextId,
}

impl EvaluationOriginV1 {
    pub fn new(
        source: SourceIdentity,
        span: SourceSpanV1,
        context_key: &SourceContextKeyV1,
    ) -> Result<Self, SourceOriginError> {
        origin_from_key(source, span, context_key).map(|fields| Self {
            source: fields.source,
            span: fields.span,
            context: fields.context,
        })
    }

    pub fn at_definition(definition: &DefinitionOriginV1) -> Self {
        Self {
            source: definition.source.clone(),
            span: definition.span,
            context: definition.context,
        }
    }

    pub fn source(&self) -> &SourceIdentity {
        &self.source
    }

    pub const fn span(&self) -> SourceSpanV1 {
        self.span
    }

    pub const fn context(&self) -> PersistentSourceContextId {
        self.context
    }
}

impl WireEncodeV1 for EvaluationOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

struct OriginFields {
    source: SourceIdentity,
    span: SourceSpanV1,
    context: PersistentSourceContextId,
}

fn origin_from_key(
    source: SourceIdentity,
    span: SourceSpanV1,
    context_key: &SourceContextKeyV1,
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
    span: SourceSpanV1,
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
pub struct ConcreteExpressionOriginV1 {
    definition: DefinitionOriginV1,
    evaluation: EvaluationOriginV1,
}

impl ConcreteExpressionOriginV1 {
    pub fn new(definition: DefinitionOriginV1, evaluation: EvaluationOriginV1) -> Self {
        Self {
            definition,
            evaluation,
        }
    }

    pub fn definition(&self) -> &DefinitionOriginV1 {
        &self.definition
    }

    pub fn evaluation(&self) -> &EvaluationOriginV1 {
        &self.evaluation
    }
}

impl WireEncodeV1 for ConcreteExpressionOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.evaluation.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionOriginV1 {
    Definition(DefinitionOriginV1),
    Concrete(ConcreteExpressionOriginV1),
}

impl WireEncodeV1 for ExpressionOriginV1 {
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
pub enum DefinitionOriginSubjectV1 {
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

impl WireEncodeV1 for DefinitionOriginSubjectV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncodeV1) = match self {
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
pub struct DefinitionOriginRecordV1 {
    subject: DefinitionOriginSubjectV1,
    origin: DefinitionOriginV1,
}

impl DefinitionOriginRecordV1 {
    pub fn new(subject: DefinitionOriginSubjectV1, origin: DefinitionOriginV1) -> Self {
        Self { subject, origin }
    }

    pub const fn subject(&self) -> DefinitionOriginSubjectV1 {
        self.subject
    }

    pub fn origin(&self) -> &DefinitionOriginV1 {
        &self.origin
    }
}

impl WireEncodeV1 for DefinitionOriginRecordV1 {
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

    use super::{DefinitionOriginV1, EvaluationOriginV1, SourceContextKeyV1, SourceSpanV1};
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
        let key = SourceContextKeyV1::File {
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
        let wrong_key = SourceContextKeyV1::File {
            source: SourceIdentity::single_file(),
        };
        assert!(
            DefinitionOriginV1::new(source, SourceSpanV1::new(1, 2).unwrap(), &wrong_key).is_err()
        );
    }

    #[test]
    fn definition_and_evaluation_origins_remain_distinct_types() {
        let source = core_source();
        let key = SourceContextKeyV1::File {
            source: source.clone(),
        };
        let definition =
            DefinitionOriginV1::new(source, SourceSpanV1::new(3, 7).unwrap(), &key).unwrap();
        let evaluation = EvaluationOriginV1::at_definition(&definition);
        assert_eq!(encode(&definition).unwrap(), encode(&evaluation).unwrap());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
