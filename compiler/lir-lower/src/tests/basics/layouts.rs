use super::*;

#[test]
fn struct_values_keep_named_lir_identity() {
    let mut b = Builder::new();
    let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Struct(point)));
    let x = locals.alloc(local("x", mir::Type::Int));
    let main = b.main(
        locals,
        vec![
            val_decl(
                p,
                expr(
                    mir::Type::Struct(point),
                    mir::ExprKind::StructInit {
                        struct_id: point,
                        args: vec![mir::Expr::int(1), mir::Expr::int(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    mir::Type::Int,
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
            lir::LirType::I64,
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
    assert_eq!(function.temps[*out].ty, lir::LirType::I64);
    assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

    // The struct layout is in the meta.
    let layout = layout_values(&module)
        .find(|l| l.name == "Point")
        .expect("a layout per struct");
    assert_eq!((layout.size, layout.align), (16, 8));
    assert!(plain_refs(layout).is_empty());
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
fn uint_shares_ints_machine_word() {
    // UInt (spec 11.2, M9) maps onto `i64` at LIR — the same
    // machine word as Int, so codegen needs no UInt-specific
    // handling.
    let mut b = Builder::new();
    // A UInt field in a class: an 8-byte scalar slot behind the
    // 16-byte header, exactly like an Int field.
    let _c = b.class("C", None, &[("u", mir::Type::UInt)], empty_vtable(), vec![]);
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::UInt));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::int(1))]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::I64);

    let c_layout = layout_values(&module)
        .find(|l| l.name == "C")
        .expect("a layout per class");
    assert_eq!((c_layout.size, c_layout.align), (24, 8));
    assert!(plain_refs(c_layout).is_empty());
    let c_td = descriptor_values(&module)
        .find(|td| td.name == "C")
        .expect("a TypeDescriptor per class");
    assert_eq!(c_td.size, 24);
    assert_eq!(*fixed_scan(c_td), lir::RefScan::None);
}

#[test]
fn layouts_mark_reference_fields_for_the_gc() {
    let mut b = Builder::new();
    // String field behind one Int: the reference sits at offset 8.
    let s = b.strukt("S", &[("a", mir::Type::Int), ("s", mir::Type::String)]);
    // A String nested inside a tuple field, after a Boolean: the
    // tuple is 8-aligned, so it starts at offset 8.
    let pair = mir::Type::Tuple(vec![mir::Type::String, mir::Type::Int]);
    let _outer = b.strukt("Outer", &[("flag", mir::Type::Boolean), ("pair", pair)]);
    let _ = s;
    // A tuple type that only appears in code (padding: Boolean
    // then Int at offset 8).
    let mut locals = Arena::new();
    let _t = locals.alloc(local(
        "t",
        mir::Type::Tuple(vec![mir::Type::Boolean, mir::Type::Int]),
    ));
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
            "Int",
            "UInt",
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

    // (Boolean, Int): Int is 8-aligned, so it sits at offset 8 and
    // the size rounds up to 16.
    let padded = by_name("(Boolean, Int)");
    assert_eq!((padded.size, padded.align), (16, 8));
    assert!(plain_refs(padded).is_empty());
}

#[test]
fn compiler_pointer_element_offsets_keep_their_domain_and_dedicated_stride() {
    let mut b = Builder::new();
    let pointer_ty = mir::Type::Ptr(Box::new(mir::Type::Int));
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
                    mir::ExprKind::PtrFromUInt {
                        operand: Box::new(expr(mir::Type::UInt, mir::ExprKind::IntLiteral(0))),
                        pointee: Box::new(mir::Type::Int),
                    },
                ),
            ),
            val_decl(
                displaced,
                expr(
                    pointer_ty.clone(),
                    mir::ExprKind::PtrOffset {
                        pointer: Box::new(local_expr(pointer, pointer_ty)),
                        pointee: Box::new(mir::Type::Int),
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
    assert_eq!(*element_size, 8);
    assert!(!subtract);
    assert_eq!(
        function.value_ty(&module.globals, *element_offset),
        lir::LirType::MachineScalar(lir::MachineScalarKind::PointerElementOffset)
    );
    let dump = lir::dump(&module);
    assert!(dump.contains(
        "element-offset=machine<pointer-element-offset>(PointerElementOffset(2)) element-size=8"
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
    let integer = locals.alloc(local("integer", mir::Type::Int));
    let main = b.main(
        locals,
        vec![val_decl(
            integer,
            expr(
                mir::Type::Int,
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
                value: Box::new(mir::Expr::int(1)),
            },
        )))],
    );

    let _ = lower(&b.finish(main));
}
