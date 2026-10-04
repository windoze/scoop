use super::*;

fn projected_handle(builder: &mut Builder) -> mir::StructId {
    let id = builder.strukt("ResourceToken", &[("raw", ULONG)]);
    let mir::StructRepresentation::Declared { c_abi, fields, .. } =
        &mut builder.structs[id].representation
    else {
        unreachable!()
    };
    *c_abi = mir::StructCAbi::UInt64Field {
        field: fields[0].identity,
    };
    id
}

#[test]
fn c_projection_produces_uint64_contracts_without_changing_the_scoop_struct() {
    let mut builder = Builder::new();
    let id = projected_handle(&mut builder);
    let ty = mir::Type::Struct(id);
    builder.c_extern("roundTrip", "round_trip", vec![ty.clone()], ty.clone());
    let main = builder.main(Arena::new(), Vec::new());
    let source = builder.finish(main);
    let exact = source
        .meta
        .source_exact_types
        .get(&ty)
        .unwrap()
        .identity_record()
        .id();
    let module = lower(source);
    let definition = &module.structs[struct_def_id(id)];
    assert_eq!((definition.size, definition.align), (8, 8));
    assert!(definition.c_layout().is_none());
    let (_, external) = module.extern_functions.iter().next().unwrap();
    let lir::ExternFunctionKind::C { signature, .. } = &external.kind else {
        unreachable!()
    };
    assert_eq!(
        signature.params,
        [lir::CType::Integer(lir::IntegerKind::UNSIGNED_64)]
    );
    assert_eq!(
        signature.return_type,
        lir::CReturnType::Value(Box::new(lir::CType::Integer(lir::IntegerKind::UNSIGNED_64)))
    );
    let signature = module.meta.canonical_c_abi.signatures()[0].signature();
    let expected = scoop_identity::CanonicalCStorageType::Integer {
        exact_type: exact,
        signedness: scoop_identity::Signedness::Unsigned,
        bit_width: scoop_identity::IntegerBitWidth::Bits64,
    };
    assert_eq!(signature.parameters()[0].storage(), expected);
    assert_eq!(
        signature.result(),
        scoop_identity::CanonicalCAbiReturn::value(exact, expected).unwrap()
    );
    let scoop = abi::classify_signature(
        &LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64),
        [lir::LirType::Struct(struct_def_id(id))],
        Some(lir::LirType::Struct(struct_def_id(id))),
        &module.structs,
        &module.enums,
    )
    .unwrap();
    assert!(scoop.arguments()[0].is_indirect());
    assert!(scoop.result().is_indirect());
}

#[test]
fn c_projection_composes_with_c_layout_fields_pointers_and_function_pointer_signatures() {
    let mut builder = Builder::new();
    let id = projected_handle(&mut builder);
    let handle = mir::Type::Struct(id);
    let packet = builder.c_strukt(
        "Packet",
        mir::MirCLayoutValue::Natural,
        mir::MirCLayoutValue::Natural,
        false,
        &[
            ("tag", mir::Type::Integer(mir::IntegerKind::UNSIGNED_8)),
            ("value", handle.clone()),
        ],
    );
    let signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![handle.clone()],
        return_type: handle.clone(),
    });
    let pointer = mir::Type::Ptr(Box::new(handle.clone()));
    let callback = mir::Type::FunPtr(signature);
    builder.c_extern(
        "exchange",
        "exchange",
        vec![mir::Type::Struct(packet), pointer.clone(), callback.clone()],
        handle,
    );
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    register_test_source_exact_type(&mut source, pointer);
    register_test_source_exact_type(&mut source, callback);
    let module = lower(source);
    let definition = &module.structs[struct_def_id(packet)];
    assert_eq!((definition.size, definition.align), (16, 8));
    assert_eq!(
        definition.c_fields().unwrap()[1].ty,
        lir::CType::Integer(lir::IntegerKind::UNSIGNED_64)
    );
    let (_, external) = module.extern_functions.iter().next().unwrap();
    let lir::ExternFunctionKind::C { signature, .. } = &external.kind else {
        unreachable!()
    };
    assert!(
        matches!(&signature.params[1], lir::CType::DataPointer { pointee: lir::CDataPointee::Object(ty), .. } if **ty == lir::CType::Integer(lir::IntegerKind::UNSIGNED_64))
    );
    assert!(
        matches!(&signature.params[2], lir::CType::CodePointer { signature, .. } if signature.params == [lir::CType::Integer(lir::IntegerKind::UNSIGNED_64)])
    );
    let dump = format!(
        "handle: Scoop=aggregate(8,8) C=uint64\npacket: size={} align={} handle-offset={}\npointer: uint64*\ncallback: uint64 -> uint64\n",
        definition.size,
        definition.align,
        definition.c_fields().unwrap()[1].layout.offset
    );
    assert_eq!(
        dump,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-native-boundary/handle-c-lir.snap"
        ))
    );
}
