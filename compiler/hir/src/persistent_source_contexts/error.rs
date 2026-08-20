use std::fmt;

use scoop_identity::PersistentSourceContextId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSourceContextReferenceKind {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
    Function,
    StructConstructor,
    ClassConstructor,
    Property,
    InitializationUnit,
    SingletonValue,
}

impl fmt::Display for HirSourceContextReferenceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Object => "object",
            Self::Function => "function",
            Self::StructConstructor => "struct constructor",
            Self::ClassConstructor => "class constructor",
            Self::Property => "property",
            Self::InitializationUnit => "initialization unit",
            Self::SingletonValue => "singleton value",
        })
    }
}

#[derive(Debug)]
pub enum HirSourceContextIdentityError {
    Length {
        expected: usize,
        actual: usize,
    },
    DuplicateSource {
        first: usize,
        duplicate: usize,
    },
    UnknownSource {
        context: u32,
    },
    MissingFileContext {
        source: usize,
    },
    UnknownReference {
        context: u32,
        kind: HirSourceContextReferenceKind,
        target: u32,
    },
    NonSourceNominal {
        context: u32,
    },
    InvalidFunctionSubject {
        context: u32,
    },
    InvalidConstructorSubject {
        context: u32,
    },
    InvalidLexicalSubject {
        context: u32,
    },
    ConflictingLexicalIdentity {
        context: u32,
    },
    SourceMismatch {
        context: u32,
    },
    IdentityMismatch {
        context: u32,
    },
    DuplicateIdentity {
        context: u32,
        identity: PersistentSourceContextId,
    },
    InvalidIdentity {
        context: u32,
        error: scoop_wire::HashError,
    },
}

impl fmt::Display for HirSourceContextIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => write!(
                formatter,
                "source-context identity table has {actual} entries, expected {expected}"
            ),
            Self::DuplicateSource { first, duplicate } => write!(
                formatter,
                "source metadata {duplicate} duplicates source metadata {first}"
            ),
            Self::UnknownSource { context } => write!(
                formatter,
                "source context {context} refers to a source absent from the module"
            ),
            Self::MissingFileContext { source } => write!(
                formatter,
                "source metadata {source} has no file source context"
            ),
            Self::UnknownReference {
                context,
                kind,
                target,
            } => write!(
                formatter,
                "source context {context} refers to unknown {kind} {target}"
            ),
            Self::NonSourceNominal { context } => write!(
                formatter,
                "source context {context} uses a generated nominal as a source declaration"
            ),
            Self::InvalidFunctionSubject { context } => write!(
                formatter,
                "source context {context} has no source-backed callable subject"
            ),
            Self::InvalidConstructorSubject { context } => write!(
                formatter,
                "source context {context} has no source-backed constructor subject"
            ),
            Self::InvalidLexicalSubject { context } => write!(
                formatter,
                "source context {context} has no matching lexical callable"
            ),
            Self::ConflictingLexicalIdentity { context } => write!(
                formatter,
                "source context {context} maps one lexical site to conflicting callable identities"
            ),
            Self::SourceMismatch { context } => write!(
                formatter,
                "source context {context} does not belong to its declared source"
            ),
            Self::IdentityMismatch { context } => write!(
                formatter,
                "source context {context} has a persistent identity for the wrong semantic key"
            ),
            Self::DuplicateIdentity { context, identity } => write!(
                formatter,
                "source context {context} duplicates persistent source-context identity {identity}"
            ),
            Self::InvalidIdentity { context, error } => write!(
                formatter,
                "source context {context} has an invalid persistent identity: {error}"
            ),
        }
    }
}

impl std::error::Error for HirSourceContextIdentityError {}
