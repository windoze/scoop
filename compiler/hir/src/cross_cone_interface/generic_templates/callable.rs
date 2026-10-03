use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

use super::GenericTemplatePredicatesV1;
use crate::{
    CallableImplementationV1, CallableSourceEffectsV1, CanonicalBinderUseListV1,
    CanonicalTemplateLocalTableV1, DefaultCallableDeclarationV1, DefaultStatementIndexError,
    DefaultStatementV1, ExportDefinitionSourceV1, IndexedDefaultStatementV1,
    TemplateLocalLookupError,
};

mod captures;
mod decode;
pub use decode::*;

/// One implementation with its declaration identity and source binder scope.
/// Parameters include the actual receiver and lexical capture ABI parameters.
/// The statement body has explicit returns, unlike a default expression root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGenericCallableBodyV1 {
    owner: DefaultCallableDeclarationV1,
    locals: CanonicalTemplateLocalTableV1,
    parameters: Vec<LocalValueSelector>,
    statements: Vec<DefaultStatementV1>,
    result: SignatureTypeKey,
    effects: CallableSourceEffectsV1,
    type_parameters: CanonicalBinderUseListV1,
    predicates: GenericTemplatePredicatesV1,
    definition_origin: ExportDefinitionSourceV1,
    capture_types: Vec<SignatureTypeKey>,
}

impl ExportGenericCallableBodyV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        owner: DefaultCallableDeclarationV1,
        locals: CanonicalTemplateLocalTableV1,
        parameters: Vec<LocalValueSelector>,
        statements: Vec<DefaultStatementV1>,
        result: SignatureTypeKey,
        effects: CallableSourceEffectsV1,
        type_parameters: CanonicalBinderUseListV1,
        predicates: GenericTemplatePredicatesV1,
        definition_origin: ExportDefinitionSourceV1,
        capture_types: Vec<SignatureTypeKey>,
    ) -> Result<Self, GenericCallableBodyBuildError> {
        if effects.implementation() != CallableImplementationV1::Scoop {
            return Err(GenericCallableBodyBuildError::BodylessImplementation(
                effects.implementation(),
            ));
        }
        u32::try_from(parameters.len())
            .map_err(|_| GenericCallableBodyBuildError::TooManyParameters)?;
        let mut seen = BTreeSet::new();
        for (index, parameter) in parameters.iter().enumerate() {
            if locals.get(parameter).is_none() {
                return Err(GenericCallableBodyBuildError::MissingParameter {
                    index,
                    selector: parameter.clone(),
                });
            }
            if !seen.insert(parameter) {
                return Err(GenericCallableBodyBuildError::DuplicateParameter {
                    index,
                    selector: parameter.clone(),
                });
            }
        }
        u32::try_from(capture_types.len())
            .map_err(|_| GenericCallableBodyBuildError::TooManyCaptures)?;
        let body = Self {
            owner,
            locals,
            parameters,
            statements,
            result,
            effects,
            type_parameters,
            predicates,
            definition_origin,
            capture_types,
        };
        captures::validate(&body)?;
        Ok(body)
    }

    pub const fn owner(&self) -> DefaultCallableDeclarationV1 {
        self.owner
    }

    pub const fn locals(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.locals
    }

    pub fn parameters(&self) -> &[LocalValueSelector] {
        &self.parameters
    }

    pub fn statements(&self) -> &[DefaultStatementV1] {
        &self.statements
    }

    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }

    pub fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects.clone()
    }

    pub const fn type_parameters(&self) -> &CanonicalBinderUseListV1 {
        &self.type_parameters
    }

    pub const fn predicates(&self) -> &GenericTemplatePredicatesV1 {
        &self.predicates
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub fn capture_types(&self) -> &[SignatureTypeKey] {
        &self.capture_types
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportGenericCallableBodyV1<'_>, GenericCallableBodyIndexError> {
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| {
                self.locals
                    .index_of(parameter)
                    .expect("the immutable parameter map was checked when the body was constructed")
            })
            .collect();
        let mut locals = self.locals.clone();
        let statements = self
            .statements
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                statement.index_locals(&mut locals).map_err(|source| {
                    GenericCallableBodyIndexError {
                        index,
                        source: Box::new(source),
                    }
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(IndexedExportGenericCallableBodyV1 {
            body: self,
            parameters,
            statements,
        })
    }
}

#[derive(Debug)]
pub struct IndexedExportGenericCallableBodyV1<'a> {
    body: &'a ExportGenericCallableBodyV1,
    parameters: Vec<u32>,
    statements: Vec<IndexedDefaultStatementV1<'a>>,
}

impl WireEncode for IndexedExportGenericCallableBodyV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.body.owner.encode(encoder)?;
        encoder.field(2)?;
        self.body.locals.encode(encoder)?;
        encode_parameter_indices(encoder, &self.parameters)?;
        encoder.field(4)?;
        encoder.array(self.statements.len() as u64)?;
        for statement in &self.statements {
            statement.encode(encoder)?;
        }
        encoder.field(5)?;
        self.body.result.encode(encoder)?;
        encoder.field(6)?;
        self.body.effects.encode(encoder)?;
        encoder.field(7)?;
        self.body.type_parameters.encode(encoder)?;
        encoder.field(8)?;
        self.body.predicates.encode(encoder)?;
        encoder.field(9)?;
        self.body.definition_origin.encode(encoder)?;
        encoder.field(10)?;
        encoder.array(self.body.capture_types.len() as u64)?;
        for value_type in &self.body.capture_types {
            value_type.encode(encoder)?;
        }
        Ok(())
    }
}

fn encode_parameter_indices(
    encoder: &mut Encoder,
    parameters: &[u32],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(3)?;
    encoder.array(parameters.len() as u64)?;
    for &parameter in parameters {
        encoder.unsigned(u64::from(parameter))?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenericCallableBodyBuildError {
    BodylessImplementation(CallableImplementationV1),
    TooManyParameters,
    TooManyCaptures,
    CaptureIndex {
        index: u32,
        count: usize,
    },
    CaptureReferences(scoop_wire::WireError),
    MissingParameter {
        index: usize,
        selector: LocalValueSelector,
    },
    DuplicateParameter {
        index: usize,
        selector: LocalValueSelector,
    },
}

impl fmt::Display for GenericCallableBodyBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BodylessImplementation(implementation) => write!(
                formatter,
                "bodyless implementation {implementation:?} has a generic body"
            ),
            Self::TooManyParameters => {
                formatter.write_str("generic body parameter count exceeds u32")
            }
            Self::TooManyCaptures => formatter.write_str("generic body capture count exceeds u32"),
            Self::CaptureIndex { index, count } => write!(
                formatter,
                "generic body reads capture {index}, but has {count} capture inputs"
            ),
            Self::CaptureReferences(error) => error.fmt(formatter),
            Self::MissingParameter { index, selector } => write!(
                formatter,
                "generic body parameter {index} names missing local {selector:?}"
            ),
            Self::DuplicateParameter { index, selector } => write!(
                formatter,
                "generic body parameter {index} repeats local {selector:?}"
            ),
        }
    }
}

impl std::error::Error for GenericCallableBodyBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub struct GenericCallableBodyIndexError {
    pub index: usize,
    pub source: Box<DefaultStatementIndexError<TemplateLocalLookupError>>,
}

impl fmt::Display for GenericCallableBodyIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot index generic statement {}: {}",
            self.index, self.source
        )
    }
}

impl std::error::Error for GenericCallableBodyIndexError {}
