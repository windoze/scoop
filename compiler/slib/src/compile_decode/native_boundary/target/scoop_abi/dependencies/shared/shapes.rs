use super::*;
use scoop_hir::{
    NativeBoundaryFieldDefinition, NativeBoundaryNominalShape, NativeBoundaryVariantDefinition,
    NativeBoundaryVariantFieldDefinition, NominalSourceShapeV1,
};
use scoop_identity::{EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey};

pub(super) fn project(
    shape: &NominalSourceShapeV1,
    owner: NativeBoundaryNominalOwner,
    identities: &ValidatedIdentityGraph,
) -> Result<NativeBoundaryNominalShape, NativeBoundaryCompileError> {
    let path = WirePath::root().field(34);
    Ok(match shape {
        NominalSourceShapeV1::Class(_)
        | NominalSourceShapeV1::Interface
        | NominalSourceShapeV1::Object(_) => NativeBoundaryNominalShape::Reference,
        NominalSourceShapeV1::Intrinsic(representation) => {
            NativeBoundaryNominalShape::Intrinsic(*representation)
        }
        NominalSourceShapeV1::Struct(source) => {
            let mut fields = allocate_vec(source.fields().len(), &path)?;
            for field in source.fields() {
                let key = identities
                    .canonical_key::<_, FieldIdentityKey>(field.field())
                    .map_err(NativeBoundaryCompileError::Reference)?;
                let field = NativeBoundaryFieldDefinition::new(&key, (field.value_type()).clone())
                    .map_err(NativeBoundaryCompileError::TypeDefinition)?;
                if field.owner() != owner.declaration_owner() {
                    return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
                }
                fields.push(field);
            }
            NativeBoundaryNominalShape::Struct {
                c_layout: source.c_layout_policy().into(),
                fields,
            }
        }
        NominalSourceShapeV1::Enum(source) => {
            let mut variants = allocate_vec(source.variants().len(), &path)?;
            for variant in source.variants() {
                let key = identities
                    .canonical_key::<_, EnumVariantIdentityKey>(variant.variant())
                    .map_err(NativeBoundaryCompileError::Reference)?;
                let mut fields = allocate_vec(variant.fields().len(), &path)?;
                for field in variant.fields() {
                    let key = identities
                        .canonical_key::<_, EnumVariantFieldKey>(field.field())
                        .map_err(NativeBoundaryCompileError::Reference)?;
                    fields.push(
                        NativeBoundaryVariantFieldDefinition::new(
                            &key,
                            (field.value_type()).clone(),
                        )
                        .map_err(NativeBoundaryCompileError::TypeDefinition)?,
                    );
                }
                let variant = NativeBoundaryVariantDefinition::new(&key, fields)
                    .map_err(NativeBoundaryCompileError::TypeDefinition)?;
                if variant.owner() != owner.declaration_owner() {
                    return Err(NativeBoundaryCompileError::ConflictingTypeWitness { owner });
                }
                variants.push(variant);
            }
            NativeBoundaryNominalShape::Enum { variants }
        }
    })
}
