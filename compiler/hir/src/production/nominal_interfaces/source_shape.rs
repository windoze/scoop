use scoop_identity::NominalDeclarationOwner;

use super::{NominalInterfaceBuildError, NominalSourceProjectionError};
use crate::{
    CanonicalSignatureTypesV1, EnumSourceFieldV1, EnumSourceShapeV1, EnumSourceVariantStyleV1,
    EnumSourceVariantV1, HirSignatureBinder, NominalSourceFieldV1, NominalSourceShapeV1,
    StructSourceShapeV1,
};

mod reference;
pub(super) use reference::{class_shape, object_shape};

/// Shared source-only projection; it has no public lookup inventory.
pub(super) struct SourceShapeProjection<'a> {
    export: &'a crate::ExportHir,
    signatures: super::HirInterfaceSignatureProjector<'a>,
}
impl<'a> SourceShapeProjection<'a> {
    pub(super) fn new(export: &'a crate::ExportHir) -> Self {
        Self {
            export,
            signatures: super::HirInterfaceSignatureProjector::new(export),
        }
    }
}

pub(super) fn class_supertypes(
    projection: &SourceShapeProjection<'_>,
    owner: NominalDeclarationOwner,
    declaration: &crate::ClassDecl,
    binders: &[HirSignatureBinder],
) -> Result<CanonicalSignatureTypesV1, NominalInterfaceBuildError> {
    canonical_supertypes(
        projection,
        owner,
        declaration
            .base_class
            .iter()
            .chain(declaration.interfaces.iter())
            .copied(),
        binders,
    )
}

pub(super) fn interface_supertypes(
    projection: &SourceShapeProjection<'_>,
    owner: NominalDeclarationOwner,
    declaration: &crate::InterfaceDecl,
    binders: &[HirSignatureBinder],
) -> Result<CanonicalSignatureTypesV1, NominalInterfaceBuildError> {
    let mut types = Vec::with_capacity(declaration.parents.len());
    for &parent in &declaration.parents {
        let application = super::arena_get(&projection.export.interface_applications, parent)
            .ok_or_else(|| NominalInterfaceBuildError::Signature {
                declaration: owner,
                source: crate::HirInterfaceSignatureProjectionError::UnknownInterfaceApplication(
                    super::raw_index(parent),
                ),
            })?;
        types.push(application.canonical_type);
    }
    canonical_supertypes(projection, owner, types, binders)
}

pub(super) fn direct_supertypes(
    projection: &SourceShapeProjection<'_>,
    owner: NominalDeclarationOwner,
    types: &[crate::TypeId],
    binders: &[HirSignatureBinder],
) -> Result<CanonicalSignatureTypesV1, NominalInterfaceBuildError> {
    canonical_supertypes(projection, owner, types.iter().copied(), binders)
}

pub(super) fn object_supertypes(
    projection: &SourceShapeProjection<'_>,
    owner: NominalDeclarationOwner,
    backing: &crate::ClassDecl,
    binders: &[HirSignatureBinder],
) -> Result<CanonicalSignatureTypesV1, NominalInterfaceBuildError> {
    canonical_supertypes(
        projection,
        owner,
        backing
            .base_class
            .iter()
            .chain(backing.interfaces.iter())
            .copied(),
        binders,
    )
}

