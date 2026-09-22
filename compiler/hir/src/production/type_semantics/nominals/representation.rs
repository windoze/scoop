use std::collections::BTreeSet;

use scoop_identity::{OptionalSignatureType, PersistentExactTypeId, PersistentTypeId};

use super::{ConcreteNominal, NominalLocalId};
use crate::*;

use super::super::CrossConeTypeSemanticsProductionError as Error;

/// Exact types whose representation or exported constructor ABI must be
/// available to M23-6. Facts projection applies the generic/local-support
/// gates recursively to this independently collected HIR inventory.
pub(super) fn fact_requirements(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
) -> Result<BTreeSet<PersistentExactTypeId>, Error> {
    let mut types = Vec::new();
    for nominal in nominals {
        match nominal.local {
            NominalLocalId::Struct(id) => {
                types.extend(
                    export.structs[id]
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty),
                );
                types.extend(export.structs[id].interfaces.iter().copied());
                for constructor in &export.structs[id].constructors {
                    let constructor = &export.struct_constructors[*constructor];
                    if is_exported_constructor(constructor.access.declared) {
                        types.extend(constructor.parameters.iter().map(|parameter| parameter.ty));
                    }
                }
            }
            NominalLocalId::Enum(id) => {
                types.extend(
                    export.enums[id]
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| field.ty)),
                );
                types.extend(export.enums[id].interfaces.iter().copied());
            }
            NominalLocalId::Class(id) => {
                let declaration = &export.classes[id];
                types.extend(
                    declaration
                        .fields
                        .iter()
                        .map(|field| export.class_fields[*field].ty),
                );
                types.extend(declaration.base_class);
                types.extend(declaration.interfaces.iter().copied());
                for constructor in &declaration.constructors {
                    let constructor = &export.class_constructors[*constructor];
                    if is_exported_constructor(constructor.access.declared) {
                        types.extend(constructor.parameters.iter().map(|parameter| parameter.ty));
                    }
                }
            }
            NominalLocalId::Interface(id) => types.extend(
                export.interfaces[id]
                    .parents
                    .iter()
                    .map(|parent| export.interface_applications[*parent].canonical_type),
            ),
            NominalLocalId::Object(id) => {
                let class = &export.classes[export.objects[id].backing_class];
                types.extend(
                    class
                        .fields
                        .iter()
                        .map(|field| export.class_fields[*field].ty),
                );
                types.extend(class.base_class);
                types.extend(class.interfaces.iter().copied());
            }
        }
    }
    types
        .into_iter()
        .map(|ty| {
            export
                .type_identities
                .get(ty)
                .and_then(HirTypeIdentity::exact)
                .map(|record| record.id())
                .ok_or(Error::MissingExactIdentity)
        })
        .collect()
}

fn is_exported_constructor(visibility: DeclaredVisibility) -> bool {
    matches!(
        visibility,
        DeclaredVisibility::Public | DeclaredVisibility::Protected
    )
}

pub(super) fn shape(
    export: &ExportHir,
    local: &LocalConcreteHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<NominalRepresentationShapeV1, Error> {
    let mapper = HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(export));
    let map = |ty| {
        mapper
            .map(ty, &[])
            .map_err(|error| Error::InvalidRepresentation {
                declaration: SourceNominalId::Concrete(nominal.owner),
                reason: error.to_string(),
            })
    };
    match nominal.local {
        NominalLocalId::Struct(id) => match &export.structs[id].representation {
            StructRepresentation::Declared(fields) => {
                let mut projected = Vec::with_capacity(fields.len());
                for (index, field) in fields.iter().enumerate() {
                    let reference = StructFieldRef::checked(&export.structs, id, index as u32)
                        .ok_or_else(|| Error::InvalidRepresentation {
                            declaration: SourceNominalId::Concrete(nominal.owner),
                            reason: format!("missing struct field {index}"),
                        })?;
                    projected.push(
                        StructRepresentationFieldV1::try_new(
                            export.field_identities[reference].key(),
                            map(field.ty)?,
                        )
                        .map_err(|error| Error::InvalidRepresentation {
                            declaration: SourceNominalId::Concrete(nominal.owner),
                            reason: error.to_string(),
                        })?,
                    );
                }
                let policy = NominalCLayoutPolicyV1::from_source_contract(
                    export.structs[id].attributes.c_layout,
                );
                Ok(NominalRepresentationShapeV1::Struct {
                    fields: projected,
                    c_layout_policy: policy,
                })
            }
            StructRepresentation::Intrinsic(declaration) => {
                Ok(NominalRepresentationShapeV1::Intrinsic {
                    representation: NominalIntrinsicRepresentationV1::new(declaration.kind),
                })
            }
        },
        NominalLocalId::Enum(id) => enumeration(export, local, nominal, id, &map),
        NominalLocalId::Class(id) => {
            let class = &export.classes[id];
            match class.representation {
                ClassRepresentation::Declared => Ok(NominalRepresentationShapeV1::Class {
                    base: match class.base_class {
                        Some(base) => OptionalSignatureType::Present(Box::new(map(base)?)),
                        None => OptionalSignatureType::Absent,
                    },
                    declared_fields: class_fields(export, &class.fields, nominal.owner, &map)?,
                }),
                ClassRepresentation::Intrinsic(declaration) => {
                    Ok(NominalRepresentationShapeV1::Intrinsic {
                        representation: NominalIntrinsicRepresentationV1::new(declaration.kind),
                    })
                }
            }
        }
        NominalLocalId::Interface(_) => Ok(NominalRepresentationShapeV1::Interface),
        NominalLocalId::Object(id) => {
            let backing = export.objects[id].backing_class;
            let backing_id = export.nominal_identities[backing]
                .concrete_type_id()
                .ok_or(Error::MissingExactIdentity)?;
            Ok(NominalRepresentationShapeV1::Object {
                backing_class: backing_id,
                declared_fields: class_fields(
                    export,
                    &export.classes[backing].fields,
                    nominal.owner,
                    &map,
                )?,
            })
        }
    }
}

