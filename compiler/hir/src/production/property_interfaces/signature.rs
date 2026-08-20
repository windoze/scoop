use scoop_identity::{NominalDeclarationOwner, PropertyOwner as PersistentPropertyOwner};

use super::{PropertyInterfaceBuildError, PropertyNominalOwnerKind};
use crate::production::signatures::HirInterfaceSignatureProjector;
use crate::{
    CanonicalBinderListV1, ExportHir, HirPropertyIdentity, HirSignatureBinder, PropertyOwner,
    PublicDeclarationOwnerV1, TypeParamDecl,
};

pub(super) struct PropertySignatureProjection {
    pub(super) owner: PublicDeclarationOwnerV1,
    pub(super) type_parameters: CanonicalBinderListV1,
    pub(super) binders: Vec<HirSignatureBinder>,
    pub(super) receiver: Option<scoop_identity::SignatureTypeKey>,
}

pub(super) fn project_property_signature(
    export: &ExportHir,
    projector: &HirInterfaceSignatureProjector<'_>,
    property_id: crate::PropertyId,
    property: &crate::Property,
) -> Result<PropertySignatureProjection, PropertyInterfaceBuildError> {
    let persistent = persistent_property_owner(export, property_id)?;
    let (owner, own_parameters, outer_parameters, receiver) = match property.owner {
        PropertyOwner::TopLevel => (PublicDeclarationOwnerV1::TopLevel, &[][..], &[][..], None),
        PropertyOwner::Extension(extension) => {
            let Some(extension) = arena_get(&export.extension_properties, extension) else {
                return Err(PropertyInterfaceBuildError::UnknownExtensionOwner {
                    property: persistent,
                    extension: raw_index(extension),
                });
            };
            if extension.property != property_id {
                return Err(PropertyInterfaceBuildError::ExtensionOwnerMismatch {
                    property: persistent,
                    actual_property: raw_index(extension.property),
                });
            }
            (
                PublicDeclarationOwnerV1::Extension,
                extension.type_params.as_slice(),
                &[][..],
                Some(extension.receiver_ty),
            )
        }
        owner => {
            let (owner, parameters) = project_nominal_owner(export, persistent, owner)?;
            (
                PublicDeclarationOwnerV1::Nominal(owner),
                &[][..],
                parameters,
                None,
            )
        }
    };

    let mut binders = projector
        .binder_frame(own_parameters, 0)
        .map_err(|source| PropertyInterfaceBuildError::Signature {
            property: persistent,
            source,
        })?;
    binders.extend(
        projector
            .binder_frame(outer_parameters, u32::from(!own_parameters.is_empty()))
            .map_err(|source| PropertyInterfaceBuildError::Signature {
                property: persistent,
                source,
            })?,
    );
    let type_parameters = projector
        .project_binder_list(own_parameters, &binders)
        .map_err(|source| PropertyInterfaceBuildError::Signature {
            property: persistent,
            source,
        })?;
    let receiver = receiver
        .map(|receiver| projector.map_type(receiver, &binders))
        .transpose()
        .map_err(|source| PropertyInterfaceBuildError::Signature {
            property: persistent,
            source,
        })?;
    Ok(PropertySignatureProjection {
        owner,
        type_parameters,
        binders,
        receiver,
    })
}

pub(super) fn validate_declaration_identity(
    export: &ExportHir,
    property: PersistentPropertyOwner,
    identity: &HirPropertyIdentity,
) -> Result<(), PropertyInterfaceBuildError> {
    let declaration = identity.declaration();
    if declaration.origin() != export.cone {
        return Err(PropertyInterfaceBuildError::ForeignDeclaration {
            property,
            expected: export.cone,
            actual: declaration.origin(),
        });
    }
    Ok(())
}

fn project_nominal_owner<'a>(
    export: &'a ExportHir,
    property: PersistentPropertyOwner,
    owner: PropertyOwner,
) -> Result<(NominalDeclarationOwner, &'a [TypeParamDecl]), PropertyInterfaceBuildError> {
    macro_rules! project {
        ($id:expr, $arena:ident, $kind:expr, $parameters:expr) => {{
            let id = $id;
            let Some(declaration) = arena_get(&export.$arena, id) else {
                return Err(PropertyInterfaceBuildError::UnknownNominalOwner {
                    property,
                    kind: $kind,
                    owner: raw_index(id),
                });
            };
            let identity = &export.nominal_identities[id];
            let Some(source) = identity.source() else {
                return Err(PropertyInterfaceBuildError::GeneratedNominalOwner {
                    property,
                    kind: $kind,
                    owner: raw_index(id),
                });
            };
            if source.declaration().origin() != export.cone {
                return Err(PropertyInterfaceBuildError::ForeignNominalOwner {
                    property,
                    expected: export.cone,
                    actual: source.declaration().origin(),
                });
            }
            let owner = match source {
                crate::HirSourceNominalIdentity::Concrete(record) => {
                    NominalDeclarationOwner::Concrete(record.id())
                }
                crate::HirSourceNominalIdentity::Generic(record) => {
                    NominalDeclarationOwner::GenericTemplate(record.id())
                }
            };
            (owner, $parameters(declaration))
        }};
    }

    Ok(match owner {
        PropertyOwner::Class(id) => project!(
            id,
            classes,
            PropertyNominalOwnerKind::Class,
            |declaration: &'a crate::ClassDecl| declaration.type_params.as_slice()
        ),
        PropertyOwner::Struct(id) => project!(
            id,
            structs,
            PropertyNominalOwnerKind::Struct,
            |declaration: &'a crate::StructDecl| declaration.type_params.as_slice()
        ),
        PropertyOwner::Enum(id) => project!(
            id,
            enums,
            PropertyNominalOwnerKind::Enum,
            |declaration: &'a crate::EnumDecl| declaration.type_params.as_slice()
        ),
        PropertyOwner::Interface(id) => project!(
            id,
            interfaces,
            PropertyNominalOwnerKind::Interface,
            |declaration: &'a crate::InterfaceDecl| declaration.type_params.as_slice()
        ),
        PropertyOwner::Object(id) => project!(
            id,
            objects,
            PropertyNominalOwnerKind::Object,
            |declaration: &'a crate::ObjectDecl| export.classes[declaration.backing_class]
                .type_params
                .as_slice()
        ),
        PropertyOwner::TopLevel | PropertyOwner::Extension(_) => {
            unreachable!("non-nominal owners are handled before nominal projection")
        }
    })
}

fn persistent_property_owner(
    export: &ExportHir,
    property: crate::PropertyId,
) -> Result<PersistentPropertyOwner, PropertyInterfaceBuildError> {
    match export.property_identities.get(property) {
        Some(HirPropertyIdentity::Ordinary(record)) => {
            Ok(PersistentPropertyOwner::Property(record.id()))
        }
        Some(HirPropertyIdentity::Extension(record)) => {
            Ok(PersistentPropertyOwner::ExtensionProperty(record.id()))
        }
        None => Err(PropertyInterfaceBuildError::MissingPropertyIdentity(
            raw_index(property),
        )),
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
