use std::fmt;

use scoop_identity::{
    DecodedSignatureTypeKey, SignatureTypeKey, SourceOriginResolutionError,
    StructuralDefinitionPath,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    BinderUseListValidationError, CanonicalBinderUseListV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DecodedCanonicalBinderUseListV1,
    DecodedCanonicalTemplateLocalTableV1, DecodedCanonicalTemplateValueParametersV1,
    DecodedExportDefaultBodyV1, DecodedExportDefaultReferenceSetV1,
    DecodedExportDefaultTemplateKeyV1, DecodedOptionalTemplateReceiverV1,
    DecodedPersistentLexicalRootV1, ExportDefaultBodyIndexError, ExportDefaultBodyResolutionError,
    ExportDefaultBodyV1, ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceSetValidationError, ExportDefaultTemplateKeyV1,
    IndexedCanonicalTemplateValueParametersV1, IndexedExportDefaultBodyV1,
    IndexedOptionalTemplateReceiverV1, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    TemplateLocalLookupError, TemplateLocalTableValidationError, TemplateReceiverIndexError,
    TemplateReceiverResolutionError, TemplateValueParameterListIndexError,
    TemplateValueParameterListValidationError,
};
use crate::{
    CanonicalBooleanV1, DecodedExportDefinitionSourceV1, DefaultStatementReferenceResolver,
    ExportDefinitionSourceV1,
};

mod semantics;

pub use semantics::{
    DefaultTemplateOriginSemanticAuthority, ExportDefaultTemplateContractSemanticValidationError,
    ExportDefaultTemplateOriginSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportDefaultTemplateV1 {
    key: ExportDefaultTemplateKeyV1,
    definition_root: PersistentLexicalRootV1,
    definition_path: StructuralDefinitionPath,
    locals: CanonicalTemplateLocalTableV1,
    body: ExportDefaultBodyV1,
    result: SignatureTypeKey,
    allows_suspend: CanonicalBooleanV1,
    type_parameters: CanonicalBinderUseListV1,
    receiver: OptionalTemplateReceiverV1,
    value_parameters: CanonicalTemplateValueParametersV1,
    references: ExportDefaultReferenceSetV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl ExportDefaultTemplateV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        key: ExportDefaultTemplateKeyV1,
        definition_root: PersistentLexicalRootV1,
        definition_path: StructuralDefinitionPath,
        locals: CanonicalTemplateLocalTableV1,
        body: ExportDefaultBodyV1,
        result: SignatureTypeKey,
        allows_suspend: CanonicalBooleanV1,
        type_parameters: CanonicalBinderUseListV1,
        receiver: OptionalTemplateReceiverV1,
        value_parameters: CanonicalTemplateValueParametersV1,
        references: ExportDefaultReferenceSetV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, ExportDefaultTemplateBuildError> {
        if body.value().result_type() != &result {
            return Err(ExportDefaultTemplateBuildError::ResultType {
                declared: result,
                body: body.value().result_type().clone(),
            });
        }

        let mut local_index = locals.clone();
        body.index_locals(&mut local_index)
            .map_err(ExportDefaultTemplateBuildError::BodyLocal)?;
        receiver
            .index_local(&mut local_index)
            .map_err(ExportDefaultTemplateBuildError::ReceiverLocal)?;
        value_parameters
            .index_locals(&mut local_index)
            .map_err(ExportDefaultTemplateBuildError::ValueParameterLocal)?;
        validate_reference_owners(key, &references)?;

        Ok(Self {
            key,
            definition_root,
            definition_path,
            locals,
            body,
            result,
            allows_suspend,
            type_parameters,
            receiver,
            value_parameters,
            references,
            definition_origin,
        })
    }

    pub const fn key(&self) -> ExportDefaultTemplateKeyV1 {
        self.key
    }

    pub const fn definition_root(&self) -> PersistentLexicalRootV1 {
        self.definition_root
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.definition_path
    }

    pub const fn locals(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.locals
    }

    pub const fn body(&self) -> &ExportDefaultBodyV1 {
        &self.body
    }

    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }

    pub const fn allows_suspend(&self) -> CanonicalBooleanV1 {
        self.allows_suspend
    }

    pub const fn type_parameters(&self) -> &CanonicalBinderUseListV1 {
        &self.type_parameters
    }

    pub const fn receiver(&self) -> &OptionalTemplateReceiverV1 {
        &self.receiver
    }

    pub const fn value_parameters(&self) -> &CanonicalTemplateValueParametersV1 {
        &self.value_parameters
    }

    pub const fn references(&self) -> &ExportDefaultReferenceSetV1 {
        &self.references
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportDefaultTemplateV1<'_>, ExportDefaultTemplateIndexError> {
        let mut local_index = self.locals.clone();
        let body = self
            .body
            .index_locals(&mut local_index)
            .map_err(ExportDefaultTemplateIndexError::Body)?;
        let receiver = self
            .receiver
            .index_local(&mut local_index)
            .map_err(ExportDefaultTemplateIndexError::Receiver)?;
        let value_parameters = self
            .value_parameters
            .index_locals(&mut local_index)
            .map_err(ExportDefaultTemplateIndexError::ValueParameters)?;
        Ok(IndexedExportDefaultTemplateV1 {
            template: self,
            body,
            receiver,
            value_parameters,
        })
    }
}

