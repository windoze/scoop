use super::*;
use crate::{NominalSourceFieldsV1, ObjectSourceShapeV1};

pub(in crate::production::nominal_interfaces) fn class_shape(
    projection: &SourceShapeProjection<'_>,
    declaration: &crate::ClassDecl,
    binders: &[HirSignatureBinder],
    owner: NominalDeclarationOwner,
) -> Result<NominalSourceShapeV1, NominalInterfaceBuildError> {
    match &declaration.representation {
        crate::ClassRepresentation::Declared => {
            declared_fields(projection, declaration, binders, owner)
                .map(NominalSourceShapeV1::Class)
        }
        crate::ClassRepresentation::Intrinsic(intrinsic) => Ok(NominalSourceShapeV1::Intrinsic(
            crate::NominalIntrinsicRepresentationV1::new(*intrinsic),
        )),
    }
}

fn declared_fields(
    projection: &SourceShapeProjection<'_>,
    declaration: &crate::ClassDecl,
    binders: &[HirSignatureBinder],
    owner: NominalDeclarationOwner,
) -> Result<NominalSourceFieldsV1, NominalInterfaceBuildError> {
    let fields = declaration
        .fields
        .iter()
        .map(|field| {
            let identity = &projection.export.field_identities[*field];
            let value_type = projection
                .signatures
                .map_type(projection.export.class_field_definition(*field).ty, binders)
                .map_err(|source| NominalInterfaceBuildError::Signature {
                    declaration: owner,
                    source,
                })?;
            Ok(NominalSourceFieldV1::new(identity.id(), value_type))
        })
        .collect::<Result<Vec<_>, NominalInterfaceBuildError>>()?;
    NominalSourceFieldsV1::try_new(fields)
        .map_err(|detail| source_error(owner, NominalSourceProjectionError::Shape(detail)))
}

pub(in crate::production::nominal_interfaces) fn object_shape(
    projection: &SourceShapeProjection<'_>,
    id: crate::ObjectId,
    declaration: &crate::ObjectDecl,
    owner: NominalDeclarationOwner,
) -> Result<NominalSourceShapeV1, NominalInterfaceBuildError> {
    let value = projection
        .export
        .object_value_identities
        .get(declaration.singleton_value)
        .ok_or_else(|| {
            source_error(
                owner,
                NominalSourceProjectionError::MissingObjectValueIdentity(
                    crate::production::nominal_interfaces::raw_index(declaration.singleton_value),
                ),
            )
        })?;
    if value.declaration() != id {
        return Err(source_error(
            owner,
            NominalSourceProjectionError::ObjectValueOwner {
                expected: crate::production::nominal_interfaces::raw_index(id),
                actual: crate::production::nominal_interfaces::raw_index(value.declaration()),
            },
        ));
    }
    let fields = declared_fields(
        projection,
        &projection.export.classes[declaration.backing_class],
        &[],
        owner,
    )?;
    Ok(NominalSourceShapeV1::Object(ObjectSourceShapeV1::new(
        match declaration.kind {
            crate::ObjectKind::Standalone => crate::ObjectSourceKindV1::Standalone,
            crate::ObjectKind::Companion(_) => crate::ObjectSourceKindV1::Companion,
        },
        value.id(),
        fields,
    )))
}
