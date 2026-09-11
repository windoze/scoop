use super::*;

fn add_encoded_global(module: &mut Module, symbol: &str, ty: LirType, payload: LirConstantImage) {
    let identity = static_storage_identity(symbol);
    module.globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            identity,
            ty,
            initial_state: LirStaticInitialState::EncodedStaticValue { payload },
            thread_local: false,
        },
    });
}

fn encoded_global_symbol(name: &str) -> String {
    static_storage_identity(name).symbol().to_string()
}

fn constant_validation_error(module: &Module) -> CodegenError {
    crate::validation::validate_module(module)
        .expect_err("malformed constant image must fail module preflight")
}

#[test]
fn global_constant_preflight_rejects_an_inexact_root_type() {
    let mut module = enum_module();
    add_encoded_global(
        &mut module,
        "wrong_root_type",
        LirType::I64,
        LirConstantImage::Bool(false),
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value: Boolean constant does not match storage type i64",
            encoded_global_symbol("wrong_root_type")
        )
    );
}

#[test]
fn global_constant_preflight_recursively_rejects_an_inexact_struct_leaf() {
    let mut module = enum_module();
    let inner = module.structs.alloc_scoop(
        "Inner".to_string(),
        8,
        8,
        false,
        vec![scoop_lir::StructField {
            ty: LirType::I64,
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 8,
            },
        }],
    );
    let outer = module.structs.alloc_scoop(
        "Outer".to_string(),
        16,
        8,
        false,
        vec![
            scoop_lir::StructField {
                ty: LirType::Struct(inner),
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 8,
                },
            },
            scoop_lir::StructField {
                ty: LirType::I1,
                layout: scoop_lir::FieldLayout {
                    offset: 8,
                    access_align: 1,
                },
            },
        ],
    );
    add_encoded_global(
        &mut module,
        "nested_leaf",
        LirType::Struct(outer),
        LirConstantImage::Struct {
            struct_id: outer,
            fields: vec![
                LirConstantImage::Struct {
                    struct_id: inner,
                    fields: vec![LirConstantImage::Bool(false)],
                },
                LirConstantImage::Bool(true),
            ],
        },
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value.field[0].field[0]: Boolean constant does not match storage type i64",
            encoded_global_symbol("nested_leaf")
        )
    );
}

#[test]
fn global_constant_preflight_rejects_a_wrong_enum_unit_owner() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let option = module.enums.iter().nth(1).expect("Option enum").0;
    let dot = module.enums.variant_ref(shape, 0).expect("Dot variant");
    add_encoded_global(
        &mut module,
        "wrong_enum_owner",
        LirType::Enum(option),
        LirConstantImage::EnumUnit { variant: dot },
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value: enum unit constant for e0 does not match storage type enum1",
            encoded_global_symbol("wrong_enum_owner")
        )
    );
}

#[test]
fn global_constant_preflight_rejects_a_payload_variant_as_a_unit() {
    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let circle = module
        .enums
        .variant_ref(shape, 1)
        .expect("Circle payload variant");
    add_encoded_global(
        &mut module,
        "payload_as_unit",
        LirType::Enum(shape),
        LirConstantImage::EnumUnit { variant: circle },
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value: a payload enum variant cannot be encoded as a unit constant",
            encoded_global_symbol("payload_as_unit")
        )
    );
}

#[test]
fn global_constant_preflight_rejects_a_foreign_checked_variant_ref() {
    let mut foreign = scoop_lir::EnumDefs::default();
    let foreign_shape = foreign.alloc(EnumDef {
        name: "ForeignShape".to_string(),
        repr: EnumRepr::Tagged {
            variants: (0..4)
                .map(|_| EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                })
                .collect(),
            size: 8,
            align: 8,
        },
        scan: RefScan::None,
    });
    let foreign_variant = foreign
        .variant_ref(foreign_shape, 3)
        .expect("variant exists in foreign store only");

    let mut module = enum_module();
    let shape = module.enums.iter().next().expect("Shape enum").0;
    let wrapper = module.structs.alloc_scoop(
        "ShapeHolder".to_string(),
        32,
        8,
        false,
        vec![scoop_lir::StructField {
            ty: LirType::Enum(shape),
            layout: scoop_lir::FieldLayout {
                offset: 0,
                access_align: 8,
            },
        }],
    );
    add_encoded_global(
        &mut module,
        "foreign_enum_unit",
        LirType::Struct(wrapper),
        LirConstantImage::Struct {
            struct_id: wrapper,
            fields: vec![LirConstantImage::EnumUnit {
                variant: foreign_variant,
            }],
        },
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value.field[0] carries invalid enum0 variant 3 reference",
            encoded_global_symbol("foreign_enum_unit")
        )
    );
}

#[test]
fn global_constant_preflight_rejects_referenced_global_provenance_mismatch() {
    let mut module = enum_module();
    let target = module.globals.iter().next().expect("trap message").0;
    add_encoded_global(
        &mut module,
        "wrong_global_provenance",
        LirType::Ptr(PointerKind::Managed),
        LirConstantImage::GlobalPointer {
            global: target,
            kind: PointerKind::Managed,
        },
    );

    let error = constant_validation_error(&module);
    assert_eq!(
        error.0,
        format!(
            "storage global `@{}` constant value: global pointer constant declares managed provenance but referenced global `@scoop.trap.0` has raw provenance",
            encoded_global_symbol("wrong_global_provenance")
        )
    );
}
