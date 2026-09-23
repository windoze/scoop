use std::fmt;

use super::{
    DefaultBodyProjectionError, DefaultEntityProjectionError, DefaultReferenceProjectionError,
    DefaultTemplateEnvelopeProjectionError, DefaultTemplateProductionError,
};

impl fmt::Display for DefaultTemplateProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableInterfaces(source) => {
                write!(formatter, "cannot project callable authority: {source}")
            }
            Self::SourceInterfaces(source) => {
                write!(
                    formatter,
                    "cannot project callable source interfaces: {source}"
                )
            }
            Self::MissingSourceInterface(owner) => {
                write!(formatter, "default owner {owner:?} has no source interface")
            }
            Self::DuplicateSourceInterface(owner) => write!(
                formatter,
                "default owner {owner:?} has more than one source interface"
            ),
            Self::MissingCanonicalSourceInterface(owner) => write!(
                formatter,
                "default owner {owner:?} has no canonical source interface"
            ),
            Self::SourceParameterArity {
                owner,
                hir,
                canonical,
            } => write!(
                formatter,
                "default owner {owner:?} has {hir} HIR source parameters but {canonical} canonical source parameters"
            ),
            Self::TooManySourceParameters { owner } => {
                write!(
                    formatter,
                    "default owner {owner:?} has more than u32 source parameters"
                )
            }
            Self::Template { key, source } => {
                write!(
                    formatter,
                    "cannot project default template {key:?}: {source}"
                )
            }
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for DefaultTemplateEnvelopeProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::UnknownDefaultSource(source) => {
                write!(formatter, "default source {source} is unknown")
            }
            Self::UnknownDefaultExpression(expression) => {
                write!(formatter, "default expression {expression} is unknown")
            }
            Self::Provider(source) => {
                write!(formatter, "cannot project default provider: {source}")
            }
            Self::TypeParameterArity { expected, actual } => write!(
                formatter,
                "default provider has {actual} type parameters but its owner requires {expected}"
            ),
            Self::TypeParameterIdentity { position } => write!(
                formatter,
                "default provider type parameter {position} has the wrong identity"
            ),
            Self::BinderUse(source) => source.fmt(formatter),
            Self::Signature(source) => source.fmt(formatter),
            Self::DefinitionOrigin(source) => source.fmt(formatter),
            Self::DuplicateLocalBinding(local) => {
                write!(formatter, "default local binding {local} is duplicated")
            }
            Self::LocalRecord { local, source } => {
                write!(formatter, "cannot project default local {local}: {source}")
            }
            Self::LocalTable(source) => source.fmt(formatter),
            Self::Receiver(source) => source.fmt(formatter),
            Self::ValueParameter { index, source } => write!(
                formatter,
                "cannot project default value parameter {index}: {source}"
            ),
            Self::ValueParameters(source) => source.fmt(formatter),
            Self::Body(source) => source.fmt(formatter),
            Self::References(source) => source.fmt(formatter),
            Self::Record(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for DefaultEntityProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Unknown { kind, index } => {
                write!(formatter, "default body references unknown {kind} {index}")
            }
            Self::MissingIdentity { kind, index } => write!(
                formatter,
                "default body {kind} {index} has no persistent identity"
            ),
            Self::UnsupportedFunctionIdentity { function } => write!(
                formatter,
                "default body function {function} has an unsupported identity"
            ),
            Self::ExpectedSourceDeclaration { function } => write!(
                formatter,
                "default body function {function} is not a source declaration"
            ),
            Self::InvalidLocalFunctionBinders { local_function } => write!(
                formatter,
                "default local function {local_function} has an invalid binder layout"
            ),
            Self::ExpectedOrdinaryProperty { property } => write!(
                formatter,
                "default body property {property} is not an ordinary property"
            ),
            Self::Type(source) => source.fmt(formatter),
            Self::Callable(source) => source.fmt(formatter),
            Self::GeneratedIdentity(source) => source.fmt(formatter),
            Self::ImportedDependencyUnavailable(function) => write!(
                formatter,
                "default body dependency callable {function} cannot be resolved without an ordinary selected set"
            ),
        }
    }
}

impl fmt::Display for DefaultBodyProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Entity(source) => source.fmt(formatter),
            Self::Signature(source) => source.fmt(formatter),
            Self::DefinitionOrigin(source) => source.fmt(formatter),
            Self::UnknownLocal(local) => {
                write!(formatter, "default body references unknown local {local}")
            }
            Self::UnknownBinding(binding) => {
                write!(
                    formatter,
                    "default body references unknown binding {binding}"
                )
            }
            Self::InvalidCaptureSource => {
                formatter.write_str("default body capture source is not canonical")
            }
            Self::InvalidLiteralPattern => {
                formatter.write_str("default body literal pattern is not canonical")
            }
            Self::UnsupportedExpression(kind) => {
                write!(
                    formatter,
                    "default body contains unsupported expression {kind}"
                )
            }
            Self::UnsupportedAssignment(kind) => {
                write!(
                    formatter,
                    "default body contains unsupported assignment {kind}"
                )
            }
            Self::LoopControlOutsideLoop(control) => {
                write!(formatter, "default body {control} appears outside a loop")
            }
            Self::NonInnermostLoop {
                control,
                expected,
                actual,
            } => write!(
                formatter,
                "default body {control} targets loop {actual}, not innermost loop {expected}"
            ),
            Self::TooManyOwnerTypeParameters => {
                formatter.write_str("default owner has more than u32 type parameters")
            }
            Self::BodyTypeArguments(source) => source.fmt(formatter),
            Self::Expression(source) => source.fmt(formatter),
            Self::Statement(source) => source.fmt(formatter),
            Self::Pattern(source) => source.fmt(formatter),
            Self::ControlFlow(source) => source.fmt(formatter),
            Self::BindingShape(source) => source.fmt(formatter),
            Self::BindingProjection(source) => source.fmt(formatter),
            Self::BindingAction(source) => source.fmt(formatter),
            Self::BindingPlan(source) => source.fmt(formatter),
            Self::ForPlan(source) => source.fmt(formatter),
            Self::ArrayAssembly(source) => source.fmt(formatter),
            Self::LocalFunction(source) => source.fmt(formatter),
            Self::LexicalCallable(source) => source.fmt(formatter),
            Self::CallableReference(source) => source.fmt(formatter),
            Self::Body(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for DefaultReferenceProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(source) => source.fmt(formatter),
            Self::Entity(source) => source.fmt(formatter),
            Self::Signature(source) => source.fmt(formatter),
            Self::DefinitionOrigin(source) => source.fmt(formatter),
            Self::RestrictedTarget { kind, index } => write!(
                formatter,
                "default reference {kind:?}[{index}] targets a restricted entity"
            ),
            Self::Owner {
                kind,
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default reference {kind:?}[{index}] is owned by {actual:?}, not {expected:?}"
            ),
            Self::InvalidCallDomain { kind, index } => write!(
                formatter,
                "default reference {kind:?}[{index}] uses the wrong call domain"
            ),
            Self::Set(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for DefaultTemplateProductionError {}
