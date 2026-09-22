use super::*;

#[test]
fn c_uint64_projection_requires_a_single_gc_free_unsigned_field_with_the_exact_id() {
    for invalid in 0..6 {
        let fields = match invalid {
            1 => vec![Type::Integer(IntegerKind::SIGNED_64)],
            2 => vec![Type::Integer(IntegerKind::UNSIGNED_64); 2],
            _ => vec![Type::Integer(IntegerKind::UNSIGNED_64)],
        };
        let (mut module, id) = module_with_declared_struct(fields);
        let definition = &mut module.structs[id];
        let StructRepresentation::Declared {
            fields,
            c_abi,
            c_layout,
            ..
        } = &mut definition.representation
        else {
            unreachable!()
        };
        *c_abi = StructCAbi::UInt64Field {
            field: if invalid == 3 {
                test_struct_field("Other", "f0")
            } else {
                fields[0].identity
            },
        };
        if invalid == 4 {
            *c_layout = Some(MirCLayoutContract {
                aligned: MirCLayoutValue::Natural,
                packed: MirCLayoutValue::Natural,
            });
        }
        if invalid == 5 {
            definition.gc_free = false;
        }
        let result = validate_module(&module);
        if invalid == 0 {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert_eq!(
                error.kind,
                MirValidationErrorKind::InvalidStructCAbiProjection
            );
            assert_eq!(
                error.location,
                MirValidationLocation::RuntimeType {
                    location: MirRuntimeTypeLocation::Struct(id)
                }
            );
        }
    }
}
