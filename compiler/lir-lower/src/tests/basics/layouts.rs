use super::*;

#[test]
fn struct_values_keep_named_lir_identity() {
    let mut b = Builder::new();
    let point = b.strukt("Point", &[("x", INT), ("y", INT)]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Struct(point)));
    let x = locals.alloc(local("x", INT));
    let main = b.main(
        locals,
        vec![
            val_decl(
                p,
                expr(
                    mir::Type::Struct(point),
                    mir::ExprKind::StructInit {
                        struct_id: point,
                        args: vec![int_expr(1), int_expr(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    INT,
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(p, mir::Type::Struct(point))),
                        index: 0,
                    },
                ),
            ),
        ],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let local_types: Vec<lir::LirType> =
        function.locals.iter().map(|(_, l)| l.ty.clone()).collect();
    assert_eq!(
        local_types,
        [
            lir::LirType::Struct(struct_def_id(point)),
            lir::LirType::I32,
        ]
    );

    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MakeAggregate { out, elements } = instructions[0] else {
        panic!("struct construction must build an aggregate")
    };
    assert_eq!(elements.len(), 2);
    assert_eq!(
        function.temps[*out].ty,
        lir::LirType::Struct(struct_def_id(point))
    );
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));
    let lir::Instruction::ExtractValue { out, index, .. } = instructions[2] else {
        panic!("field access must extract from the aggregate")
    };
    assert_eq!(*index, 0);
    assert_eq!(function.temps[*out].ty, lir::LirType::I32);
    assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

    // The struct layout is in the meta.
    let layout = layout_values(&module)
        .find(|l| l.name == "Point")
        .expect("a layout per struct");
    assert_eq!((layout.size, layout.align), (8, 4));
    assert!(plain_refs(layout).is_empty());
}

#[test]
fn raw_struct_construction_lowers_fields_in_declaration_order() {
    let mut builder = Builder::new();
    let pair = builder.strukt("Pair", &[("first", INT), ("second", INT)]);
    let pair_ty = mir::Type::Struct(pair);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", pair_ty.clone()));
    let main = builder.main(
        locals,
        vec![val_decl(
            value,
            expr(
                pair_ty,
                mir::ExprKind::StructConstruct {
                    struct_id: pair,
                    fields: vec![int_expr(20), int_expr(10)],
                },
            ),
        )],
    );

    let module = lower(&builder.finish(main));
    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MakeAggregate { elements, .. } = instructions[0] else {
        panic!("raw struct construction must lower to MakeAggregate")
    };
    assert_eq!(
        elements,
        &[
            lir::Value::IntegerConst(lir::LirIntegerConstant::Signed32(20)),
            lir::Value::IntegerConst(lir::LirIntegerConstant::Signed32(10)),
        ]
    );
    assert!(
        instructions
            .iter()
            .all(|instruction| !matches!(instruction, lir::Instruction::Call { .. })),
        "raw reconstruction must not call a source constructor"
    );
}

#[test]
fn unit_is_the_empty_aggregate() {
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::Unit));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::unit())]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::Aggregate(Vec::new()));
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MakeAggregate { out, elements } = instructions[0] else {
        panic!("Unit must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));

    // Unit itself gets no layout entry (it is just `{}`).
    assert!(!layout_values(&module).any(|l| l.name == "Unit"));
}

#[test]
fn all_eight_integer_kinds_use_their_exact_target_scalar_layout() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);

    for mir_kind in mir::IntegerKind::ALL {
        let lir_kind = integer_kind(mir_kind);
        assert_eq!(
            lir_type(&mir::Type::Integer(mir_kind)),
            lir_kind.scalar_type()
        );
        let intrinsic = layout_values(&module)
            .find(|layout| {
                layout.kind
                    == lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Integer(
                        lir_kind,
                    ))
            })
            .unwrap_or_else(|| panic!("missing {} intrinsic layout", mir_kind.canonical_name()));
        let expected = context.integer_layout(lir_kind);
        assert_eq!(
            (intrinsic.size, intrinsic.align),
            (expected.size, expected.align)
        );
    }
}

