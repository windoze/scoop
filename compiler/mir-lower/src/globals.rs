use super::*;

pub(super) fn lower_managed_static_state(
    state: &hir::HirStaticInitialState,
    ty: &mir::Type,
    structs: &Arena<mir::StructDef>,
    strings: &mut Arena<mir::StringConst>,
) -> mir::MirStaticInitialState {
    match state {
        hir::HirStaticInitialState::ZeroedForRuntimeUnit { .. } => {
            mir::MirStaticInitialState::ZeroedForRuntimeUnit
        }
        hir::HirStaticInitialState::EncodedStaticValue { payload } => {
            lower_encoded_static_state(payload, ty, structs, strings)
        }
    }
}

pub(super) fn lower_encoded_static_state(
    payload: &hir::HirConstantImage,
    ty: &mir::Type,
    structs: &Arena<mir::StructDef>,
    strings: &mut Arena<mir::StringConst>,
) -> mir::MirStaticInitialState {
    mir::MirStaticInitialState::EncodedStaticValue {
        payload: lower_global_constant(payload, ty, structs, strings),
    }
}

pub(super) fn lower_global_constant(
    value: &hir::HirConstantImage,
    ty: &mir::Type,
    structs: &Arena<mir::StructDef>,
    strings: &mut Arena<mir::StringConst>,
) -> mir::MirConstantImage {
    match (value, ty) {
        (hir::HirConstantImage::Integer(value), mir::Type::Integer(kind)) => {
            let value = lower_integer_constant(*value);
            assert_eq!(
                value.kind(),
                *kind,
                "static integer image has its exact type"
            );
            mir::MirConstantImage::Integer(value)
        }
        (hir::HirConstantImage::Boolean(value), mir::Type::Boolean) => {
            mir::MirConstantImage::Boolean(*value)
        }
        (hir::HirConstantImage::String(value), mir::Type::String) => {
            let symbol = format!("scoop.str.{}", strings.len());
            let id = strings.alloc(mir::StringConst {
                value: value.clone(),
                symbol,
            });
            mir::MirConstantImage::String(id)
        }
        (hir::HirConstantImage::NullPointer(hir::HirPointerNullKind::Raw), mir::Type::Ptr(_)) => {
            mir::MirConstantImage::PointerNull(mir::MirPointerNull::Data)
        }
        (
            hir::HirConstantImage::NullPointer(hir::HirPointerNullKind::Code),
            mir::Type::FunPtr(_),
        ) => mir::MirConstantImage::PointerNull(mir::MirPointerNull::Code),
        (hir::HirConstantImage::EnumUnit { variant, .. }, mir::Type::Enum(enum_id, _)) => {
            mir::MirConstantImage::EnumUnit {
                enum_id: *enum_id,
                variant: *variant,
            }
        }
        (hir::HirConstantImage::Struct { fields, .. }, mir::Type::Struct(struct_id)) => {
            let definition = &structs[*struct_id];
            let definition_fields = definition.declared_fields();
            assert_eq!(
                fields.len(),
                definition_fields.len(),
                "typed global struct constants preserve field arity"
            );
            mir::MirConstantImage::Struct {
                struct_id: *struct_id,
                fields: fields
                    .iter()
                    .zip(definition_fields)
                    .map(|(field, definition)| {
                        lower_global_constant(field, &definition.ty, structs, strings)
                    })
                    .collect(),
            }
        }
        _ => unreachable!("HIR global constants match their declared type"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_exact_integer_static_image_and_state_is_preserved() {
        let values = [
            hir::HirIntegerConstant::Signed8(u8::MAX),
            hir::HirIntegerConstant::Signed16(u16::MAX),
            hir::HirIntegerConstant::Signed32(u32::MAX),
            hir::HirIntegerConstant::Signed64(u64::MAX),
            hir::HirIntegerConstant::Unsigned8(u8::MAX),
            hir::HirIntegerConstant::Unsigned16(u16::MAX),
            hir::HirIntegerConstant::Unsigned32(u32::MAX),
            hir::HirIntegerConstant::Unsigned64(u64::MAX),
        ];
        let structs = Arena::new();
        let mut strings = Arena::new();
        for value in values {
            let kind = lower_integer_kind(value.kind());
            let state = lower_encoded_static_state(
                &hir::HirConstantImage::Integer(value),
                &mir::Type::Integer(kind),
                &structs,
                &mut strings,
            );
            let mir::MirStaticInitialState::EncodedStaticValue {
                payload: mir::MirConstantImage::Integer(lowered),
            } = state
            else {
                panic!("an integer static image stays an encoded integer")
            };
            assert_eq!(lowered.kind(), kind);
            assert_eq!(lowered.raw_bits(), kind.width().raw_mask());
        }
    }

    #[test]
    fn runtime_initialized_managed_storage_keeps_its_distinct_zero_state() {
        let state = lower_managed_static_state(
            &hir::HirStaticInitialState::ZeroedForRuntimeUnit {
                unit: hir::InitializationUnitId::from_raw(0.into()),
            },
            &mir::Type::Unit,
            &Arena::new(),
            &mut Arena::new(),
        );
        assert_eq!(state, mir::MirStaticInitialState::ZeroedForRuntimeUnit);
    }
}
