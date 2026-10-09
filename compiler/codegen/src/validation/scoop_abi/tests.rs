mod pointer_values;
mod struct_zst;
mod tagged_zst;
use super::*;
use la_arena::Arena;

fn module_with_types(structs: StructDefs, enums: EnumDefs) -> Module {
    let mut local_functions = scoop_lir::LocalFunctionIdentities::default();
    Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs,
        enums,
        functions: Vec::new(),
        extern_functions: scoop_lir::ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: scoop_lir::NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: scoop_lir::LirOutput::Executable {
            entry: scoop_lir::LocalFunctionRef::Managed(local_functions.alloc_managed()),
        },
        meta: scoop_lir::LirMeta {
            exact_types: Vec::new(),
            target_profile: scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            canonical_c_abi: scoop_lir::CanonicalCAbiMetadata::default(),
            native_externals: scoop_lir::NativeExternalMetadata::default(),
            well_known_type_descriptors: scoop_lir::WellKnownTypeDescriptors {
                string: scoop_lir::TypeDescriptorRef::Local(scoop_lir::TypeDescriptorId::from_raw(
                    0.into(),
                )),
            },
            arrays: Arena::new(),
            layouts: Arena::new(),
            type_descriptors: Arena::new(),
            external_type_descriptors: Arena::new(),
            external_callables: Arena::new(),
        },
    }
}

fn abi_value(ty: LirType, size: u64, align: u64, scan: RefScan) -> scoop_lir::AbiValue {
    scoop_lir::AbiValue::new(
        ty,
        scoop_lir::AbiNonZeroLayout::new(size, align).expect("valid test ABI layout"),
        scan,
    )
    .expect("valid test ABI storage type")
}

#[test]
fn abi_value_layout_must_match_the_embedded_target_profile() {
    let module = module_with_types(StructDefs::default(), EnumDefs::default());
    let value = abi_value(LirType::I64, 16, 8, RefScan::None);
    let error = AbiMetadataValidator::new(&module)
        .validate_value(&value, "test value")
        .expect_err("wrong target size must be rejected");
    assert!(
        error.0.contains("size/alignment 16/8") && error.0.contains("expected 8/8"),
        "unexpected error: {error}"
    );
}

#[test]
fn struct_field_offsets_are_revalidated_before_accepting_an_abi_scan() {
    let mut structs = StructDefs::default();
    let id = structs.alloc_scoop(
        crate::tests::test_physical_exact("BadOffset", scoop_identity::SourceNominalKind::Struct),
        "BadOffset".to_string(),
        16,
        8,
        false,
        vec![scoop_lir::StructField {
            ty: scoop_lir::MANAGED_PTR,
            layout: scoop_lir::FieldLayout {
                offset: 8,
                access_align: 8,
            },
        }],
    );
    let module = module_with_types(structs, EnumDefs::default());
    let value = abi_value(LirType::Struct(id), 16, 8, RefScan::References(vec![8]));
    let error = AbiMetadataValidator::new(&module)
        .validate_value(&value, "test value")
        .expect_err("wrong struct field offset must be rejected");
    assert!(
        error.0.contains("field 0 layout 8/8") && error.0.contains("target layout 0/8"),
        "unexpected error: {error}"
    );
}

#[test]
fn enum_scan_must_match_its_variant_field_offsets() {
    let mut enums = EnumDefs::default();
    let id = enums.alloc(scoop_lir::EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "BadScan",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "BadScan".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![scoop_lir::EnumVariantRepr {
                fields: vec![scoop_lir::EnumFieldRepr {
                    ty: scoop_lir::MANAGED_PTR,
                    offset: 8,
                }],
                slot_offset: 8,
                slot_size: 8,
                slot_align: 8,
                gc_free: false,
            }],
            size: 16,
            align: 8,
        },
        scan: RefScan::References(vec![0]),
    });
    let module = module_with_types(StructDefs::default(), enums);
    let value = abi_value(LirType::Enum(id), 16, 8, RefScan::References(vec![0]));
    let error = AbiMetadataValidator::new(&module)
        .validate_value(&value, "test value")
        .expect_err("enum scan must come from its exact field offsets");
    assert!(
        error.0.contains("scan refs[0]") && error.0.contains("field-derived scan refs[8]"),
        "unexpected error: {error}"
    );
}

#[test]
fn aggregate_argument_cannot_claim_direct_scoop_passing() {
    let module = module_with_types(StructDefs::default(), EnumDefs::default());
    let argument = scoop_lir::AbiArgument::Direct(
        abi_value(
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
            16,
            8,
            RefScan::None,
        )
        .into(),
    );
    let error = AbiMetadataValidator::new(&module)
        .validate_arguments(&[argument], "test signature")
        .expect_err("aggregate direct passing must be rejected");
    assert!(
        error
            .0
            .contains("uses direct passing for incompatible {i64, i64} storage"),
        "unexpected error: {error}"
    );
}

#[test]
fn scalar_result_cannot_claim_indirect_scoop_passing() {
    let module = module_with_types(StructDefs::default(), EnumDefs::default());
    let signature = scoop_lir::ScoopAbiSignature::new(
        Vec::new(),
        scoop_lir::AbiReturn::Indirect(abi_value(LirType::I64, 8, 8, RefScan::None)),
        scoop_lir::CallingConvention::Cdecl,
    );
    let error = AbiMetadataValidator::new(&module)
        .validate_scoop_signature(&signature, "test signature")
        .expect_err("scalar indirect passing must be rejected");
    assert!(
        error
            .0
            .contains("uses indirect passing for incompatible i64 storage"),
        "unexpected error: {error}"
    );
}