#[test]
fn compiler_pointer_declaration_shells_keep_target_layout_and_closed_shape() {
    let mut builder = Builder::new();
    let signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![
            mir::Type::Integer(mir::IntegerKind::SIGNED_8),
            mir::Type::Ptr(Box::new(mir::Type::Unit)),
        ],
        return_type: mir::Type::Unit,
    });
    let data_shell = builder.structs.alloc(mir::StructDef {
        type_arguments: Vec::new(),
        name: "Ptr<UInt16>".to_string(),
        gc_free: true,
        representation: mir::StructRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Ptr {
                pointee: mir::Type::Integer(mir::IntegerKind::UNSIGNED_16),
            },
        ),
    });
    let code_shell = builder.structs.alloc(mir::StructDef {
        type_arguments: Vec::new(),
        name: "FunPtr<(Int8, Ptr<Unit>) -> Unit>".to_string(),
        gc_free: true,
        representation: mir::StructRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::FunPtr { signature },
        ),
    });
    let main = builder.main(Arena::new(), Vec::new());
    let source = builder.finish(main);
    let data_type = mir::Type::Ptr(Box::new(mir::Type::Integer(mir::IntegerKind::UNSIGNED_16)));
    let code_type = mir::Type::FunPtr(signature);
    let data_exact = source
        .meta
        .source_exact_types
        .get(&data_type)
        .expect("raw pointer layout has an exact identity")
        .identity_record()
        .id();
    let code_exact = source
        .meta
        .source_exact_types
        .get(&code_type)
        .expect("native function pointer layout has an exact identity")
        .identity_record()
        .id();
    let module = lower(&source);
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);

    let data_representation = lir::IntrinsicTypeRepresentation::Ptr {
        pointee: lir::LirDataPointee::Value(Box::new(lir::LirType::I16)),
    };
    let code_representation = lir::IntrinsicTypeRepresentation::FunPtr {
        signature: lir::LirFunctionType {
            params: vec![lir::LirType::I8, lir::RAW_PTR],
            return_type: lir::LirReturnType::Void,
        },
    };
    assert_eq!(
        module.structs[struct_def_id(data_shell)].representation,
        lir::StructRepresentation::Intrinsic(data_representation.clone())
    );
    assert_eq!(
        module.structs[struct_def_id(code_shell)].representation,
        lir::StructRepresentation::Intrinsic(code_representation.clone())
    );

    for (name, kind, representation, exact, role) in [
        (
            "Ptr<UInt16>",
            lir::PointerKind::Raw,
            data_representation,
            data_exact,
            scoop_identity::RepresentationRole::ManagedValue,
        ),
        (
            "FunPtr<(Int8, Ptr<Unit>) -> Unit>",
            lir::PointerKind::Code,
            code_representation,
            code_exact,
            scoop_identity::RepresentationRole::NativeFunctionPointer,
        ),
    ] {
        let layout = layout_values(&module)
            .find(|layout| layout.name == name)
            .unwrap_or_else(|| panic!("missing compiler pointer layout {name}"));
        let expected = context.pointer_layout(kind);
        assert_eq!((layout.size, layout.align), (expected.size, expected.align));
        assert_eq!(layout.kind, lir::LayoutKind::Intrinsic(representation),);
        let identity = match role {
            scoop_identity::RepresentationRole::ManagedValue => lir::LayoutIdentity::managed_value(
                exact,
                lir::LirTargetProfile::DARWIN_AARCH64,
                lir::MaterializationRoot::lir_structural_odr(exact).unwrap(),
            ),
            scoop_identity::RepresentationRole::NativeFunctionPointer => {
                lir::LayoutIdentity::native_function_pointer(
                    exact,
                    lir::LirTargetProfile::DARWIN_AARCH64,
                    lir::MaterializationRoot::lir_structural_odr(exact).unwrap(),
                )
            }
            scoop_identity::RepresentationRole::ManagedObject
            | scoop_identity::RepresentationRole::CValue => {
                unreachable!("the fixture contains only compiler pointer layouts")
            }
        }
        .unwrap();
        assert_eq!(layout.identity, identity);
    }
}

#[test]
fn uint_and_int_are_exact_32_bit_scalars() {
    let mut b = Builder::new();
    // A UInt field in a class is four bytes behind the 16-byte header.
    let _c = b.class("C", None, &[("u", UINT)], empty_vtable(), vec![]);
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", UINT));
    let main = b.main(locals, vec![val_decl(u, uint_expr(1))]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::I32);

    let c_layout = layout_values(&module)
        .find(|l| l.name == "C")
        .expect("a layout per class");
    assert_eq!((c_layout.size, c_layout.align), (24, 8));
    assert!(plain_refs(c_layout).is_empty());
    let c_td = descriptor_values(&module)
        .find(|td| td.diagnostic_name == "C")
        .expect("a TypeDescriptor per class");
    assert_eq!(c_td.instance_shape.minimum_size(), 24);
    assert_eq!(*fixed_scan(c_td), lir::RefScan::None);
}

