use la_arena::Idx;
use scoop_identity::CoreBuiltinNominal;

use super::declarations::LocalNominalDeclaration;
use super::{HirNativeBoundaryTypeDefinitionError, HirNativeBoundaryTypeDefinitionInputs};
use crate::{
    EnumVariantFieldRef, EnumVariantRef, HirNominalIdentity, HirSignatureBinder,
    HirSignatureTypeMapper, HirSourceNominalIdentity, NativeBoundaryCLayoutPolicy,
    NativeBoundaryFieldDefinition, NativeBoundaryNominalShape, NativeBoundaryTypeDefinitionRecord,
    NativeBoundaryVariantDefinition, NativeBoundaryVariantFieldDefinition, StructFieldRef,
    TypeParamDecl,
};

pub(super) fn build(
    declaration: LocalNominalDeclaration,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
    mapper: &HirSignatureTypeMapper<'_>,
) -> Result<NativeBoundaryTypeDefinitionRecord, HirNativeBoundaryTypeDefinitionError> {
    match declaration {
        LocalNominalDeclaration::CoreBuiltin(builtin) => core_builtin(builtin, inputs),
        LocalNominalDeclaration::Struct(id) => {
            let structure = &inputs.structs[id];
            let source = source_nominal(&inputs.nominal_identities[id])?;
            let binders = nominal_binders(&structure.type_params)?;
            let mut fields = Vec::with_capacity(structure.semantic_fields().len());
            for (index, field) in structure.semantic_fields().iter().enumerate() {
                let field_ref = StructFieldRef::checked(inputs.structs, id, checked_index(index)?)
                    .ok_or(HirNativeBoundaryTypeDefinitionError::InvalidStructField {
                        structure: raw_index(id),
                        field: index,
                    })?;
                let ty = mapper
                    .map(field.ty, &binders)
                    .map_err(HirNativeBoundaryTypeDefinitionError::InvalidSignatureType)?;
                fields.push(
                    NativeBoundaryFieldDefinition::new(
                        inputs.field_identities[field_ref].key(),
                        ty,
                    )
                    .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)?,
                );
            }
            NativeBoundaryTypeDefinitionRecord::new(
                source.declaration(),
                &[checked_parameter_count(&structure.type_params)?],
                NativeBoundaryNominalShape::Struct {
                    c_layout: crate::NominalCLayoutPolicyV1::from_source_contract(
                        structure.attributes.c_layout,
                    )
                    .into(),
                    fields,
                },
            )
            .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)
        }
        LocalNominalDeclaration::Enum(id) => {
            let enumeration = &inputs.enums[id];
            let source = source_nominal(&inputs.nominal_identities[id])?;
            let binders = nominal_binders(&enumeration.type_params)?;
            let mut variants = Vec::with_capacity(enumeration.variants.len());
            for (variant_index, variant) in enumeration.variants.iter().enumerate() {
                let variant_ref =
                    EnumVariantRef::checked(inputs.enums, id, checked_index(variant_index)?)
                        .ok_or(HirNativeBoundaryTypeDefinitionError::InvalidEnumVariant {
                            enumeration: raw_index(id),
                            variant: variant_index,
                        })?;
                let mut fields = Vec::with_capacity(variant.fields.len());
                for (field_index, field) in variant.fields.iter().enumerate() {
                    let field_ref = EnumVariantFieldRef::checked(
                        inputs.enums,
                        variant_ref,
                        checked_index(field_index)?,
                    )
                    .ok_or(
                        HirNativeBoundaryTypeDefinitionError::InvalidEnumVariantField {
                            enumeration: raw_index(id),
                            variant: variant_index,
                            field: field_index,
                        },
                    )?;
                    let ty = mapper
                        .map(field.ty, &binders)
                        .map_err(HirNativeBoundaryTypeDefinitionError::InvalidSignatureType)?;
                    fields.push(
                        NativeBoundaryVariantFieldDefinition::new(
                            inputs.enum_member_identities[field_ref].key(),
                            ty,
                        )
                        .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)?,
                    );
                }
                variants.push(
                    NativeBoundaryVariantDefinition::new(
                        inputs.enum_member_identities[variant_ref].key(),
                        fields,
                    )
                    .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)?,
                );
            }
            NativeBoundaryTypeDefinitionRecord::new(
                source.declaration(),
                &[checked_parameter_count(&enumeration.type_params)?],
                NativeBoundaryNominalShape::Enum { variants },
            )
            .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)
        }
        LocalNominalDeclaration::Class(id) => reference(
            source_nominal(&inputs.nominal_identities[id])?,
            &inputs.classes[id].type_params,
        ),
        LocalNominalDeclaration::Interface(id) => reference(
            source_nominal(&inputs.nominal_identities[id])?,
            &inputs.interfaces[id].type_params,
        ),
        LocalNominalDeclaration::Object(id) => {
            reference(source_nominal(&inputs.nominal_identities[id])?, &[])
        }
    }
}

fn core_builtin(
    builtin: CoreBuiltinNominal,
    inputs: &HirNativeBoundaryTypeDefinitionInputs<'_>,
) -> Result<NativeBoundaryTypeDefinitionRecord, HirNativeBoundaryTypeDefinitionError> {
    let shape = match builtin {
        CoreBuiltinNominal::Unit => NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: Vec::new(),
        },
        CoreBuiltinNominal::Any => NativeBoundaryNominalShape::Reference,
    };
    NativeBoundaryTypeDefinitionRecord::new(
        inputs.nominal_identities.core_builtin(builtin).key(),
        &[0],
        shape,
    )
    .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)
}

fn reference(
    identity: &HirSourceNominalIdentity,
    parameters: &[TypeParamDecl],
) -> Result<NativeBoundaryTypeDefinitionRecord, HirNativeBoundaryTypeDefinitionError> {
    NativeBoundaryTypeDefinitionRecord::new(
        identity.declaration(),
        &[checked_parameter_count(parameters)?],
        NativeBoundaryNominalShape::Reference,
    )
    .map_err(HirNativeBoundaryTypeDefinitionError::InvalidDefinition)
}

fn source_nominal(
    identity: &HirNominalIdentity,
) -> Result<&HirSourceNominalIdentity, HirNativeBoundaryTypeDefinitionError> {
    identity
        .source()
        .ok_or(HirNativeBoundaryTypeDefinitionError::UnexpectedGeneratedNominal)
}

fn nominal_binders(
    parameters: &[TypeParamDecl],
) -> Result<Vec<HirSignatureBinder>, HirNativeBoundaryTypeDefinitionError> {
    parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(HirSignatureBinder {
                parameter: parameter.id,
                depth: 0,
                index: checked_index(index)?,
            })
        })
        .collect()
}

fn checked_parameter_count(
    parameters: &[TypeParamDecl],
) -> Result<u32, HirNativeBoundaryTypeDefinitionError> {
    u32::try_from(parameters.len())
        .map_err(|_| HirNativeBoundaryTypeDefinitionError::TooManyTypeParameters)
}

fn checked_index(index: usize) -> Result<u32, HirNativeBoundaryTypeDefinitionError> {
    u32::try_from(index).map_err(|_| HirNativeBoundaryTypeDefinitionError::SourceOrderOverflow)
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