fn enumeration(
    export: &ExportHir,
    local: &LocalConcreteHir,
    nominal: &ConcreteNominal<'_>,
    id: EnumId,
    map: &impl Fn(TypeId) -> Result<scoop_identity::SignatureTypeKey, Error>,
) -> Result<NominalRepresentationShapeV1, Error> {
    let declaration = &export.enums[id];
    let concrete_ty = local
        .exact_type_identities
        .type_for_identity(nominal.exact)
        .ok_or(Error::MissingConcreteType(nominal.exact))?;
    let concrete::TypeKind::Enum(concrete_id) = local.types[concrete_ty].kind else {
        return Err(Error::ExactIdentityMismatch(nominal.exact));
    };
    let mut variants = Vec::with_capacity(declaration.variants.len());
    for (variant_index, variant) in declaration.variants.iter().enumerate() {
        let reference = EnumVariantRef::checked(&export.enums, id, variant_index as u32)
            .ok_or_else(|| Error::InvalidRepresentation {
                declaration: SourceNominalId::Concrete(nominal.owner),
                reason: format!("missing enum variant {variant_index}"),
            })?;
        let mut fields = Vec::with_capacity(variant.fields.len());
        for (field_index, field) in variant.fields.iter().enumerate() {
            let field_ref =
                EnumVariantFieldRef::checked(&export.enums, reference, field_index as u32)
                    .ok_or_else(|| Error::InvalidRepresentation {
                        declaration: SourceNominalId::Concrete(nominal.owner),
                        reason: format!("missing enum field {variant_index}:{field_index}"),
                    })?;
            fields.push(
                EnumRepresentationFieldV1::try_new(
                    export.enum_member_identities[field_ref].key(),
                    map(field.ty)?,
                )
                .map_err(|error| Error::InvalidRepresentation {
                    declaration: SourceNominalId::Concrete(nominal.owner),
                    reason: error.to_string(),
                })?,
            );
        }
        let gc = if local.enums[concrete_id].variants[variant_index].gc_free {
            ExactTypeGcV1::GcFree
        } else {
            ExactTypeGcV1::ContainsManagedReferences
        };
        variants.push(
            EnumRepresentationVariantV1::try_new(
                export.enum_member_identities[reference].key(),
                fields,
                gc,
            )
            .map_err(|error| Error::InvalidRepresentation {
                declaration: SourceNominalId::Concrete(nominal.owner),
                reason: error.to_string(),
            })?,
        );
    }
    Ok(NominalRepresentationShapeV1::Enum { variants })
}

fn class_fields(
    export: &ExportHir,
    fields: &[ClassFieldId],
    owner: PersistentTypeId,
    map: &impl Fn(TypeId) -> Result<scoop_identity::SignatureTypeKey, Error>,
) -> Result<Vec<ClassRepresentationFieldV1>, Error> {
    fields
        .iter()
        .map(|field| {
            ClassRepresentationFieldV1::try_new(
                export.field_identities[*field].key(),
                map(export.class_fields[*field].ty)?,
            )
            .map_err(|error| Error::InvalidRepresentation {
                declaration: SourceNominalId::Concrete(owner),
                reason: error.to_string(),
            })
        })
        .collect()
}
