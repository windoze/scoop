use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableAbiValidationError<E> {
    Authority {
        kind: DefaultNestedCallableKindV1,
        query: DefaultNestedCallableAuthorityQueryV1,
        error: E,
    },
    DefinitionPath {
        kind: DefaultNestedCallableKindV1,
        expected: StructuralDefinitionPath,
        actual: StructuralDefinitionPath,
    },
    DefinitionPathNotDescendant {
        kind: DefaultNestedCallableKindV1,
        template: StructuralDefinitionPath,
        actual: StructuralDefinitionPath,
    },
    OwnerTypeParameterCount {
        kind: DefaultNestedCallableKindV1,
        expected: u32,
        actual: u32,
    },
    BodyArguments {
        kind: DefaultNestedCallableKindV1,
        expected: DefaultNestedCallableBodyShapeV1,
        actual: DefaultNestedCallableBodyShapeV1,
    },
    FunctionType {
        kind: DefaultNestedCallableKindV1,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    CaptureArity {
        kind: DefaultNestedCallableKindV1,
        expected: usize,
        actual: usize,
    },
    CaptureType {
        kind: DefaultNestedCallableKindV1,
        index: usize,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    DuplicateLocalFunction {
        declaration: CallableTemplateOrigin,
    },
    MissingLocalFunction {
        declaration: CallableTemplateOrigin,
        site: DefaultNestedCallableLocalUseV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for DefaultNestedCallableAbiValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority { kind, query, error } => write!(
                formatter,
                "default nested {kind:?} {query:?} authority failed: {error}"
            ),
            Self::DefinitionPath {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} definition path mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::DefinitionPathNotDescendant {
                kind,
                template,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} path {actual:?} is not a strict descendant of template path {template:?}"
            ),
            Self::OwnerTypeParameterCount {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} owner type-parameter count mismatch: expected {expected}, found {actual}"
            ),
            Self::BodyArguments {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} body arguments mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::FunctionType {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} function type mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::CaptureArity {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} capture count mismatch: expected {expected}, found {actual}"
            ),
            Self::CaptureType {
                kind,
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} capture {index} type mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::DuplicateLocalFunction { declaration } => write!(
                formatter,
                "default body declares local function {declaration:?} more than once in one lexical scope"
            ),
            Self::MissingLocalFunction { declaration, site } => write!(
                formatter,
                "default body {site:?} refers to undeclared local function {declaration:?}"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default nested callable ABI resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultNestedCallableAbiValidationError<E>
{
}
