use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    PackagePath, PersistentCallableBodyId, PersistentFunctionId, PersistentSafepointSiteId,
    SafepointId, SafepointSiteKey, SafepointSiteRole, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};
use scoop_wire::encode_runtime;

use super::*;

#[test]
fn amd64_roots_exclude_the_frame_record_red_zone_and_unaligned_slots() {
    let expected = semantic_plan(1);
    let record = |register, offset| {
        let slot = ProvisionalLlvmStackmapLocationV3::new(LOCATION_INDIRECT, 8, register, offset);
        let mut value = provisional(
            expected,
            vec![constant(8, 0), constant(8, 0), constant(8, 0), slot, slot],
            Vec::new(),
        );
        value.header.stack_size = 24;
        value
    };
    for (register, offset) in [(7, 0), (7, 8), (6, -16), (6, -8)] {
        normalize_record(
            StackmapArchitecture::X86_64,
            expected,
            &[],
            record(register, offset),
        )
        .unwrap();
    }
    for (register, offset) in [(7, -8), (7, 1), (7, 16), (6, 0), (6, 8), (6, -24)] {
        assert!(matches!(
            normalize_record(
                StackmapArchitecture::X86_64,
                expected,
                &[],
                record(register, offset)
            ),
            Err(StackmapNormalizationError::RootOutsideFrame { .. })
        ));
    }
    assert_eq!(
        normalize_record(StackmapArchitecture::X86_64, expected, &[], record(31, 0)),
        Err(StackmapNormalizationError::InvalidRootLocation(0))
    );
    for size in [0, 16, 32, u64::MAX] {
        let mut value = record(7, 0);
        value.header.stack_size = size;
        assert_eq!(
            normalize_record(StackmapArchitecture::X86_64, expected, &[], value),
            Err(StackmapNormalizationError::InvalidStackSize(size))
        );
    }
}

#[test]
fn constant_and_constant_index_normalize_to_the_same_record() {
    let expected = semantic_plan(1);
    let direct = provisional(
        expected,
        vec![
            constant(8, 0),
            constant(8, 0),
            constant(8, 0),
            root(16),
            root(16),
        ],
        Vec::new(),
    );
    let indexed = provisional(
        expected,
        vec![
            constant(8, 0),
            ProvisionalLlvmStackmapLocationV3::new(LOCATION_CONSTANT_INDEX, 8, 0, 1),
            constant(8, 0),
            root(16),
            root(16),
        ],
        Vec::new(),
    );

    let direct =
        normalize_record(StackmapArchitecture::Aarch64, expected, &[0xfeed], direct).unwrap();
    let indexed = normalize_record(
        StackmapArchitecture::Aarch64,
        expected,
        &[0xfeed, 0],
        indexed,
    )
    .unwrap();

    assert_eq!(direct, indexed);
    assert_eq!(direct.canonical().format_version(), 3);
    assert_eq!(direct.canonical().root_pair_count(), 1);
    assert_eq!(direct.canonical().locations().len(), 5);
    assert_eq!(
        direct.fingerprint().to_string(),
        "e434fed3e095642b7616438c30070458289ea15f5c74e86b4617e4a69cd53f6f"
    );
    assert_eq!(
        hex(&encode_runtime(direct.canonical()).unwrap()),
        concat!(
            "03000000",
            "24662ec09c17a05613852c3c83a7b291e4bd7886f402f146541333fd5909d75b",
            "7dd7fe7fb2852b5e04cca6bba5d951dea84d4751d554a38f715cf043dea0e83a",
            "17a3518c3eee2e33",
            "01000000",
            "01000000",
            "08000000",
            "4000000000000000",
            "0500000000000000",
            "04000000080000000000000000000000",
            "04000000080000000000000000000000",
            "04000000080000000000000000000000",
            "03000000080000001f0000001000000000000000",
            "03000000080000001f0000001000000000000000",
            "0000000000000000",
        )
    );
}

