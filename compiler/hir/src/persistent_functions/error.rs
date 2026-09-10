use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionIdentityRelation {
    PropertyGetter,
    PropertySetter,
    Lambda,
    AnonymousFunction,
    InitializationInitializer,
    InitializationEnsure,
    DerivedEqualityApplication,
    StructDerivedEquality,
    EnumDerivedEquality,
}

impl fmt::Display for FunctionIdentityRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PropertyGetter => "property getter",
            Self::PropertySetter => "property setter",
            Self::Lambda => "lambda",
            Self::AnonymousFunction => "anonymous function",
            Self::InitializationInitializer => "initialization initializer",
            Self::InitializationEnsure => "initialization ensure",
            Self::DerivedEqualityApplication => "derived equality application",
            Self::StructDerivedEquality => "struct derived equality",
            Self::EnumDerivedEquality => "enum derived equality",
        })
    }
}

#[derive(Debug)]
pub enum HirFunctionIdentityError {
    SourceIdentity(scoop_identity::SourceDeclarationIdentityError),
    GeneratedIdentity(scoop_identity::GeneratedCallableIdentityError),
    LexicalParent(scoop_identity::LexicalParentError),
    Length {
        expected: usize,
        actual: usize,
    },
    UnknownFunction {
        relation: FunctionIdentityRelation,
        owner: u32,
        function: u32,
    },
    ConflictingClaim {
        function: u32,
        first: FunctionIdentityRelation,
        second: FunctionIdentityRelation,
    },
    IdentityKind {
        function: u32,
    },
    AccessorIdentity {
        function: u32,
    },
    LexicalIdentity {
        function: u32,
    },
    InitializationIdentity {
        function: u32,
    },
    DerivedEqualityIdentity {
        function: u32,
    },
    OpenDerivedEqualityOwner {
        application: u32,
    },
    UnownedDerivedEqualityTemplate {
        function: u32,
    },
    DuplicatePlainIdentity {
        function: u32,
    },
    DuplicateGenericIdentity {
        function: u32,
    },
    DuplicateGeneratedIdentity {
        function: u32,
    },
}

impl fmt::Display for HirFunctionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceIdentity(error) => error.fmt(formatter),
            Self::GeneratedIdentity(error) => error.fmt(formatter),
            Self::LexicalParent(error) => error.fmt(formatter),
            Self::Length { expected, actual } => write!(
                formatter,
                "function identity table has {actual} entries, expected {expected}"
            ),
            Self::UnknownFunction {
                relation,
                owner,
                function,
            } => write!(
                formatter,
                "{relation} {owner} references unknown function {function}"
            ),
            Self::ConflictingClaim {
                function,
                first,
                second,
            } => write!(
                formatter,
                "function {function} is claimed by both {first} and {second}"
            ),
            Self::IdentityKind { function } => write!(
                formatter,
                "function {function} identity kind disagrees with its HIR relations"
            ),
            Self::AccessorIdentity { function } => {
                write!(
                    formatter,
                    "function {function} has an invalid accessor identity"
                )
            }
            Self::LexicalIdentity { function } => write!(
                formatter,
                "function {function} has an invalid lexical generated identity"
            ),
            Self::InitializationIdentity { function } => write!(
                formatter,
                "function {function} has an invalid initialization identity"
            ),
            Self::DerivedEqualityIdentity { function } => write!(
                formatter,
                "function {function} has an invalid derived equality identity relation"
            ),
            Self::OpenDerivedEqualityOwner { application } => write!(
                formatter,
                "derived equality application {application} has an open owner type"
            ),
            Self::UnownedDerivedEqualityTemplate { function } => write!(
                formatter,
                "unmaterialized derived equality function {function} has no nominal template owner"
            ),
            Self::DuplicatePlainIdentity { function } => write!(
                formatter,
                "function {function} duplicates another persistent plain function identity"
            ),
            Self::DuplicateGenericIdentity { function } => write!(
                formatter,
                "function {function} duplicates another persistent generic function identity"
            ),
            Self::DuplicateGeneratedIdentity { function } => write!(
                formatter,
                "function {function} duplicates another persistent generated callable identity"
            ),
        }
    }
}

impl std::error::Error for HirFunctionIdentityError {}
