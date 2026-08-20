use scoop_identity::{DeclarationScope, NominalDeclarationOwner};

use super::{
    NominalConstructorProjectionError, NominalInterfaceBuildError, NominalProjection, owner_atom,
};
use crate::{CanonicalPersistentIdsV1, HirClassConstructorIdentity};

pub(super) fn none() -> CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId> {
    CanonicalPersistentIdsV1::empty()
}

pub(super) fn from_struct(
    projection: &NominalProjection<'_>,
    id: crate::StructId,
    declaration: &crate::StructDecl,
    owner: NominalDeclarationOwner,
) -> Result<
    CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId>,
    NominalInterfaceBuildError,
> {
    let mut constructors = Vec::new();
    for &constructor_id in &declaration.constructors {
        if !projection
            .public_struct_constructors
            .contains(&constructor_id)
        {
            continue;
        }
        let constructor = super::arena_get(&projection.export.struct_constructors, constructor_id)
            .ok_or_else(|| {
                error(
                    owner,
                    NominalConstructorProjectionError::Unknown(super::raw_index(constructor_id)),
                )
            })?;
        if constructor.owner != id {
            return Err(error(
                owner,
                NominalConstructorProjectionError::OwnerMismatch {
                    actual: super::raw_index(constructor.owner),
                },
            ));
        }
        let identity = projection
            .export
            .constructor_identities
            .get_struct(constructor_id)
            .ok_or_else(|| {
                error(
                    owner,
                    NominalConstructorProjectionError::MissingIdentity(super::raw_index(
                        constructor_id,
                    )),
                )
            })?;
        validate_key(projection, owner, identity.key())?;
        constructors.push(identity.id());
    }
    canonicalize(owner, constructors)
}

pub(super) fn from_class(
    projection: &NominalProjection<'_>,
    id: crate::ClassId,
    declaration: &crate::ClassDecl,
    owner: NominalDeclarationOwner,
) -> Result<
    CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId>,
    NominalInterfaceBuildError,
> {
    let mut constructors = Vec::new();
    for &constructor_id in &declaration.constructors {
        if !projection
            .public_class_constructors
            .contains(&constructor_id)
        {
            continue;
        }
        let constructor = super::arena_get(&projection.export.class_constructors, constructor_id)
            .ok_or_else(|| {
            error(
                owner,
                NominalConstructorProjectionError::Unknown(super::raw_index(constructor_id)),
            )
        })?;
        if constructor.owner != id {
            return Err(error(
                owner,
                NominalConstructorProjectionError::OwnerMismatch {
                    actual: super::raw_index(constructor.owner),
                },
            ));
        }
        let identity = projection
            .export
            .constructor_identities
            .get_class(constructor_id)
            .ok_or_else(|| {
                error(
                    owner,
                    NominalConstructorProjectionError::MissingIdentity(super::raw_index(
                        constructor_id,
                    )),
                )
            })?;
        let identity = match identity {
            HirClassConstructorIdentity::Source(identity) => identity,
            HirClassConstructorIdentity::ZeroArgumentAdapter { source, .. } => {
                if declaration.constructors.contains(source)
                    && projection.public_class_constructors.contains(source)
                {
                    continue;
                }
                return Err(error(
                    owner,
                    NominalConstructorProjectionError::OrphanGeneratedAdapter {
                        adapter: super::raw_index(constructor_id),
                        source: super::raw_index(*source),
                    },
                ));
            }
        };
        validate_key(projection, owner, identity.key())?;
        constructors.push(identity.id());
    }
    canonicalize(owner, constructors)
}

fn validate_key(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    key: &scoop_identity::SourceDeclarationKey,
) -> Result<(), NominalInterfaceBuildError> {
    if key.origin() != projection.export.cone {
        return Err(error(
            owner,
            NominalConstructorProjectionError::ForeignDeclaration {
                expected: projection.export.cone,
                actual: key.origin(),
            },
        ));
    }
    if key.scope() != &DeclarationScope::ConeWide {
        return Err(error(
            owner,
            NominalConstructorProjectionError::InvalidDeclarationScope,
        ));
    }
    let expected = owner_atom(owner);
    let actual = key.owners().owners().last().cloned();
    if actual != Some(expected.clone()) {
        return Err(error(
            owner,
            NominalConstructorProjectionError::PersistentOwnerMismatch { expected, actual },
        ));
    }
    Ok(())
}

fn canonicalize(
    owner: NominalDeclarationOwner,
    constructors: Vec<scoop_identity::PersistentConstructorId>,
) -> Result<
    CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId>,
    NominalInterfaceBuildError,
> {
    CanonicalPersistentIdsV1::try_new(constructors).map_err(|source| {
        NominalInterfaceBuildError::Constructors {
            declaration: owner,
            source,
        }
    })
}

fn error(
    declaration: NominalDeclarationOwner,
    detail: NominalConstructorProjectionError,
) -> NominalInterfaceBuildError {
    NominalInterfaceBuildError::Constructor {
        declaration,
        detail,
    }
}
