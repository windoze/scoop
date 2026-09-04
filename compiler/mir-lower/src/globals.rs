use super::*;

pub(super) fn lower_global_constant(
    value: &hir::ConstantValue,
    ty: &mir::Type,
    structs: &Arena<mir::StructDef>,
    strings: &mut Arena<mir::StringConst>,
) -> mir::ConstantValue {
    match (value, ty) {
        (hir::ConstantValue::Int(value), mir::Type::Int | mir::Type::UInt) => {
            mir::ConstantValue::Int(*value)
        }
        (hir::ConstantValue::Bool(value), mir::Type::Boolean) => mir::ConstantValue::Bool(*value),
        (hir::ConstantValue::String(value), mir::Type::String) => {
            let symbol = format!("scoop.str.{}", strings.len());
            let id = strings.alloc(mir::StringConst {
                value: value.clone(),
                symbol,
            });
            mir::ConstantValue::String(id)
        }
        (hir::ConstantValue::NullPtr, mir::Type::Ptr(_)) => mir::ConstantValue::NullPtr,
        (hir::ConstantValue::NullFunPtr, mir::Type::FunPtr(_)) => mir::ConstantValue::NullFunPtr,
        (hir::ConstantValue::EnumUnit { .. }, mir::Type::Enum(enum_id, _)) => {
            mir::ConstantValue::EnumUnit {
                enum_id: *enum_id,
                variant: match value {
                    hir::ConstantValue::EnumUnit { variant, .. } => *variant,
                    _ => unreachable!("the outer pattern is an enum unit constant"),
                },
            }
        }
        (hir::ConstantValue::Struct { fields, .. }, mir::Type::Struct(struct_id)) => {
            let definition = &structs[*struct_id];
            let definition_fields = definition.declared_fields();
            assert_eq!(
                fields.len(),
                definition_fields.len(),
                "typed global struct constants preserve field arity"
            );
            mir::ConstantValue::Struct {
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
