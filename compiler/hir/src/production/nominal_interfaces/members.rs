use scoop_identity::{
    CallableTemplateOrigin, DeclarationScope, NominalDeclarationOwner,
    PropertyOwner as PersistentPropertyOwner,
};

use super::{
    NominalInterfaceBuildError, NominalMemberProjectionError, NominalProjection, owner_atom,
    owner_resolution,
};
use crate::{
    CanonicalPublicMemberRefsV1, HirFunctionIdentity, HirPropertyIdentity, InterfaceMemberRole,
    PublicMemberRefV1,
};

pub(super) fn ordinary(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    methods: &[crate::FunctionId],
    properties: &[crate::PropertyId],
) -> Result<CanonicalPublicMemberRefsV1, NominalInterfaceBuildError> {
    let mut members = Vec::new();
    for &function in methods {
        project_function(projection, owner, function, &mut members)?;
    }
    for &property in properties {
        project_property(projection, owner, property, &mut members)?;
    }
    canonicalize(owner, members)
}

pub(super) fn interface(
    projection: &NominalProjection<'_>,
    interface: crate::InterfaceId,
    declaration: &crate::InterfaceDecl,
    owner: NominalDeclarationOwner,
) -> Result<CanonicalPublicMemberRefsV1, NominalInterfaceBuildError> {
    let mut members = Vec::new();
    for &member_id in &declaration.methods {
        let member =
            super::arena_get(&projection.export.interface_methods, member_id).ok_or_else(|| {
                error(
                    owner,
                    NominalMemberProjectionError::UnknownInterfaceMethod(super::raw_index(
                        member_id,
                    )),
                )
            })?;
        if member.owner != interface {
            return Err(error(
                owner,
                NominalMemberProjectionError::InterfaceMethodOwner {
                    expected: super::raw_index(interface),
                    actual: super::raw_index(member.owner),
                },
            ));
        }
        match member.role {
            InterfaceMemberRole::Function => {
                project_function(projection, owner, member.function, &mut members)?;
            }
            InterfaceMemberRole::PropertyGetter(_) | InterfaceMemberRole::PropertySetter(_) => {}
        }
    }
    for &property in &declaration.properties {
        project_property(projection, owner, property, &mut members)?;
    }
    canonicalize(owner, members)
}

fn project_function(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    function_id: crate::FunctionId,
    members: &mut Vec<PublicMemberRefV1>,
) -> Result<(), NominalInterfaceBuildError> {
    if !projection.public_functions.contains(&function_id) {
        return Ok(());
    }
    let function =
        super::arena_get(&projection.export.functions, function_id).ok_or_else(|| {
            error(
                owner,
                NominalMemberProjectionError::UnknownFunction(super::raw_index(function_id)),
            )
        })?;
    let method = function.method.ok_or_else(|| {
        error(
            owner,
            NominalMemberProjectionError::MissingMethodOwner(super::raw_index(function_id)),
        )
    })?;
    let hir_owner = owner_resolution::from_type(projection.export, method.owner);
    if hir_owner != Some(owner) {
        return Err(error(
            owner,
            NominalMemberProjectionError::HirMethodOwnerMismatch {
                function: super::raw_index(function_id),
                actual: hir_owner,
            },
        ));
    }
    let identity = projection
        .export
        .function_identities
        .get(function_id)
        .ok_or_else(|| {
            error(
                owner,
                NominalMemberProjectionError::MissingFunctionIdentity(super::raw_index(
                    function_id,
                )),
            )
        })?;
    let HirFunctionIdentity::Source(source) = identity else {
        return Err(error(
            owner,
            NominalMemberProjectionError::NonSourceFunction(super::raw_index(function_id)),
        ));
    };
    validate_key(projection, owner, source.declaration())?;
    let declaration = match source {
        crate::HirSourceFunctionIdentity::Plain(record) => {
            CallableTemplateOrigin::Function(record.id())
        }
        crate::HirSourceFunctionIdentity::Generic(record) => {
            CallableTemplateOrigin::GenericFunction(record.id())
        }
    };
    members.push(PublicMemberRefV1::Callable(declaration));
    Ok(())
}

fn project_property(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    property_id: crate::PropertyId,
    members: &mut Vec<PublicMemberRefV1>,
) -> Result<(), NominalInterfaceBuildError> {
    if !projection.public_properties.contains(&property_id) {
        return Ok(());
    }
    let property =
        super::arena_get(&projection.export.properties, property_id).ok_or_else(|| {
            error(
                owner,
                NominalMemberProjectionError::UnknownProperty(super::raw_index(property_id)),
            )
        })?;
    let hir_owner = owner_resolution::from_property(projection.export, property.owner);
    if hir_owner != Some(owner) {
        return Err(error(
            owner,
            NominalMemberProjectionError::HirPropertyOwnerMismatch {
                property: super::raw_index(property_id),
                actual: hir_owner,
            },
        ));
    }
    let identity = projection
        .export
        .property_identities
        .get(property_id)
        .ok_or_else(|| {
            error(
                owner,
                NominalMemberProjectionError::MissingPropertyIdentity(super::raw_index(
                    property_id,
                )),
            )
        })?;
    let HirPropertyIdentity::Ordinary(identity) = identity else {
        return Err(error(
            owner,
            NominalMemberProjectionError::ExtensionProperty(super::raw_index(property_id)),
        ));
    };
    validate_key(projection, owner, identity.key())?;
    members.push(PublicMemberRefV1::Property(
        PersistentPropertyOwner::Property(identity.id()),
    ));
    Ok(())
}

fn validate_key(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    key: &scoop_identity::SourceDeclarationKey,
) -> Result<(), NominalInterfaceBuildError> {
    if key.origin() != projection.export.cone {
        return Err(error(
            owner,
            NominalMemberProjectionError::ForeignDeclaration {
                expected: projection.export.cone,
                actual: key.origin(),
            },
        ));
    }
    if key.scope() != &DeclarationScope::ConeWide {
        return Err(error(
            owner,
            NominalMemberProjectionError::InvalidDeclarationScope,
        ));
    }
    let expected = owner_atom(owner);
    let actual = key.owners().owners().last().cloned();
    if actual != Some(expected.clone()) {
        return Err(error(
            owner,
            NominalMemberProjectionError::PersistentOwnerMismatch { expected, actual },
        ));
    }
    Ok(())
}

fn canonicalize(
    owner: NominalDeclarationOwner,
    members: Vec<PublicMemberRefV1>,
) -> Result<CanonicalPublicMemberRefsV1, NominalInterfaceBuildError> {
    CanonicalPublicMemberRefsV1::try_new(members).map_err(|source| {
        NominalInterfaceBuildError::Members {
            declaration: owner,
            source,
        }
    })
}

fn error(
    declaration: NominalDeclarationOwner,
    detail: NominalMemberProjectionError,
) -> NominalInterfaceBuildError {
    NominalInterfaceBuildError::Member {
        declaration,
        detail,
    }
}