#[test]
fn canonical_fingerprint_preserves_instruction_stack_and_live_out_fields() {
    let expected = semantic_plan(0);
    let baseline = normalize_record(
        StackmapArchitecture::Aarch64,
        expected,
        &[],
        provisional(
            expected,
            vec![constant(8, 0), constant(8, 0), constant(8, 0)],
            vec![ProvisionalLlvmStackmapLiveOutV3::new(19, 8)],
        ),
    )
    .unwrap();
    let mut changed = provisional(
        expected,
        vec![constant(8, 0), constant(8, 0), constant(8, 0)],
        vec![ProvisionalLlvmStackmapLiveOutV3::new(20, 8)],
    );
    changed.header.instruction_offset = 12;
    changed.header.stack_size = 80;
    let changed = normalize_record(StackmapArchitecture::Aarch64, expected, &[], changed).unwrap();

    assert_ne!(baseline.fingerprint(), changed.fingerprint());
    assert_eq!(changed.canonical().instruction_offset(), 12);
    assert_eq!(changed.canonical().stack_size(), 80);
    assert_eq!(changed.canonical().live_outs()[0].dwarf_register(), 20);
    assert_eq!(changed.canonical().live_outs()[0].size(), 8);
}

#[test]
fn rejects_identity_header_and_count_mismatches() {
    let expected = semantic_plan(0);
    let valid_locations = || vec![constant(8, 0), constant(8, 0), constant(8, 0)];

    let mut wrong_id = provisional(expected, valid_locations(), Vec::new());
    wrong_id.header.safepoint_id += 1;
    assert!(matches!(
        normalize_without_constants(expected, wrong_id),
        Err(StackmapNormalizationError::SafepointIdMismatch { .. })
    ));

    let mut wrong_owner = provisional(expected, valid_locations(), Vec::new());
    wrong_owner.header.owner = callable_body("other");
    assert!(matches!(
        normalize_without_constants(expected, wrong_owner),
        Err(StackmapNormalizationError::OwnerMismatch { .. })
    ));

    let mut flags = provisional(expected, valid_locations(), Vec::new());
    flags.header.flags = 1;
    assert_eq!(
        normalize_without_constants(expected, flags),
        Err(StackmapNormalizationError::NonZeroRecordFlags(1))
    );

    let bad_header = provisional(
        expected,
        vec![constant(8, 0), constant(8, 1), constant(8, 0)],
        Vec::new(),
    );
    assert_eq!(
        normalize_without_constants(expected, bad_header),
        Err(StackmapNormalizationError::NonZeroStatepointFlags(1))
    );

    let missing = provisional(expected, vec![constant(8, 0), constant(8, 0)], Vec::new());
    assert_eq!(
        normalize_without_constants(expected, missing),
        Err(StackmapNormalizationError::LocationCountMismatch {
            expected: 3,
            actual: 2,
        })
    );

    let mut deopt = provisional(expected, valid_locations(), Vec::new());
    deopt.locations[2] = constant(8, 1);
    assert_eq!(
        normalize_without_constants(expected, deopt),
        Err(StackmapNormalizationError::NonZeroDeoptCount(1))
    );

    for stack_size in [0, 15, 24, u64::MAX] {
        let mut invalid_stack = provisional(expected, valid_locations(), Vec::new());
        invalid_stack.header.stack_size = stack_size;
        assert_eq!(
            normalize_without_constants(expected, invalid_stack),
            Err(StackmapNormalizationError::InvalidStackSize(stack_size))
        );
    }
}

#[test]
fn rejects_noncanonical_locations_and_invalid_root_slots() {
    let expected = semantic_plan(1);
    let headers = || vec![constant(8, 0), constant(8, 0), constant(8, 0)];

    let mut bad_index = headers();
    bad_index.extend([root(16), root(16)]);
    bad_index[0] = ProvisionalLlvmStackmapLocationV3::new(LOCATION_CONSTANT_INDEX, 8, 0, -1);
    assert!(matches!(
        normalize_without_constants(expected, provisional(expected, bad_index, Vec::new())),
        Err(StackmapNormalizationError::InvalidConstantPoolIndex { .. })
    ));

    let mut distinct = headers();
    distinct.extend([root(16), root(24)]);
    assert_eq!(
        normalize_without_constants(expected, provisional(expected, distinct, Vec::new())),
        Err(StackmapNormalizationError::DistinctRootPair(0))
    );

    let mut register_root = headers();
    register_root.extend([
        ProvisionalLlvmStackmapLocationV3::new(LOCATION_REGISTER, 8, 31, 0),
        ProvisionalLlvmStackmapLocationV3::new(LOCATION_REGISTER, 8, 31, 0),
    ]);
    assert_eq!(
        normalize_without_constants(expected, provisional(expected, register_root, Vec::new())),
        Err(StackmapNormalizationError::InvalidRootLocation(0))
    );

    let mut outside = headers();
    outside.extend([root(-8), root(-8)]);
    assert!(matches!(
        normalize_without_constants(expected, provisional(expected, outside, Vec::new())),
        Err(StackmapNormalizationError::RootOutsideFrame { .. })
    ));
}

