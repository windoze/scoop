use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, PersistentSourceContextId,
    PropertyOwner, SourceIdentity,
};

use super::super::{DirectImportedTargetMergeError, ImportedTarget};
use crate::CoreClosedCallableClassificationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionPlanBuildError {
    Classification(CoreClosedCallableClassificationError),
    DuplicateCallable(CallableTemplateOrigin),
    MissingCallableSourceName(CallableTemplateOrigin),
    TooManyCallables {
        count: usize,
    },
    DuplicateConstant(PersistentPropertyId),
    DuplicateProperty(PropertyOwner),
    DuplicateTypeAlias(scoop_identity::PersistentTypeAliasId),
    TooManyConstants {
        count: usize,
    },
    TooManyTypeAliases {
        count: usize,
    },
    MissingTypeAliasExpansion(scoop_identity::PersistentTypeAliasId),
    MissingDefinitionSource {
        provider: ConeIdentity,
        source: SourceIdentity,
    },
    MissingDefinitionContext {
        provider: ConeIdentity,
        context: PersistentSourceContextId,
    },
    DirectBindingMerge {
        declaration: CallableTemplateOrigin,
        source: DirectImportedTargetMergeError,
    },
}

impl fmt::Display for ImportedDependencySelectionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Classification(error) => error.fmt(formatter),
            Self::DuplicateCallable(declaration) => write!(
                formatter,
                "dependency semantic world contains duplicate callable {declaration:?}"
            ),
            Self::MissingCallableSourceName(declaration) => write!(
                formatter,
                "dependency callable {declaration:?} has no canonical function source name"
            ),
            Self::TooManyCallables { count } => write!(
                formatter,
                "dependency semantic world contains {count} callables, exceeding the u32 id domain"
            ),
            Self::DuplicateConstant(property) => write!(
                formatter,
                "dependency semantic world contains duplicate constant {property:?}"
            ),
            Self::DuplicateProperty(property) => write!(
                formatter,
                "dependency semantic world contains duplicate property {property:?}"
            ),
            Self::DuplicateTypeAlias(alias) => write!(
                formatter,
                "dependency semantic world contains duplicate type alias {alias:?}"
            ),
            Self::TooManyConstants { count } => write!(
                formatter,
                "dependency semantic world contains {count} constants, exceeding the u32 id domain"
            ),
            Self::TooManyTypeAliases { count } => write!(
                formatter,
                "dependency semantic world contains {count} type aliases, exceeding the u32 id domain"
            ),
            Self::MissingTypeAliasExpansion(alias) => write!(
                formatter,
                "dependency type alias {alias:?} has no validated closure expansion"
            ),
            Self::MissingDefinitionSource { provider, source } => write!(
                formatter,
                "dependency provider {provider:?} has no HIR source record for exported definition source {source:?}"
            ),
            Self::MissingDefinitionContext { provider, context } => write!(
                formatter,
                "dependency provider {provider:?} has no HIR source context for exported definition context {context:?}"
            ),
            Self::DirectBindingMerge {
                declaration,
                source,
            } => write!(
                formatter,
                "dependency callable {declaration:?} has inconsistent direct binding routes: {source}"
            ),
        }
    }
}

