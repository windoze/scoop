use super::*;
use scoop_hir::{
    DefaultSourceTargetSubjectError, DefaultTemplateTypeSubstitutionError,
    ExportDefaultReferenceKindV1, ExportDefaultTemplateKeyV1,
};
use scoop_wire::WireError;

#[derive(Debug)]
pub enum CrossConeHirDefaultCallDomainError {
    Resource(WireError),
    Source(Box<DefaultSourceTargetSubjectError>),
    DeclarationSource(Box<super::super::CrossConeHirNominalAuthorityError>),
    Substitution(Box<DefaultTemplateTypeSubstitutionError>),
    Encoding(String),
    MissingDeclaration(CallableTemplateOrigin),
    CallableRole(CallableTemplateOrigin),
    CallableCycle(CallableTemplateOrigin),
    NominalCycle(SourceNominalId),
    SupertypeShape,
    SupertypeArity {
        owner: SourceNominalId,
        expected: u32,
        actual: usize,
    },
    MultipleSuperclasses(SourceNominalId),
    SignatureEffects(CallableTemplateOrigin),
    FinalOverride(CallableTemplateOrigin),
    PhysicalSlot,
    SlotCoverage(CallableTemplateOrigin),
    PublicLookup,
    OriginalProviderOverride,
    InheritedProvider,
    ProviderMapping,
    Witness {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        reason: &'static str,
    },
    Declaration {
        declaration: CallableTemplateOrigin,
        source: Box<Self>,
    },
    Template {
        key: ExportDefaultTemplateKeyV1,
        source: Box<Self>,
    },
}

impl From<WireError> for Error {
    fn from(e: WireError) -> Self {
        Self::Resource(e)
    }
}
impl From<DefaultSourceTargetSubjectError> for Error {
    fn from(e: DefaultSourceTargetSubjectError) -> Self {
        Self::Source(Box::new(e))
    }
}
impl From<super::super::CrossConeHirNominalAuthorityError> for Error {
    fn from(e: super::super::CrossConeHirNominalAuthorityError) -> Self {
        Self::DeclarationSource(Box::new(e))
    }
}
impl From<DefaultTemplateTypeSubstitutionError> for Error {
    fn from(e: DefaultTemplateTypeSubstitutionError) -> Self {
        Self::Substitution(Box::new(e))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::DeclarationSource(e) => e.fmt(f),
            Self::Substitution(e) => e.fmt(f),
            Self::Encoding(e) => f.write_str(e),
            Self::MissingDeclaration(id) => {
                write!(f, "missing actual default callable declaration {id:?}")
            }
            Self::CallableRole(id) => write!(f, "invalid default publisher role {id:?}"),
            Self::CallableCycle(id) => write!(f, "cyclic default callable inheritance at {id:?}"),
            Self::NominalCycle(id) => write!(f, "cyclic source nominal inheritance at {id:?}"),
            Self::SupertypeShape => {
                f.write_str("source superclass or interface must be a nominal application")
            }
            Self::SupertypeArity {
                owner,
                expected,
                actual,
            } => write!(
                f,
                "ancestor {owner:?} requires {expected} arguments, received {actual}"
            ),
            Self::MultipleSuperclasses(owner) => {
                write!(f, "source nominal {owner:?} has multiple superclasses")
            }
            Self::SignatureEffects(id) => write!(
                f,
                "default callable differs from inherited execution/operator/infix contract {id:?}"
            ),
            Self::FinalOverride(id) => {
                write!(f, "default callable overrides final source method {id:?}")
            }
            Self::PhysicalSlot => {
                f.write_str("default callable slot does not match its actual source inheritance")
            }
            Self::SlotCoverage(id) => write!(
                f,
                "default callable slot domain does not cover inherited method {id:?}"
            ),
            Self::PublicLookup => f.write_str(
                "public callable lookup differs from its actual direct and slot domains",
            ),
            Self::OriginalProviderOverride => {
                f.write_str("an original default declaration cannot be an override")
            }
            Self::InheritedProvider => {
                f.write_str("default provider is not an actual inherited declaration")
            }
            Self::ProviderMapping => f.write_str(
                "default provider mapping differs from the actual inherited application",
            ),
            Self::Witness {
                kind,
                index,
                reason,
            } => write!(f, "default {kind} witness {index}: {reason}"),
            Self::Declaration {
                declaration,
                source,
            } => write!(f, "source callable {declaration:?}: {source}"),
            Self::Template { key, source } => write!(f, "default template {key:?}: {source}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Source(e) => Some(e.as_ref()),
            Self::DeclarationSource(e) => Some(e.as_ref()),
            Self::Substitution(e) => Some(e.as_ref()),
            Self::Declaration { source, .. } | Self::Template { source, .. } => {
                Some(source.as_ref())
            }
            _ => None,
        }
    }
}
