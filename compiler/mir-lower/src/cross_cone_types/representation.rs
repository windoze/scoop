use super::*;
use hir::NominalRepresentationShapeV1 as Source;
use mir::MirTypeRepresentationV1 as Repr;

mod fields;

type Backing = (PersistentTypeId, PersistentExactTypeId, Repr);

pub(super) fn project(
    module: &mir::Module,
    ty: &mir::Type,
    source: &hir::NominalRepresentationSupportV1,
    meter: &mut BudgetMeter,
) -> Result<(Repr, Option<Backing>), SourceMirTypeProductionError> {
    work(1, meter)?;
    let mismatch = || SourceMirTypeProductionError::RepresentationMismatch(source.owner());
    let shape = match (ty, source.shape()) {
        (mir::Type::Unit, Source::Object { .. } | Source::Struct { .. }) => {
            Repr::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit)
        }
        (mir::Type::Integer(kind), Source::Intrinsic { .. }) => {
            Repr::Intrinsic(mir::MirParamFreeIntrinsicV1::Integer(*kind))
        }
        (mir::Type::Boolean, Source::Intrinsic { .. }) => {
            Repr::Intrinsic(mir::MirParamFreeIntrinsicV1::Boolean)
        }
        (mir::Type::String, Source::Intrinsic { .. }) => {
            Repr::Intrinsic(mir::MirParamFreeIntrinsicV1::String)
        }
        (
            mir::Type::Struct(id),
            Source::Struct {
                fields: expected, ..
            },
        ) => {
            let mir::StructRepresentation::Declared {
                fields,
                c_layout,
                interior_mutable,
            } = &module.structs[*id].representation
            else {
                return Err(mismatch());
            };
            if fields.len() != expected.len()
                || fields
                    .iter()
                    .zip(expected)
                    .any(|(field, expected)| field.identity != expected.field())
            {
                return Err(mismatch());
            }
            let mut projected = Vec::new();
            reserve(&mut projected, fields.len(), meter)?;
            for field in fields {
                projected.push(mir::MirRepresentationFieldV1 {
                    field: field.identity,
                    value: fields::exact(module, &field.ty, meter)?,
                });
            }
            Repr::Struct {
                fields: projected,
                c_layout: c_layout.map_or(
                    mir::MirTypeCLayoutPolicyV1::Ordinary,
                    mir::MirTypeCLayoutPolicyV1::CLayout,
                ),
                interior_mutable: *interior_mutable,
            }
        }
        (mir::Type::Enum(id, _), Source::Enum { variants: expected }) => {
            let variants = &module.enums[*id].variants;
            if variants.len() != expected.len()
                || variants.iter().zip(expected).any(|(variant, expected)| {
                    variant.identity != expected.variant()
                        || variant.fields.len() != expected.fields().len()
                        || variant
                            .fields
                            .iter()
                            .zip(expected.fields())
                            .any(|(field, expected)| field.identity != expected.field())
                })
            {
                return Err(mismatch());
            }
            Repr::Enum {
                variants: fields::variants(module, variants, meter)?,
            }
        }
        (
            mir::Type::Class(id),
            Source::Class {
                declared_fields, ..
            },
        ) => {
            let class = &module.classes[*id];
            Repr::Class {
                kind: match class.modifier {
                    mir::ClassModifier::Final => mir::MirClassKindV1::Final,
                    mir::ClassModifier::Open => mir::MirClassKindV1::Open,
                    mir::ClassModifier::Abstract => mir::MirClassKindV1::Abstract,
                },
                declared_fields: fields::class(
                    module,
                    class,
                    source.owner(),
                    declared_fields,
                    meter,
                )?,
            }
        }
        (mir::Type::Interface(_) | mir::Type::Any, Source::Interface) => Repr::Interface,
        (
            mir::Type::Class(id),
            Source::Object {
                backing_class,
                declared_fields,
            },
        ) => {
            let fields = fields::class(
                module,
                &module.classes[*id],
                source.owner(),
                declared_fields,
                meter,
            )?;
            let key = ExactTypeKey::Nominal(*backing_class);
            let length = scoop_wire::encoded_length(&key)
                .map_err(|error| SourceMirTypeProductionError::Identity(error.to_string()))?;
            meter
                .charge_sha256(length, &WirePath::root())
                .map_err(SourceMirTypeProductionError::Resource)?;
            let exact = PersistentExactTypeId::from_key(&key)
                .map_err(|error| SourceMirTypeProductionError::Identity(error.to_string()))?;
            return Ok((
                Repr::Object { backing: exact },
                Some((
                    *backing_class,
                    exact,
                    Repr::ObjectBacking {
                        declared_fields: fields,
                    },
                )),
            ));
        }
        _ => return Err(mismatch()),
    };
    Ok((shape, None))
}