#[derive(Debug)]
pub struct IndexedExportDefaultTemplateV1<'a> {
    template: &'a ExportDefaultTemplateV1,
    body: IndexedExportDefaultBodyV1<'a>,
    receiver: IndexedOptionalTemplateReceiverV1<'a>,
    value_parameters: IndexedCanonicalTemplateValueParametersV1<'a>,
}

impl WireEncode for IndexedExportDefaultTemplateV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.template.key.encode(encoder)?;
        encoder.field(2)?;
        self.template.definition_root.encode(encoder)?;
        encoder.field(3)?;
        self.template.definition_path.encode(encoder)?;
        encoder.field(4)?;
        self.template.locals.encode(encoder)?;
        encoder.field(5)?;
        self.body.encode(encoder)?;
        encoder.field(6)?;
        self.template.result.encode(encoder)?;
        encoder.field(7)?;
        self.template.allows_suspend.encode(encoder)?;
        encoder.field(8)?;
        self.template.type_parameters.encode(encoder)?;
        encoder.field(9)?;
        self.receiver.encode(encoder)?;
        encoder.field(10)?;
        self.value_parameters.encode(encoder)?;
        encoder.field(11)?;
        self.template.references.encode(encoder)?;
        encoder.field(12)?;
        self.template.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultTemplateV1 {
    key: DecodedExportDefaultTemplateKeyV1,
    definition_root: DecodedPersistentLexicalRootV1,
    definition_path: StructuralDefinitionPath,
    locals: DecodedCanonicalTemplateLocalTableV1,
    body: DecodedExportDefaultBodyV1,
    result: DecodedSignatureTypeKey,
    allows_suspend: CanonicalBooleanV1,
    type_parameters: DecodedCanonicalBinderUseListV1,
    receiver: DecodedOptionalTemplateReceiverV1,
    value_parameters: DecodedCanonicalTemplateValueParametersV1,
    references: DecodedExportDefaultReferenceSetV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedExportDefaultTemplateV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportDefaultTemplateV1, ExportDefaultTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        self.resolve_metered(
            resolver,
            &mut scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default()),
            &scoop_wire::WirePath::root(),
        )
    }

    pub fn resolve_metered<R, E>(
        self,
        resolver: &mut R,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<ExportDefaultTemplateV1, ExportDefaultTemplateResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        let key = self
            .key
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::Key)?;
        let definition_root = self
            .definition_root
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::DefinitionRoot)?;
        let mut locals = self
            .locals
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::Locals)?;
        let body = self
            .body
            .resolve(resolver, &mut locals)
            .map_err(ExportDefaultTemplateResolutionError::Body)?;
        let result = self
            .result
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::Result)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::TypeParameters)?;
        let receiver = self
            .receiver
            .resolve(resolver, &mut locals)
            .map_err(ExportDefaultTemplateResolutionError::Receiver)?;
        let value_parameters = self
            .value_parameters
            .resolve(&mut locals)
            .map_err(ExportDefaultTemplateResolutionError::ValueParameters)?;
        let references = self
            .references
            .resolve_metered(resolver, meter, &path.clone().field(11))
            .map_err(ExportDefaultTemplateResolutionError::References)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(ExportDefaultTemplateResolutionError::DefinitionOrigin)?;
        ExportDefaultTemplateV1::try_new(
            key,
            definition_root,
            self.definition_path,
            locals,
            body,
            result,
            self.allows_suspend,
            type_parameters,
            receiver,
            value_parameters,
            references,
            definition_origin,
        )
        .map_err(ExportDefaultTemplateResolutionError::Record)
    }
}