#[test]
fn layouts_mark_reference_fields_for_the_gc() {
    let mut b = Builder::new();
    // String field behind one Int: the reference sits at offset 8.
    let s = b.strukt("S", &[("a", INT), ("s", mir::Type::String)]);
    // A String nested inside a tuple field, after a Boolean: the
    // tuple is 8-aligned, so it starts at offset 8.
    let pair = mir::Type::Tuple(vec![mir::Type::String, INT]);
    let _outer = b.strukt("Outer", &[("flag", mir::Type::Boolean), ("pair", pair)]);
    let _ = s;
    // A tuple type that only appears in code (padding: Boolean
    // then Int at offset 8).
    let mut locals = Arena::new();
    let _t = locals.alloc(local("t", mir::Type::Tuple(vec![mir::Type::Boolean, INT])));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };

    // The String singleton is structurally separate; the remaining typed
    // intrinsic layouts stay in declaration order with ordinary layouts.
    let names: Vec<&str> = layout_values(&module).map(|l| l.name.as_str()).collect();
    assert_eq!(
        module.meta.layouts[module.meta.well_known_layouts.string].kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(
        names,
        [
            "S",
            "Outer",
            "Int8",
            "Int16",
            "Int",
            "Long",
            "UInt8",
            "UInt16",
            "UInt",
            "ULong",
            "Boolean",
            "String",
            "(String, Int)",
            "(Boolean, Int)"
        ]
    );

    let string = &module.meta.layouts[module.meta.well_known_layouts.string];
    assert_eq!((string.size, string.align), (24, 8));
    assert!(string.fields.is_empty());

    // S { a: Int @0, s: String @8 }: size 16, align 8, refs [8].
    let s_layout = by_name("S");
    assert_eq!((s_layout.size, s_layout.align), (16, 8));
    assert_eq!(plain_refs(s_layout), [8]);

    // Outer { flag: Boolean @0, pair: (String, Int) @8 } with the
    // String at pair+0: size 24, align 8, refs [8].
    let outer = by_name("Outer");
    assert_eq!((outer.size, outer.align), (24, 8));
    assert_eq!(plain_refs(outer), [8]);

    // The tuple field type gets its own layout too.
    let pair_layout = by_name("(String, Int)");
    assert_eq!((pair_layout.size, pair_layout.align), (16, 8));
    assert_eq!(plain_refs(pair_layout), [0]);

    // (Boolean, Int): Int is 4-aligned, so it sits at offset 4 and
    // the size rounds up to 8.
    let padded = by_name("(Boolean, Int)");
    assert_eq!((padded.size, padded.align), (8, 4));
    assert!(plain_refs(padded).is_empty());
}

#[test]
fn compiler_pointer_element_offsets_keep_their_domain_and_dedicated_stride() {
    let mut b = Builder::new();
    let pointer_ty = mir::Type::Ptr(Box::new(INT));
    let mut locals = Arena::new();
    let pointer = locals.alloc(local("pointer", pointer_ty.clone()));
    let displaced = locals.alloc(local("displaced", pointer_ty.clone()));
    let main = b.main(
        locals,
        vec![
            val_decl(
                pointer,
                expr(
                    pointer_ty.clone(),
                    mir::ExprKind::PtrFromNonZeroULong {
                        operand: Box::new(ulong_expr(0)),
                        pointee: Box::new(INT),
                    },
                ),
            ),
            val_decl(
                displaced,
                expr(
                    pointer_ty.clone(),
                    mir::ExprKind::PtrOffset {
                        pointer: Box::new(local_expr(pointer, pointer_ty)),
                        pointee: Box::new(INT),
                        offset: Box::new(mir::Expr::machine_scalar(
                            mir::MachineScalarValue::PointerElementOffset(2),
                        )),
                        subtract: false,
                    },
                ),
            ),
        ],
    );
    let module = lower(&b.finish(main));
    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::PtrOffset {
        element_offset,
        element_size,
        subtract,
        ..
    } = instructions[2]
    else {
        panic!("the pointer displacement must stay a dedicated instruction")
    };
    assert_eq!(*element_size, 4);
    assert!(!subtract);
    assert_eq!(
        function.value_ty(&module.globals, *element_offset),
        lir::LirType::MachineScalar(lir::MachineScalarKind::PointerElementOffset)
    );
    let dump = lir::dump(&module);
    assert!(dump.contains(
        "element-offset=machine<pointer-element-offset>(PointerElementOffset(2)) element-size=4"
    ));
}

#[test]
#[should_panic(expected = "PtrLoad result must match its pointee")]
fn pointer_load_cannot_relabel_a_machine_pointee_as_source_integer() {
    let mut b = Builder::new();
    let machine = mir::Type::MachineScalar(mir::MachineScalarKind::EnumTag);
    let pointer_ty = mir::Type::Ptr(Box::new(machine.clone()));
    let mut locals = Arena::new();
    let pointer = locals.alloc(local("pointer", pointer_ty.clone()));
    let integer = locals.alloc(local("integer", INT));
    let main = b.main(
        locals,
        vec![val_decl(
            integer,
            expr(
                INT,
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(local_expr(pointer, pointer_ty)),
                    pointee: Box::new(machine),
                    offset: None,
                },
            ),
        )],
    );

    let _ = lower(&b.finish(main));
}

#[test]
#[should_panic(expected = "PtrStore value must match its pointee")]
fn pointer_store_cannot_write_a_source_integer_as_a_machine_pointee() {
    let mut b = Builder::new();
    let machine = mir::Type::MachineScalar(mir::MachineScalarKind::EnumTag);
    let pointer_ty = mir::Type::Ptr(Box::new(machine.clone()));
    let mut locals = Arena::new();
    let pointer = locals.alloc(local("pointer", pointer_ty.clone()));
    let main = b.main(
        locals,
        vec![stmt(mir::StatementKind::Expr(expr(
            mir::Type::Unit,
            mir::ExprKind::PtrStore {
                pointer: Box::new(local_expr(pointer, pointer_ty)),
                pointee: Box::new(machine),
                offset: None,
                value: Box::new(int_expr(1)),
            },
        )))],
    );

    let _ = lower(&b.finish(main));
}
