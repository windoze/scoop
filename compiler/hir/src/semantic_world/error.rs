use std::fmt;

use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, ConeIdentity, PersistentEnumVariantId,
    PersistentObjectValueId, PersistentTypeAliasId, PropertyOwner,
};

use crate::SourceNominalId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedSemanticEntityId {
    Nominal(SourceNominalId),
    Callable(CallableTemplateOrigin),
    Property(PropertyOwner),
    ObjectValue(PersistentObjectValueId),
    TypeAlias(PersistentTypeAliasId),
    EnumVariant(PersistentEnumVariantId),
}

#[derive(Debug)]
pub enum ImportedSemanticWorldBuildError {
    CurrentUsedAsProvider(ConeIdentity),
    DuplicateProvider(ConeIdentity),
    MissingEntityIdentity {
        provider: ConeIdentity,
        entity: ImportedSemanticEntityId,
    },
    DuplicateEntityAuthority {
        entity: ImportedSemanticEntityId,
        first: ConeIdentity,
        second: ConeIdentity,
    },
    MissingAliasExpansion {
        provider: ConeIdentity,
        alias: PersistentTypeAliasId,
    },
    MissingBindingIdentity {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
    },
    MissingBindingKey {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
    },
    BindingExporterMismatch {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
        actual: ConeIdentity,
    },
    MissingBindingTarget {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
        target: BindableEntity,
    },
    MissingNestedBinding {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
    },
    InvalidLookupRoute {
        provider: ConeIdentity,
        binding: scoop_identity::PersistentExportBindingId,
        error: Box<crate::ReexportRouteBuildError>,
    },
}

impl fmt::Display for ImportedSemanticWorldBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentUsedAsProvider(identity) => {
                write!(
                    formatter,
                    "current Cone {identity} also appears as an imported provider"
                )
            }
            Self::DuplicateProvider(identity) => {
                write!(
                    formatter,
                    "provider {identity} occurs more than once in the world"
                )
            }
            Self::MissingEntityIdentity { provider, entity } => write!(
                formatter,
                "provider {provider} has an interface record without imported identity {entity:?}"
            ),
            Self::DuplicateEntityAuthority {
                entity,
                first,
                second,
            } => write!(
                formatter,
                "semantic entity {entity:?} is owned by both Cone {first} and Cone {second}"
            ),
            Self::MissingAliasExpansion { provider, alias } => write!(
                formatter,
                "provider {provider} has no validated expansion for type alias {alias}"
            ),
            Self::MissingBindingIdentity { provider, binding } => write!(
                formatter,
                "provider {provider} has public binding {binding} without an imported identity"
            ),
            Self::MissingBindingKey { provider, binding } => write!(
                formatter,
                "provider {provider} has public binding {binding} without a canonical key"
            ),
            Self::BindingExporterMismatch {
                provider,
                binding,
                actual,
            } => write!(
                formatter,
                "provider {provider} public binding {binding} names exporter {actual}"
            ),
            Self::MissingBindingTarget {
                provider,
                binding,
                target,
            } => write!(
                formatter,
                "provider {provider} public binding {binding} targets missing entity {target:?}"
            ),
            Self::MissingNestedBinding { provider, binding } => write!(
                formatter,
                "provider {provider} nominal namespace names absent public binding {binding}"
            ),
            Self::InvalidLookupRoute {
                provider,
                binding,
                error,
            } => write!(
                formatter,
                "provider {provider} public binding {binding} has an invalid consumer import route: {error}"
            ),
        }
    }
}

impl std::error::Error for ImportedSemanticWorldBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectNamespaceLookupError {
    NoVisiblePackage,
    ExpectedBindingAfterPackage,
    MissingBinding { segment: usize, name: String },
    NotStaticOwner { segment: usize, name: String },
    AmbiguousStaticOwner { segment: usize, name: String },
    MissingNominalInterface(SourceNominalId),
}

impl fmt::Display for DirectNamespaceLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVisiblePackage => {
                formatter.write_str("selector has no visible direct package prefix")
            }
            Self::ExpectedBindingAfterPackage => {
                formatter.write_str("exact selector ends at a package namespace")
            }
            Self::MissingBinding { segment, name } => {
                write!(
                    formatter,
                    "no binding named {name} at selector segment {segment}"
                )
            }
            Self::NotStaticOwner { segment, name } => write!(
                formatter,
                "binding {name} at selector segment {segment} is not a static owner"
            ),
            Self::AmbiguousStaticOwner { segment, name } => write!(
                formatter,
                "binding {name} at selector segment {segment} has multiple static owners"
            ),
            Self::MissingNominalInterface(owner) => {
                write!(
                    formatter,
                    "static owner {owner:?} has no imported nominal interface"
                )
            }
        }
    }
}

impl std::error::Error for DirectNamespaceLookupError {}