impl std::error::Error for ImportedDependencySelectionPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Classification(error) => Some(error),
            Self::DirectBindingMerge { source, .. } => Some(source),
            Self::DuplicateCallable(_)
            | Self::MissingCallableSourceName(_)
            | Self::TooManyCallables { .. }
            | Self::DuplicateConstant(_)
            | Self::DuplicateProperty(_)
            | Self::DuplicateTypeAlias(_)
            | Self::TooManyConstants { .. }
            | Self::TooManyTypeAliases { .. }
            | Self::MissingTypeAliasExpansion(_)
            | Self::MissingDefinitionSource { .. }
            | Self::MissingDefinitionContext { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedDependencyCandidateError {
    NotCallable(ImportedTarget),
    NotConstant(ImportedTarget),
    NotProperty(ImportedTarget),
    NotTypeAlias(ImportedTarget),
    ForeignWorld,
    ForeignProjection,
    MissingCallable(CallableTemplateOrigin),
    MissingCallableSource(CallableTemplateOrigin),
    MissingConstant(PersistentPropertyId),
    MissingProperty(PropertyOwner),
    MissingTypeAlias(scoop_identity::PersistentTypeAliasId),
    MissingPropertySetter(PropertyOwner),
    RestrictedPropertySetter(PropertyOwner),
    TerminalProviderMismatch {
        declaration: CallableTemplateOrigin,
        expected: ConeIdentity,
    },
    ConstantTerminalProviderMismatch {
        property: PersistentPropertyId,
        expected: ConeIdentity,
    },
    PropertyTerminalProviderMismatch {
        property: PropertyOwner,
        expected: ConeIdentity,
    },
    TypeAliasTerminalProviderMismatch {
        alias: scoop_identity::PersistentTypeAliasId,
        expected: ConeIdentity,
    },
    MissingDefaultCallableBinding(CallableTemplateOrigin),
    GeneratedDefaultCallable,
}

impl fmt::Display for ImportedDependencyCandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCallable(target) => {
                write!(formatter, "imported target {target:?} is not a callable")
            }
            Self::NotConstant(target) => {
                write!(formatter, "imported target {target:?} is not a constant")
            }
            Self::NotProperty(target) => {
                write!(formatter, "imported target {target:?} is not a property")
            }
            Self::NotTypeAlias(target) => {
                write!(formatter, "imported target {target:?} is not a type alias")
            }
            Self::ForeignWorld => {
                formatter.write_str("imported binding belongs to another semantic world")
            }
            Self::ForeignProjection => {
                formatter.write_str("imported property belongs to another dependency projection")
            }
            Self::MissingCallableSource(declaration) => write!(
                formatter,
                "imported callable {declaration:?} has no source interface"
            ),
            Self::MissingCallable(declaration) => write!(
                formatter,
                "imported callable {declaration:?} is absent from the dependency selection catalog"
            ),
            Self::MissingConstant(property) => write!(
                formatter,
                "imported property {property:?} has no dependency constant record"
            ),
            Self::MissingProperty(property) => write!(
                formatter,
                "imported property {property:?} is absent from the dependency selection catalog"
            ),
            Self::MissingTypeAlias(alias) => write!(
                formatter,
                "imported type alias {alias:?} is absent from the dependency selection catalog"
            ),
            Self::MissingPropertySetter(property) => {
                write!(formatter, "imported property {property:?} is read-only")
            }
            Self::RestrictedPropertySetter(property) => write!(
                formatter,
                "setter of imported property {property:?} is not public"
            ),
            Self::TerminalProviderMismatch {
                declaration,
                expected,
            } => write!(
                formatter,
                "imported callable {declaration:?} does not terminate at provider {expected}"
            ),
            Self::ConstantTerminalProviderMismatch { property, expected } => write!(
                formatter,
                "imported constant {property:?} does not terminate at provider {expected}"
            ),
            Self::PropertyTerminalProviderMismatch { property, expected } => write!(
                formatter,
                "imported property {property:?} does not terminate at provider {expected}"
            ),
            Self::TypeAliasTerminalProviderMismatch { alias, expected } => write!(
                formatter,
                "imported type alias {alias:?} does not terminate at provider {expected}"
            ),
            Self::MissingDefaultCallableBinding(declaration) => write!(
                formatter,
                "dependency default callable {declaration:?} has no direct public route"
            ),
            Self::GeneratedDefaultCallable => formatter.write_str(
                "dependency default references a generated callable without a public route",
            ),
        }
    }
}

impl std::error::Error for ImportedDependencyCandidateError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionError {
    ForeignProjection,
    CapabilityUnavailable { target: ImportedTarget },
    ConstantCapabilityUnavailable { target: ImportedTarget },
    RouteMerge(DirectImportedTargetMergeError),
}

impl fmt::Display for ImportedDependencySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignProjection => {
                formatter.write_str("dependency candidate belongs to another projection")
            }
            Self::CapabilityUnavailable { target } => write!(
                formatter,
                "imported callable {target:?} is not executable by the M23-5 core-closed bridge"
            ),
            Self::ConstantCapabilityUnavailable { target } => write!(
                formatter,
                "imported constant {target:?} is not executable by the M23-5 core-closed constant bridge"
            ),
            Self::RouteMerge(error) => write!(
                formatter,
                "cannot merge selected dependency routes: {error}"
            ),
        }
    }
}

impl std::error::Error for ImportedDependencySelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RouteMerge(error) => Some(error),
            Self::ForeignProjection
            | Self::CapabilityUnavailable { .. }
            | Self::ConstantCapabilityUnavailable { .. } => None,
        }
    }
}