impl WireEncode for DecodedExportDefaultTemplateV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.key.encode(encoder)?;
        encoder.field(2)?;
        self.definition_root.encode(encoder)?;
        encoder.field(3)?;
        self.definition_path.encode(encoder)?;
        encoder.field(4)?;
        self.locals.encode(encoder)?;
        encoder.field(5)?;
        self.body.encode(encoder)?;
        encoder.field(6)?;
        self.result.encode(encoder)?;
        encoder.field(7)?;
        self.allows_suspend.encode(encoder)?;
        encoder.field(8)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(9)?;
        self.receiver.encode(encoder)?;
        encoder.field(10)?;
        self.value_parameters.encode(encoder)?;
        encoder.field(11)?;
        self.references.encode(encoder)?;
        encoder.field(12)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedExportDefaultTemplateV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(12)?;
        Ok(Self {
            key: decoder.field(1, DecodedExportDefaultTemplateKeyV1::decode)?,
            definition_root: decoder.field(2, DecodedPersistentLexicalRootV1::decode)?,
            definition_path: decoder.field(3, StructuralDefinitionPath::decode)?,
            locals: decoder.field(4, DecodedCanonicalTemplateLocalTableV1::decode)?,
            body: decoder.field(5, DecodedExportDefaultBodyV1::decode)?,
            result: decoder.field(6, DecodedSignatureTypeKey::decode)?,
            allows_suspend: decoder.field(7, CanonicalBooleanV1::decode)?,
            type_parameters: decoder.field(8, DecodedCanonicalBinderUseListV1::decode)?,
            receiver: decoder.field(9, DecodedOptionalTemplateReceiverV1::decode)?,
            value_parameters: decoder
                .field(10, DecodedCanonicalTemplateValueParametersV1::decode)?,
            references: decoder.field(11, DecodedExportDefaultReferenceSetV1::decode)?,
            definition_origin: decoder.field(12, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateBuildError {
    ResultType {
        declared: SignatureTypeKey,
        body: SignatureTypeKey,
    },
    BodyLocal(ExportDefaultBodyIndexError<TemplateLocalLookupError>),
    ReceiverLocal(TemplateReceiverIndexError<TemplateLocalLookupError>),
    ValueParameterLocal(TemplateValueParameterListIndexError<TemplateLocalLookupError>),
    ReferenceOwner {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        expected: scoop_identity::CallableTemplateOrigin,
        actual: scoop_identity::CallableTemplateOrigin,
    },
}

impl fmt::Display for ExportDefaultTemplateBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResultType { declared, body } => write!(
                formatter,
                "default-template result type {declared:?} differs from body result {body:?}"
            ),
            Self::BodyLocal(error) => write!(formatter, "invalid default body local: {error}"),
            Self::ReceiverLocal(error) => {
                write!(formatter, "invalid default receiver local: {error}")
            }
            Self::ValueParameterLocal(error) => {
                write!(formatter, "invalid default value-parameter local: {error}")
            }
            Self::ReferenceOwner {
                kind,
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default {kind} reference {index} has witness owner {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for ExportDefaultTemplateBuildError {}

pub type ExportDefaultTemplateIndexError =
    ExportDefaultTemplateLocalIndexError<TemplateLocalLookupError>;

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateLocalIndexError<E> {
    Body(ExportDefaultBodyIndexError<E>),
    Receiver(TemplateReceiverIndexError<E>),
    ValueParameters(TemplateValueParameterListIndexError<E>),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateLocalIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body(error) => write!(formatter, "cannot index default body: {error}"),
            Self::Receiver(error) => write!(formatter, "cannot index default receiver: {error}"),
            Self::ValueParameters(error) => {
                write!(formatter, "cannot index default value parameters: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExportDefaultTemplateLocalIndexError<E> {}

#[derive(Debug)]
pub enum ExportDefaultTemplateResolutionError<E> {
    Key(E),
    DefinitionRoot(E),
    Locals(TemplateLocalTableValidationError<E>),
    Body(ExportDefaultBodyResolutionError<E, TemplateLocalLookupError>),
    Result(E),
    TypeParameters(BinderUseListValidationError<E>),
    Receiver(TemplateReceiverResolutionError<E, TemplateLocalLookupError>),
    ValueParameters(TemplateValueParameterListValidationError<TemplateLocalLookupError>),
    References(ExportDefaultReferenceSetValidationError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(ExportDefaultTemplateBuildError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(error) => write!(formatter, "invalid default-template key: {error}"),
            Self::DefinitionRoot(error) => {
                write!(
                    formatter,
                    "invalid default-template definition root: {error}"
                )
            }
            Self::Locals(error) => write!(formatter, "invalid default-template locals: {error}"),
            Self::Body(error) => write!(formatter, "invalid default-template body: {error}"),
            Self::Result(error) => write!(formatter, "invalid default-template result: {error}"),
            Self::TypeParameters(error) => {
                write!(
                    formatter,
                    "invalid default-template type parameters: {error}"
                )
            }
            Self::Receiver(error) => {
                write!(formatter, "invalid default-template receiver: {error}")
            }
            Self::ValueParameters(error) => {
                write!(
                    formatter,
                    "invalid default-template value parameters: {error}"
                )
            }
            Self::References(error) => {
                write!(formatter, "invalid default-template references: {error}")
            }
            Self::DefinitionOrigin(error) => {
                write!(
                    formatter,
                    "invalid default-template definition origin: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExportDefaultTemplateResolutionError<E> {}

fn validate_reference_owners(
    key: ExportDefaultTemplateKeyV1,
    references: &ExportDefaultReferenceSetV1,
) -> Result<(), ExportDefaultTemplateBuildError> {
    let expected = key.owner();
    validate_reference_owner(
        references.callables(),
        ExportDefaultReferenceKindV1::Callable,
        expected,
    )?;
    validate_reference_owner(
        references.constructors(),
        ExportDefaultReferenceKindV1::Constructor,
        expected,
    )?;
    validate_reference_owner(
        references.types(),
        ExportDefaultReferenceKindV1::Type,
        expected,
    )?;
    validate_reference_owner(
        references.globals(),
        ExportDefaultReferenceKindV1::Global,
        expected,
    )?;
    validate_reference_owner(
        references.singleton_values(),
        ExportDefaultReferenceKindV1::Singleton,
        expected,
    )?;
    validate_reference_owner(
        references.fields(),
        ExportDefaultReferenceKindV1::Field,
        expected,
    )
}

fn validate_reference_owner<T>(
    references: &[super::ExportDefaultReferenceV1<T>],
    kind: ExportDefaultReferenceKindV1,
    expected: scoop_identity::CallableTemplateOrigin,
) -> Result<(), ExportDefaultTemplateBuildError> {
    for (index, reference) in references.iter().enumerate() {
        let actual = reference.witness().owner();
        if actual != expected {
            return Err(ExportDefaultTemplateBuildError::ReferenceOwner {
                kind,
                index,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