fn canonical_supertypes(
    projection: &SourceShapeProjection<'_>,
    declaration: NominalDeclarationOwner,
    types: impl IntoIterator<Item = crate::TypeId>,
    binders: &[HirSignatureBinder],
) -> Result<CanonicalSignatureTypesV1, NominalInterfaceBuildError> {
    let types = types
        .into_iter()
        .map(|ty| {
            projection
                .signatures
                .map_type(ty, binders)
                .map_err(|source| NominalInterfaceBuildError::Signature {
                    declaration,
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    CanonicalSignatureTypesV1::try_new(types).map_err(|source| {
        NominalInterfaceBuildError::Supertypes {
            declaration,
            source,
        }
    })
}

pub(super) fn struct_shape(
    projection: &SourceShapeProjection<'_>,
    id: crate::StructId,
    declaration: &crate::StructDecl,
    binders: &[HirSignatureBinder],
    owner: NominalDeclarationOwner,
) -> Result<NominalSourceShapeV1, NominalInterfaceBuildError> {
    if let crate::StructRepresentation::Intrinsic(intrinsic) = &declaration.representation {
        return Ok(NominalSourceShapeV1::Intrinsic(
            crate::NominalIntrinsicRepresentationV1::new(*intrinsic),
        ));
    }
    let mut fields = Vec::with_capacity(declaration.semantic_fields().len());
    for (index, field) in declaration.semantic_fields().iter().enumerate() {
        let local_index = u32::try_from(index)
            .map_err(|_| source_error(owner, NominalSourceProjectionError::TooManyStructFields))?;
        let reference = crate::StructFieldRef::checked(&projection.export.structs, id, local_index)
            .ok_or_else(|| {
                source_error(
                    owner,
                    NominalSourceProjectionError::MissingStructFieldIdentity { field: local_index },
                )
            })?;
        let identity = projection
            .export
            .field_identities
            .get_struct(reference)
            .ok_or_else(|| {
                source_error(
                    owner,
                    NominalSourceProjectionError::MissingStructFieldIdentity { field: local_index },
                )
            })?;
        let value_type = projection
            .signatures
            .map_type(field.ty, binders)
            .map_err(|source| NominalInterfaceBuildError::Signature {
                declaration: owner,
                source,
            })?;
        fields.push(NominalSourceFieldV1::new(identity.id(), value_type));
    }
    StructSourceShapeV1::try_new(
        fields,
        crate::NominalCLayoutPolicyV1::from_source_contract(declaration.attributes.c_layout),
        declaration.attributes.interior_mutable,
    )
    .map(NominalSourceShapeV1::Struct)
    .map_err(|detail| source_error(owner, NominalSourceProjectionError::Shape(detail)))
}

pub(super) fn enum_shape(
    projection: &SourceShapeProjection<'_>,
    id: crate::EnumId,
    declaration: &crate::EnumDecl,
    binders: &[HirSignatureBinder],
    owner: NominalDeclarationOwner,
) -> Result<NominalSourceShapeV1, NominalInterfaceBuildError> {
    let mut variants = Vec::with_capacity(declaration.variants.len());
    for (variant_index, variant) in declaration.variants.iter().enumerate() {
        let variant_index = u32::try_from(variant_index)
            .map_err(|_| source_error(owner, NominalSourceProjectionError::TooManyEnumVariants))?;
        let reference = crate::EnumVariantRef::checked(&projection.export.enums, id, variant_index)
            .ok_or_else(|| {
                source_error(
                    owner,
                    NominalSourceProjectionError::MissingEnumVariantIdentity {
                        variant: variant_index,
                    },
                )
            })?;
        let identity = projection
            .export
            .enum_member_identities
            .get_variant(reference)
            .ok_or_else(|| {
                source_error(
                    owner,
                    NominalSourceProjectionError::MissingEnumVariantIdentity {
                        variant: variant_index,
                    },
                )
            })?;
        let mut fields = Vec::with_capacity(variant.fields.len());
        for (field_index, field) in variant.fields.iter().enumerate() {
            let field_index = u32::try_from(field_index).map_err(|_| {
                source_error(
                    owner,
                    NominalSourceProjectionError::TooManyEnumFields {
                        variant: variant_index,
                    },
                )
            })?;
            let field_ref = crate::EnumVariantFieldRef::checked(
                &projection.export.enums,
                reference,
                field_index,
            )
            .ok_or_else(|| {
                source_error(
                    owner,
                    NominalSourceProjectionError::MissingEnumFieldIdentity {
                        variant: variant_index,
                        field: field_index,
                    },
                )
            })?;
            let field_identity = projection
                .export
                .enum_member_identities
                .get_field(field_ref)
                .ok_or_else(|| {
                    source_error(
                        owner,
                        NominalSourceProjectionError::MissingEnumFieldIdentity {
                            variant: variant_index,
                            field: field_index,
                        },
                    )
                })?;
            let value_type =
                projection
                    .signatures
                    .map_type(field.ty, binders)
                    .map_err(|source| NominalInterfaceBuildError::Signature {
                        declaration: owner,
                        source,
                    })?;
            fields.push(EnumSourceFieldV1::new(field_identity.id(), value_type));
        }
        let style = match variant.style {
            crate::VariantStyle::Unit => EnumSourceVariantStyleV1::Unit,
            crate::VariantStyle::Positional => EnumSourceVariantStyleV1::Positional,
            crate::VariantStyle::Named => EnumSourceVariantStyleV1::Named,
            crate::VariantStyle::Constructor => EnumSourceVariantStyleV1::Constructor,
        };
        let variant =
            EnumSourceVariantV1::try_new(identity.id(), style, fields).map_err(|detail| {
                source_error(owner, NominalSourceProjectionError::EnumVariant(detail))
            })?;
        variants.push(variant);
    }
    EnumSourceShapeV1::try_new(variants)
        .map(NominalSourceShapeV1::Enum)
        .map_err(|detail| source_error(owner, NominalSourceProjectionError::Shape(detail)))
}

fn source_error(
    declaration: NominalDeclarationOwner,
    detail: NominalSourceProjectionError,
) -> NominalInterfaceBuildError {
    NominalInterfaceBuildError::SourceShape {
        declaration,
        detail,
    }
}
