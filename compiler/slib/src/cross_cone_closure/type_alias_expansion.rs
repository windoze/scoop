//! References used by current exported type aliases.
use scoop_hir::{CrossConeHirInterfaceSectionV1, ExternalHirTargetV1, TypeAliasTargetV1};
use scoop_identity::{
    ConeIdentity, PersistentTypeAliasId, SourceDeclarationKey, ValidatedIdentityGraph,
};
use std::fmt;
pub(crate) fn validate_alias_targets(
    current: ConeIdentity,
    interface: &CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
) -> Result<(), CrossConeHirAliasReferenceError> {
    use CrossConeHirAliasReferenceError as Error;
    for record in interface.type_aliases().records() {
        let TypeAliasTargetV1::Alias(target) = record.target() else {
            continue;
        };
        let source = record.alias();
        let target = *target;
        let declaration = identities
            .canonical_key::<_, SourceDeclarationKey>(target)
            .map_err(|_| Error::MissingDeclaration { source, target })?;
        if declaration.origin() == current {
            if interface.type_aliases().get(target).is_none() {
                return Err(Error::MissingCurrentPublicTarget { source, target });
            }
        } else if interface
            .external_references()
            .get(ExternalHirTargetV1::TypeAlias(target))
            .is_none()
        {
            return Err(Error::MissingForeignReference { source, target });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum CrossConeHirAliasReferenceError {
    MissingDeclaration {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    MissingCurrentPublicTarget {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
    MissingForeignReference {
        source: PersistentTypeAliasId,
        target: PersistentTypeAliasId,
    },
}
impl fmt::Display for CrossConeHirAliasReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDeclaration { source, target } => write!(
                f,
                "type alias {source} has no declaration for target {target}"
            ),
            Self::MissingCurrentPublicTarget { source, target } => write!(
                f,
                "type alias {source} targets current non-public alias {target}"
            ),
            Self::MissingForeignReference { source, target } => write!(
                f,
                "type alias {source} targets foreign alias {target} without an external reference"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirAliasReferenceError {}
