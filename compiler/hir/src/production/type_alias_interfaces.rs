//! Projection of public Export HIR typealiases into their canonical interface.

use std::collections::HashSet;
use std::fmt;

use scoop_identity::{ConeIdentity, DefinitionOriginSubject, PersistentTypeAliasId};

use crate::{
    CanonicalTypeAliasInterfacesV1, DeclaredVisibility, ExportDefinitionSourceV1, ExportHir,
    ExportTypeAliasId, HirSignatureTypeMapper, HirSignatureTypeMappingError, PublicLookupAccessV1,
    TypeAliasInterfaceRecordBuildError, TypeAliasInterfaceRecordV1,
    TypeAliasInterfaceSetBuildError, TypeAliasSourceTarget, TypeAliasTargetV1,
};

impl CanonicalTypeAliasInterfacesV1 {
    /// Projects exactly the current Cone's public non-generic typealiases.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, TypeAliasInterfaceBuildError> {
        let public_aliases = export
            .public_surface
            .type_aliases
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mapper = HirSignatureTypeMapper::new(crate::HirTypeIdentityInputs::from_export(export));
        let mut records = Vec::with_capacity(export.public_surface.type_aliases.len());

        for &alias in &export.public_surface.type_aliases {
            let declaration = alias_declaration(export, alias).ok_or_else(|| {
                TypeAliasInterfaceBuildError::UnknownPublicAlias(raw_alias(alias))
            })?;
            let identity = export.type_alias_identities.get(alias).ok_or_else(|| {
                TypeAliasInterfaceBuildError::MissingAliasIdentity(raw_alias(alias))
            })?;
            let persistent = identity.id();
            if identity.key().origin() != export.cone {
                return Err(TypeAliasInterfaceBuildError::ForeignDeclaration {
                    alias: persistent,
                    expected: export.cone,
                    actual: identity.key().origin(),
                });
            }
            if declaration.access.declared != DeclaredVisibility::Public
                || !declaration.access.lookup.0.is_universal()
                || declaration.access.slot.is_some()
            {
                return Err(TypeAliasInterfaceBuildError::InvalidPublicAccess(
                    persistent,
                ));
            }

            let target = match declaration.source_target {
                TypeAliasSourceTarget::Expanded => mapper
                    .map(declaration.target, &[])
                    .map(TypeAliasTargetV1::Signature)
                    .map_err(|source| TypeAliasInterfaceBuildError::Signature {
                        alias: persistent,
                        source,
                    })?,
                TypeAliasSourceTarget::Alias(target) => {
                    if alias_declaration(export, target).is_none() {
                        return Err(TypeAliasInterfaceBuildError::UnknownAliasTarget {
                            alias: persistent,
                            target: raw_alias(target),
                        });
                    }
                    let target_identity = export.type_alias_identities.get(target).ok_or(
                        TypeAliasInterfaceBuildError::MissingTargetIdentity {
                            alias: persistent,
                            target: raw_alias(target),
                        },
                    )?;
                    if target_identity.key().origin() != export.cone {
                        return Err(TypeAliasInterfaceBuildError::ForeignAliasTarget {
                            alias: persistent,
                            target: target_identity.id(),
                            actual: target_identity.key().origin(),
                        });
                    }
                    if !public_aliases.contains(&target) {
                        return Err(TypeAliasInterfaceBuildError::NonPublicAliasTarget {
                            alias: persistent,
                            target: target_identity.id(),
                        });
                    }
                    TypeAliasTargetV1::Alias(target_identity.id())
                }
                TypeAliasSourceTarget::ImportedAlias(target) => TypeAliasTargetV1::Alias(target),
            };
            let origin = export
                .export_definition_origins
                .get(DefinitionOriginSubject::TypeAlias(persistent))
                .ok_or(TypeAliasInterfaceBuildError::MissingDefinitionOrigin(
                    persistent,
                ))?;
            let record = TypeAliasInterfaceRecordV1::try_new(
                persistent,
                target,
                PublicLookupAccessV1::DirectOnly,
                ExportDefinitionSourceV1::new(origin.origin().clone()),
            )
            .map_err(|source| TypeAliasInterfaceBuildError::Record {
                alias: persistent,
                source,
            })?;
            records.push(record);
        }

        Self::try_new(records).map_err(TypeAliasInterfaceBuildError::Table)
    }
}

fn alias_declaration(
    export: &ExportHir,
    alias: ExportTypeAliasId,
) -> Option<&crate::TypeAliasDecl> {
    ((raw_alias(alias) as usize) < export.type_aliases.len()).then(|| &export.type_aliases[alias])
}

fn raw_alias(alias: ExportTypeAliasId) -> u32 {
    alias.into_raw().into_u32()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAliasInterfaceBuildError {
    UnknownPublicAlias(u32),
    MissingAliasIdentity(u32),
    ForeignDeclaration {
        alias: PersistentTypeAliasId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidPublicAccess(PersistentTypeAliasId),
    UnknownAliasTarget {
        alias: PersistentTypeAliasId,
        target: u32,
    },
    MissingTargetIdentity {
        alias: PersistentTypeAliasId,
        target: u32,
    },
    ForeignAliasTarget {
        alias: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
        actual: ConeIdentity,
    },
    NonPublicAliasTarget {
        alias: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    Signature {
        alias: PersistentTypeAliasId,
        source: HirSignatureTypeMappingError,
    },
    MissingDefinitionOrigin(PersistentTypeAliasId),
    Record {
        alias: PersistentTypeAliasId,
        source: TypeAliasInterfaceRecordBuildError,
    },
    Table(TypeAliasInterfaceSetBuildError),
}

impl fmt::Display for TypeAliasInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPublicAlias(alias) => {
                write!(
                    formatter,
                    "public type-alias id {alias} is outside the HIR arena"
                )
            }
            Self::MissingAliasIdentity(alias) => {
                write!(
                    formatter,
                    "public type-alias id {alias} has no persistent identity"
                )
            }
            Self::ForeignDeclaration {
                alias,
                expected,
                actual,
            } => write!(
                formatter,
                "type-alias interface {alias} belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::InvalidPublicAccess(alias) => write!(
                formatter,
                "type-alias interface {alias} does not have public top-level lookup access"
            ),
            Self::UnknownAliasTarget { alias, target } => write!(
                formatter,
                "type-alias interface {alias} references local alias id {target} outside the HIR arena"
            ),
            Self::MissingTargetIdentity { alias, target } => write!(
                formatter,
                "type-alias interface {alias} references local alias id {target} without a persistent identity"
            ),
            Self::ForeignAliasTarget {
                alias,
                target,
                actual,
            } => write!(
                formatter,
                "type-alias interface {alias} targets local alias {target} whose declaration belongs to foreign Cone {actual}"
            ),
            Self::NonPublicAliasTarget { alias, target } => write!(
                formatter,
                "public type-alias interface {alias} directly targets non-public alias {target}"
            ),
            Self::Signature { alias, source } => {
                write!(
                    formatter,
                    "cannot map type-alias interface {alias}: {source}"
                )
            }
            Self::MissingDefinitionOrigin(alias) => write!(
                formatter,
                "type-alias interface {alias} has no persistent definition origin"
            ),
            Self::Record { alias, source } => {
                write!(formatter, "invalid type-alias interface {alias}: {source}")
            }
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for TypeAliasInterfaceBuildError {}
