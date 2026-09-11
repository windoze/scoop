//! Defensive validation for recursively typed global constant images.

use super::super::*;

pub(super) fn validate_constant_images(module: &Module) -> Result<(), CodegenError> {
    for (_, global) in module.globals.iter() {
        let GlobalInit::Storage {
            ty,
            initial_state: LirStaticInitialState::EncodedStaticValue { payload },
            ..
        } = &global.init
        else {
            continue;
        };
        validate_constant_image(module, global, ty, payload, "value")?;
    }
    Ok(())
}

fn validate_constant_image(
    module: &Module,
    global: &Global,
    expected: &LirType,
    image: &LirConstantImage,
    path: &str,
) -> Result<(), CodegenError> {
    match image {
        LirConstantImage::Integer(value) => {
            require_exact_type(global, path, expected, &value.scalar_type(), || {
                format!("{} constant", value.kind().canonical_name())
            })
        }
        LirConstantImage::Bool(_) => {
            require_exact_type(global, path, expected, &LirType::I1, || {
                "Boolean constant".to_string()
            })
        }
        LirConstantImage::NullPointer(kind) => {
            require_exact_type(global, path, expected, &LirType::Ptr(*kind), || {
                format!("{} null constant", kind.dump())
            })
        }
        LirConstantImage::GlobalPointer {
            global: target_id,
            kind,
        } => {
            require_exact_type(global, path, expected, &LirType::Ptr(*kind), || {
                format!("{} global pointer constant", kind.dump())
            })?;
            let target_index = target_id.into_raw().into_u32() as usize;
            if target_index >= module.globals.len() {
                return Err(constant_error(
                    global,
                    path,
                    format!(
                        "global pointer constant references unavailable global {}",
                        target_id.into_raw().into_u32()
                    ),
                ));
            }
            let target = &module.globals[*target_id];
            if target.address_kind != *kind {
                return Err(constant_error(
                    global,
                    path,
                    format!(
                        "global pointer constant declares {} provenance but referenced global `@{}` has {} provenance",
                        kind.dump(),
                        target.symbol(),
                        target.address_kind.dump()
                    ),
                ));
            }
            Ok(())
        }
        LirConstantImage::EnumUnit { variant } => {
            let owner = format!("storage global `@{}` constant {path}", global.symbol());
            super::validate_variant_ref(module, *variant, &owner)?;
            let enum_id = variant.definition();
            require_exact_type(global, path, expected, &LirType::Enum(enum_id), || {
                format!("enum unit constant for e{}", enum_id.into_raw())
            })?;
            let has_payload = match &module.enums[enum_id].repr {
                EnumRepr::Niche {
                    payload_variant, ..
                } => variant.index() == *payload_variant,
                EnumRepr::Tagged { variants, .. } => {
                    !variants[variant.index() as usize].fields.is_empty()
                }
            };
            if has_payload {
                return Err(constant_error(
                    global,
                    path,
                    "a payload enum variant cannot be encoded as a unit constant",
                ));
            }
            Ok(())
        }
        LirConstantImage::Struct { struct_id, fields } => {
            let struct_index = struct_id.into_raw().into_u32() as usize;
            if struct_index >= module.structs.len() {
                return Err(constant_error(
                    global,
                    path,
                    format!(
                        "struct constant carries invalid struct{} reference",
                        struct_id.into_raw()
                    ),
                ));
            }
            require_exact_type(global, path, expected, &LirType::Struct(*struct_id), || {
                format!("struct constant for s{}", struct_id.into_raw())
            })?;
            let definition = &module.structs[*struct_id];
            let field_types = match &definition.representation {
                StructRepresentation::Scoop { fields } => fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect::<Vec<_>>(),
                StructRepresentation::C { fields, .. } => fields
                    .iter()
                    .map(|field| field.ty.storage_type())
                    .collect::<Vec<_>>(),
                StructRepresentation::Intrinsic(representation) => {
                    return Err(constant_error(
                        global,
                        path,
                        format!(
                            "compiler intrinsic declaration shell `{}` ({representation:?}) cannot be encoded as an aggregate constant",
                            definition.name
                        ),
                    ));
                }
            };
            if fields.len() != field_types.len() {
                return Err(constant_error(
                    global,
                    path,
                    format!(
                        "struct constant for `{}` has {} fields, expected {}",
                        definition.name,
                        fields.len(),
                        field_types.len()
                    ),
                ));
            }
            for (index, (field, field_type)) in fields.iter().zip(field_types).enumerate() {
                validate_constant_image(
                    module,
                    global,
                    &field_type,
                    field,
                    &format!("{path}.field[{index}]"),
                )?;
            }
            Ok(())
        }
    }
}

fn require_exact_type(
    global: &Global,
    path: &str,
    expected: &LirType,
    actual: &LirType,
    subject: impl FnOnce() -> String,
) -> Result<(), CodegenError> {
    if actual != expected {
        return Err(constant_error(
            global,
            path,
            format!(
                "{} does not match storage type {}",
                subject(),
                expected.dump()
            ),
        ));
    }
    Ok(())
}

fn constant_error(global: &Global, path: &str, detail: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!(
        "storage global `@{}` constant {path}: {detail}",
        global.symbol()
    ))
}
