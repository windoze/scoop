use super::*;

#[derive(Debug)]
pub enum HirDependencyCallSignatureError {
    Resource(WireError),
    Type(Box<SharedTypeMetadataError>),
    Target(ExternalHirTargetV1),
    Declaration(CallableTemplateOrigin),
    GenericDeclaration(CallableTemplateOrigin),
    ApplicationOrigin {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    ApplicationOwner(CallableTemplateOrigin),
    ApplicationArity {
        expected: usize,
        actual: usize,
    },
    RedundantApplication(CallableTemplateOrigin),
    ReceiverRole {
        expected: bool,
        actual: bool,
    },
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    Argument {
        index: usize,
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
    Result {
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
}

impl From<WireError> for HirDependencyCallSignatureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<SharedTypeMetadataError> for HirDependencyCallSignatureError {
    fn from(error: SharedTypeMetadataError) -> Self {
        Self::Type(Box::new(error))
    }
}

impl std::fmt::Display for HirDependencyCallSignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Type(error) => error.fmt(f),
            Self::Target(target) => write!(
                f,
                "source call target {target:?} is not a concrete source callable"
            ),
            Self::Declaration(target) => write!(
                f,
                "source call target {target:?} has no provider declaration"
            ),
            Self::GenericDeclaration(target) => write!(
                f,
                "source call target {target:?} has unresolved type parameters"
            ),
            Self::ApplicationOrigin { expected, actual } => write!(
                f,
                "source call application names {actual:?}, expected {expected:?}"
            ),
            Self::ApplicationOwner(declaration) => write!(
                f,
                "source call application has an invalid owner for {declaration:?}"
            ),
            Self::ApplicationArity { expected, actual } => write!(
                f,
                "source call application has {actual} type arguments, expected {expected}"
            ),
            Self::RedundantApplication(declaration) => write!(
                f,
                "source call to {declaration:?} has no type substitution and requires a direct target"
            ),
            Self::ArgumentCount { expected, actual } => write!(
                f,
                "source call has {actual} logical arguments, expected {expected}"
            ),
            Self::ReceiverRole { expected, actual } => write!(
                f,
                "source call receiver presence is {actual}, expected {expected} from its declaration"
            ),
            Self::Argument {
                index,
                expected,
                actual,
            } => write!(
                f,
                "source call argument {index} has exact type {actual}, expected {expected}"
            ),
            Self::Result { expected, actual } => write!(
                f,
                "source call has result exact type {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for HirDependencyCallSignatureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Type(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
