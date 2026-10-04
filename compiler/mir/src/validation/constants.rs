use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum MirConstantImageError {
    TypeMismatch {
        image: &'static str,
    },
    InvalidStringReference {
        string: StringConstId,
    },
    InvalidStructReference {
        struct_id: StructId,
    },
    StructRequiresDeclared {
        struct_id: StructId,
    },
    StructArity {
        struct_id: StructId,
        expected: usize,
        actual: usize,
    },
    InvalidVariantReference {
        error: MirVariantRefError,
    },
    EnumUnitHasPayload {
        variant: MirVariantRef,
    },
}

type ConstantImageFailure = (Vec<u32>, Type, MirConstantImageError);

pub(super) fn validate_constant_images(module: &Module) -> Result<(), MirValidationError> {
    for (global_id, global) in module.globals.iter() {
        let payload = match &global.storage {
            GlobalStorage::Managed {
                initial_state: MirStaticInitialState::EncodedStaticValue { payload },
            }
            | GlobalStorage::Local {
                initializer: payload,
                ..
            } => payload,
            GlobalStorage::Managed {
                initial_state: MirStaticInitialState::ZeroedForRuntimeUnit,
            }
            | GlobalStorage::Extern { .. }
            | GlobalStorage::Imported { .. } => continue,
        };
        validate_constant_image(module, payload, &global.ty, &mut Vec::new()).map_err(
            |(path, expected, error)| MirValidationError {
                location: MirValidationLocation::Global { global: global_id },
                kind: MirValidationErrorKind::InvalidConstantImage {
                    path,
                    expected,
                    error,
                },
            },
        )?;
    }
    Ok(())
}

fn validate_constant_image(
    module: &Module,
    image: &MirConstantImage,
    expected: &Type,
    path: &mut Vec<u32>,
) -> Result<(), ConstantImageFailure> {
    let failure = |error| (path.clone(), expected.clone(), error);
    match image {
        MirConstantImage::Integer(value) => {
            if expected != &Type::Integer(value.kind()) {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "integer image",
                }));
            }
        }
        MirConstantImage::Char(_) => {
            if !matches!(expected, Type::Struct(id)
                if (id.into_raw().into_u32() as usize) < module.structs.len()
                    && matches!(module.structs[*id].representation,
                        StructRepresentation::Intrinsic(IntrinsicTypeRepresentation::Char)))
            {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "Char image",
                }));
            }
        }
        MirConstantImage::Boolean(_) => {
            if expected != &Type::Boolean {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "Boolean image",
                }));
            }
        }
        MirConstantImage::String(string) => {
            if expected != &Type::String {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "String image",
                }));
            }
            if string.into_raw().into_u32() as usize >= module.strings.len() {
                return Err(failure(MirConstantImageError::InvalidStringReference {
                    string: *string,
                }));
            }
        }
        MirConstantImage::PointerNull(MirPointerNull::Data) => {
            if !matches!(expected, Type::Ptr(_)) {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "raw-data null image",
                }));
            }
        }
        MirConstantImage::PointerNull(MirPointerNull::Code) => {
            if !matches!(expected, Type::FunPtr(_)) {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "code-pointer null image",
                }));
            }
        }
        MirConstantImage::EnumUnit { variant } => {
            let definition = variant.definition(&module.enums).map_err(|error| {
                failure(MirConstantImageError::InvalidVariantReference { error })
            })?;
            if expected
                != &variant
                    .enum_type(&module.enums)
                    .expect("the variant was validated")
            {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "EnumUnit image",
                }));
            }
            if !definition.fields.is_empty() {
                return Err(failure(MirConstantImageError::EnumUnitHasPayload {
                    variant: *variant,
                }));
            }
        }
        MirConstantImage::Struct { struct_id, fields } => {
            if struct_id.into_raw().into_u32() as usize >= module.structs.len() {
                return Err(failure(MirConstantImageError::InvalidStructReference {
                    struct_id: *struct_id,
                }));
            }
            if expected != &Type::Struct(*struct_id) {
                return Err(failure(MirConstantImageError::TypeMismatch {
                    image: "Struct image",
                }));
            }
            let StructRepresentation::Declared {
                fields: expected_fields,
                ..
            } = &module.structs[*struct_id].representation
            else {
                return Err(failure(MirConstantImageError::StructRequiresDeclared {
                    struct_id: *struct_id,
                }));
            };
            if fields.len() != expected_fields.len() {
                return Err(failure(MirConstantImageError::StructArity {
                    struct_id: *struct_id,
                    expected: expected_fields.len(),
                    actual: fields.len(),
                }));
            }
            for (index, (field, definition)) in
                fields.iter().zip(expected_fields.iter()).enumerate()
            {
                path.push(index as u32);
                let result = validate_constant_image(module, field, &definition.ty, path);
                path.pop();
                result?;
            }
        }
    }
    Ok(())
}
