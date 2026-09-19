use super::*;

#[test]
fn native_abi_replays_c_field_geometry_before_recording_the_contract() {
    let mut builder = Builder::new();
    let id = builder.c_strukt(
        "Pair",
        mir::MirCLayoutValue::Natural,
        mir::MirCLayoutValue::Natural,
        false,
        &[("flag", mir::Type::Boolean), ("value", LONG)],
    );
    builder.c_extern(
        "consume",
        "consume",
        vec![mir::Type::Struct(id)],
        mir::Type::Unit,
    );
    let main = builder.main(Arena::new(), Vec::new());
    let module = builder.finish(main);
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);
    let enums = lower_enums(&context, &module).unwrap();
    let mut structs = lower_structs(&context, &module, &enums).unwrap();
    assert!(native_abi::lower(&context, &module, &structs, &enums).is_ok());
    let reference = structs.c_ref(struct_def_id(id)).unwrap();
    let mut fields = structs[reference.definition()].c_fields().unwrap().to_vec();
    fields[1].layout.offset = 1;
    structs.set_c_fields(reference, fields);
    assert!(matches!(
        native_abi::lower(&context, &module, &structs, &enums),
        Err(StorageLoweringError::InvalidRepresentation(
            "C field placement differs from checked layout replay"
        ))
    ));
}