#[test]
fn rejects_malformed_location_encodings_before_profile_promotion() {
    let expected = semantic_plan(0);
    let mut unknown = provisional(
        expected,
        vec![constant(8, 0), constant(8, 0), constant(8, 0)],
        Vec::new(),
    );
    unknown.locations[0] = ProvisionalLlvmStackmapLocationV3::new(6, 8, 0, 0);
    assert_eq!(
        normalize_without_constants(expected, unknown),
        Err(StackmapNormalizationError::UnknownLocationKind { index: 0, kind: 6 })
    );

    let mut register_offset = provisional(
        expected,
        vec![constant(8, 0), constant(8, 0), constant(8, 0)],
        Vec::new(),
    );
    register_offset.locations[0] =
        ProvisionalLlvmStackmapLocationV3::new(LOCATION_REGISTER, 8, 0, 1);
    assert_eq!(
        normalize_without_constants(expected, register_offset),
        Err(StackmapNormalizationError::NonZeroRegisterOffset {
            index: 0,
            offset: 1,
        })
    );

    let mut constant_register = provisional(
        expected,
        vec![constant(8, 0), constant(8, 0), constant(8, 0)],
        Vec::new(),
    );
    constant_register.locations[0] =
        ProvisionalLlvmStackmapLocationV3::new(LOCATION_CONSTANT, 8, 1, 0);
    assert_eq!(
        normalize_without_constants(expected, constant_register),
        Err(StackmapNormalizationError::NonZeroConstantRegister {
            index: 0,
            dwarf_register: 1,
        })
    );

    let mut nonconstant_header = provisional(
        expected,
        vec![constant(8, 0), constant(8, 0), constant(8, 0)],
        Vec::new(),
    );
    nonconstant_header.locations[0] =
        ProvisionalLlvmStackmapLocationV3::new(LOCATION_DIRECT, 8, 31, 0);
    assert_eq!(
        normalize_without_constants(expected, nonconstant_header),
        Err(StackmapNormalizationError::InvalidHeaderLocation { index: 0 })
    );
}

fn provisional(
    expected: ExpectedStackmapSemanticsV1,
    locations: Vec<ProvisionalLlvmStackmapLocationV3>,
    live_outs: Vec<ProvisionalLlvmStackmapLiveOutV3>,
) -> ProvisionalLlvmStackmapRecordV3 {
    ProvisionalLlvmStackmapRecordV3::new(
        ProvisionalLlvmStackmapRecordHeaderV3::new(expected.safepoint_id, expected.owner, 8, 64, 0),
        locations,
        live_outs,
    )
}

fn normalize_without_constants(
    expected: ExpectedStackmapSemanticsV1,
    provisional: ProvisionalLlvmStackmapRecordV3,
) -> Result<VerifiedNormalizedStackmapRecordV1, StackmapNormalizationError> {
    normalize_record(StackmapArchitecture::Aarch64, expected, &[], provisional)
}

fn constant(size: u16, value: i32) -> ProvisionalLlvmStackmapLocationV3 {
    ProvisionalLlvmStackmapLocationV3::new(LOCATION_CONSTANT, size, 0, value)
}

fn root(offset: i32) -> ProvisionalLlvmStackmapLocationV3 {
    ProvisionalLlvmStackmapLocationV3::new(LOCATION_INDIRECT, 8, 31, offset)
}

fn semantic_plan(root_pair_count: u32) -> ExpectedStackmapSemanticsV1 {
    let owner = callable_body("siteOwner");
    let role = SafepointSiteRole::ManagedPoll;
    let site = PersistentSafepointSiteId::from_key(&SafepointSiteKey::new(owner, role, 0)).unwrap();
    ExpectedStackmapSemanticsV1 {
        site,
        safepoint_id: SafepointId::derive(site).unwrap().get(),
        owner,
        role,
        root_pair_count,
    }
}

fn callable_body(name: &str) -> PersistentCallableBodyId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function),
    ))
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
