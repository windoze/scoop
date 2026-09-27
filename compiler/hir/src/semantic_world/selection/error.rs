use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, PersistentSourceContextId,
    PropertyOwner, SourceIdentity,
};

use super::super::{DirectImportedTargetMergeError, ImportedTarget};
use crate::NominalCallableClassificationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionPlanBuildError {
    MissingDispatchSlot(scoop_identity::PersistentDispatchSlotId),
    MissingNominal(scoop_identity::PersistentTypeId),
    MissingNominalField(scoop_identity::PersistentFieldId),
    MissingEnumVariant(scoop_identity::PersistentEnumVariantId),
    MissingEnumField(scoop_identity::PersistentEnumVariantFieldId),
    DuplicateNominal(scoop_identity::PersistentTypeId),
    NominalClassifier(crate::NominalExactLeafClassifierBuildError),
    Classification(NominalCallableClassificationError),
    Initialization(crate::HirInitializationUseError),
    DuplicateCallable(CallableTemplateOrigin),
    MissingCallableSourceName(CallableTemplateOrigin),
    DuplicateConstant(PersistentPropertyId),
    DuplicateProperty(PropertyOwner),
    MissingPropertySourceName(PropertyOwner),
    DuplicateTypeAlias(scoop_identity::PersistentTypeAliasId),
    MissingTypeAliasExpansion(scoop_identity::PersistentTypeAliasId),
    MissingDefinitionSource {
        provider: ConeIdentity,
        source: SourceIdentity,
    },
    MissingDefinitionContext {
        provider: ConeIdentity,
        context: PersistentSourceContextId,
    },
}

impl fmt::Display for ImportedDependencySelectionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDispatchSlot(id) => {
                write!(formatter, "dependency dispatch slot {id} is missing")
            }
            Self::MissingNominal(id) => write!(
                formatter,
                "dependency nominal {id} has no declaration identity"
            ),
            Self::MissingNominalField(id) => write!(
                formatter,
                "dependency field {id} has no declaration identity"
            ),
            Self::MissingEnumVariant(id) => write!(
                formatter,
                "dependency enum variant {id} has no source declaration"
            ),
            Self::MissingEnumField(id) => write!(
                formatter,
                "dependency enum payload field {id} has no declaration identity"
            ),
            Self::DuplicateNominal(id) => write!(
                formatter,
                "dependency nominal {id} is defined more than once"
            ),
            Self::NominalClassifier(error) => error.fmt(formatter),
            Self::Classification(error) => error.fmt(formatter),
            Self::Initialization(error) => error.fmt(formatter),
            Self::DuplicateCallable(declaration) => write!(
                formatter,
                "dependency semantic world contains duplicate callable {declaration:?}"
            ),
            Self::MissingCallableSourceName(declaration) => write!(
                formatter,
                "dependency callable {declaration:?} has no canonical function source name"
            ),
            Self::DuplicateConstant(property) => write!(
                formatter,
                "dependency semantic world contains duplicate constant {property:?}"
            ),
            Self::DuplicateProperty(property) => write!(
                formatter,
                "dependency semantic world contains duplicate property {property:?}"
            ),
            Self::MissingPropertySourceName(property) => write!(
                formatter,
                "dependency property {property:?} has no declaration name"
            ),
            Self::DuplicateTypeAlias(alias) => write!(
                formatter,
                "dependency semantic world contains duplicate type alias {alias:?}"
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
        }
    }
}

impl std::error::Error for ImportedDependencySelectionPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NominalClassifier(error) => Some(error),
            Self::Classification(error) => Some(error),
            Self::Initialization(error) => Some(error),
            Self::MissingDispatchSlot(_)
            | Self::MissingNominal(_)
            | Self::MissingNominalField(_)
            | Self::MissingEnumVariant(_)
            | Self::MissingEnumField(_)
            | Self::DuplicateNominal(_)
            | Self::DuplicateCallable(_)
            | Self::MissingCallableSourceName(_)
            | Self::MissingPropertySourceName(_)
            | Self::DuplicateConstant(_)
            | Self::DuplicateProperty(_)
            | Self::DuplicateTypeAlias(_)
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
    MissingCallable(CallableTemplateOrigin),
    MissingCallableSource(CallableTemplateOrigin),
    MissingConstant(PersistentPropertyId),
    MissingProperty(PropertyOwner),
    MissingTypeAlias(scoop_identity::PersistentTypeAliasId),
    MissingPropertySetter(PropertyOwner),
    RestrictedPropertySetter(PropertyOwner),
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
        }
    }
}

impl std::error::Error for ImportedDependencyCandidateError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionError {
    InvalidDispatch { declaration: CallableTemplateOrigin },
    MissingCallable(CallableTemplateOrigin),
    MissingConstant(PersistentPropertyId),
    MissingTypeAlias(scoop_identity::PersistentTypeAliasId),
    CapabilityUnavailable { declaration: CallableTemplateOrigin },
    ConstantCapabilityUnavailable { target: ImportedTarget },
    RouteMerge(DirectImportedTargetMergeError),
}

impl fmt::Display for ImportedDependencySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDispatch { declaration } => write!(
                formatter,
                "imported callable {declaration:?} has no matching dispatch slot"
            ),
            Self::MissingCallable(id) => write!(
                formatter,
                "selected callable {id:?} is absent from the dependency catalog"
            ),
            Self::MissingConstant(id) => write!(
                formatter,
                "selected constant {id} is absent from the dependency catalog"
            ),
            Self::MissingTypeAlias(id) => write!(
                formatter,
                "selected type alias {id} is absent from the dependency catalog"
            ),
            Self::CapabilityUnavailable { declaration } => write!(
                formatter,
                "imported callable {declaration:?} has no executable param-free nominal bridge"
            ),
            Self::ConstantCapabilityUnavailable { target } => write!(
                formatter,
                "imported constant {target:?} has no executable nominal constant bridge"
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
            Self::InvalidDispatch { .. }
            | Self::MissingCallable(_)
            | Self::MissingConstant(_)
            | Self::MissingTypeAlias(_)
            | Self::CapabilityUnavailable { .. }
            | Self::ConstantCapabilityUnavailable { .. } => None,
        }
    }
}
