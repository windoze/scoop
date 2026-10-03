use scoop_identity::{BindableEntity, NominalDeclarationOwner};

use super::{
    LocalNominalId, NominalArenaKind, NominalInterfaceBuildError,
    NominalNestedBindingProjectionError, NominalProjection, NominalSourceProjectionError,
};
use crate::CanonicalPersistentIdsV1;

pub(super) fn project(
    projection: &NominalProjection<'_>,
    local: LocalNominalId,
    owner: NominalDeclarationOwner,
) -> Result<
    CanonicalPersistentIdsV1<scoop_identity::PersistentExportBindingId>,
    NominalInterfaceBuildError,
> {
    let expected_owner = local.owner();
    let mut bindings = Vec::new();
    if let LocalNominalId::Object(id) = local {
        let backing = &projection.export.classes[projection.export.objects[id].backing_class];
        for function in &backing.methods {
            if projection.public_functions.contains(function)
                && let crate::HirFunctionIdentity::Source(identity) =
                    &projection.export.function_identities[*function]
            {
                let target = match identity {
                    crate::HirSourceFunctionIdentity::Plain(record) => {
                        BindableEntity::Function(record.id())
                    }
                    crate::HirSourceFunctionIdentity::Generic(record) => {
                        BindableEntity::GenericFunction(record.id())
                    }
                };
                collect_target(projection, owner, target, &mut bindings)?;
            }
        }
        for property in &backing.properties {
            if projection.public_properties.contains(property) {
                let target = match &projection.export.property_identities[*property] {
                    crate::HirPropertyIdentity::Ordinary(record) => {
                        BindableEntity::Property(record.id())
                    }
                    crate::HirPropertyIdentity::Extension(record) => {
                        BindableEntity::ExtensionProperty(record.id())
                    }
                };
                collect_target(projection, owner, target, &mut bindings)?;
            }
        }
    }

    if let LocalNominalId::Enum(id) = local
        && projection.export.public_surface.enums.contains(&id)
    {
        collect_enum_variants(projection, owner, id, &mut bindings)?;
    }
    for &id in &projection.export.public_surface.structs {
        if projection.export.structs[id].owner == Some(expected_owner) {
            collect_nominal(projection, owner, LocalNominalId::Struct(id), &mut bindings)?;
        }
    }
    for &id in &projection.export.public_surface.enums {
        if projection.export.enums[id].owner == Some(expected_owner) {
            collect_nominal(projection, owner, LocalNominalId::Enum(id), &mut bindings)?;
        }
    }
    for &id in &projection.export.public_surface.classes {
        if projection.export.classes[id].owner == Some(expected_owner) {
            collect_nominal(projection, owner, LocalNominalId::Class(id), &mut bindings)?;
        }
    }
    for &id in &projection.export.public_surface.interfaces {
        if projection.export.interfaces[id].owner == Some(expected_owner) {
            collect_nominal(
                projection,
                owner,
                LocalNominalId::Interface(id),
                &mut bindings,
            )?;
        }
    }
    for &id in &projection.export.public_surface.objects {
        if projection.export.objects[id].owner == Some(expected_owner) {
            collect_nominal(projection, owner, LocalNominalId::Object(id), &mut bindings)?;
            let object = &projection.export.objects[id];
            let value = projection
                .export
                .object_value_identities
                .get(object.singleton_value)
                .ok_or_else(|| {
                    error(
                        owner,
                        NominalNestedBindingProjectionError::MissingIdentity {
                            kind: NominalArenaKind::Object,
                            index: super::raw_index(id),
                        },
                    )
                })?;
            collect_target(
                projection,
                owner,
                BindableEntity::ObjectValue(value.id()),
                &mut bindings,
            )?;
        }
    }
    CanonicalPersistentIdsV1::try_new(bindings).map_err(|source| {
        NominalInterfaceBuildError::NestedBindings {
            declaration: owner,
            source,
        }
    })
}

fn collect_enum_variants(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    id: crate::EnumId,
    bindings: &mut Vec<scoop_identity::PersistentExportBindingId>,
) -> Result<(), NominalInterfaceBuildError> {
    let source_error = |detail| NominalInterfaceBuildError::SourceShape {
        declaration: owner,
        detail,
    };
    for index in 0..projection.export.enums[id].variants.len() {
        let index = u32::try_from(index)
            .map_err(|_| source_error(NominalSourceProjectionError::TooManyEnumVariants))?;
        let identity = crate::EnumVariantRef::checked(&projection.export.enums, id, index)
            .and_then(|reference| {
                projection
                    .export
                    .enum_member_identities
                    .get_variant(reference)
            })
            .ok_or_else(|| {
                source_error(NominalSourceProjectionError::MissingEnumVariantIdentity {
                    variant: index,
                })
            })?;
        collect_target(
            projection,
            owner,
            BindableEntity::EnumVariant(identity.id()),
            bindings,
        )?;
    }
    Ok(())
}

fn collect_nominal(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    child: LocalNominalId,
    bindings: &mut Vec<scoop_identity::PersistentExportBindingId>,
) -> Result<(), NominalInterfaceBuildError> {
    let identity = child.identity(projection.export).ok_or_else(|| {
        let (kind, index) = child.location();
        error(
            owner,
            NominalNestedBindingProjectionError::MissingIdentity { kind, index },
        )
    })?;
    let source = identity.source().ok_or_else(|| {
        let (kind, index) = child.location();
        error(
            owner,
            NominalNestedBindingProjectionError::GeneratedIdentity { kind, index },
        )
    })?;
    let target = match source {
        crate::HirSourceNominalIdentity::Concrete(record) => BindableEntity::Type(record.id()),
        crate::HirSourceNominalIdentity::Generic(record) => {
            BindableEntity::GenericType(record.id())
        }
    };
    collect_target(projection, owner, target, bindings)
}

fn collect_target(
    projection: &NominalProjection<'_>,
    owner: NominalDeclarationOwner,
    target: BindableEntity,
    bindings: &mut Vec<scoop_identity::PersistentExportBindingId>,
) -> Result<(), NominalInterfaceBuildError> {
    let mut found = false;
    for identity in projection.export.export_binding_identities.iter() {
        if identity.key().target() != target {
            continue;
        }
        let Some(record) = projection.export.public_export_bindings.get(identity.id()) else {
            return Err(error(
                owner,
                NominalNestedBindingProjectionError::NonCurrentBinding(identity.id()),
            ));
        };
        if !matches!(
            record.source(),
            crate::ExportBindingSourceV1::DeclaredCurrent { declaration } if *declaration == target
        ) {
            return Err(error(
                owner,
                NominalNestedBindingProjectionError::NonCurrentBinding(identity.id()),
            ));
        }
        found = true;
        bindings.push(identity.id());
    }
    if found {
        Ok(())
    } else {
        Err(error(
            owner,
            NominalNestedBindingProjectionError::MissingBinding(target),
        ))
    }
}

fn error(
    declaration: NominalDeclarationOwner,
    detail: NominalNestedBindingProjectionError,
) -> NominalInterfaceBuildError {
    NominalInterfaceBuildError::NestedBinding {
        declaration,
        detail,
    }
}
