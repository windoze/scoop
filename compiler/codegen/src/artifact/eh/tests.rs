use super::frame::parse_eh_frame;
use super::lsda::parse_lsda;
use super::*;
use crate::artifact::ObservedSafepoint;

const PROFILE: LsdaEncodingProfile = LsdaEncodingProfile {
    lp_start: 0xff,
    type_table: 0x9b,
    call_site: 0x01,
};

// LLVM 22.1 Darwin/AArch64 output for one function with catch-all invokes
// and a cleanup invoke inside the active handler.
const CATCH_AND_CLEANUP: &[u8] = &[
    0xff, 0x9b, 0x29, 0x01, 0x21, 0x00, 0x34, 0x00, 0x00, 0x34, 0x04, 0x80, 0x01, 0x01, 0x38, 0x24,
    0x00, 0x00, 0x5c, 0x04, 0x80, 0x01, 0x01, 0x60, 0x4c, 0x00, 0x00, 0xac, 0x01, 0x04, 0xc8, 0x01,
    0x00, 0xb0, 0x01, 0x60, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const EH_FRAME: &[u8] = &[
    0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, b'z', b'P', b'L', b'R', 0x00, 0x01, 0x78,
    0x1e, 0x07, 0x9b, 0xed, 0xff, 0xff, 0xff, 0x10, 0x10, 0x0c, 0x1f, 0x00, 0x28, 0x00, 0x00, 0x00,
    0x20, 0x00, 0x00, 0x00, 0xdc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x10, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x08, 0xcb, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x4c, 0x0c, 0x1d,
    0x10, 0x9e, 0x01, 0x9d, 0x02, 0x00, 0x00, 0x00,
];

// LLVM 22.1 Darwin/AArch64 output for two callback bridges whose frames
// cannot be encoded in __compact_unwind. This is ordinary DWARF CFI: the CIE
// has no personality or LSDA augmentation, and each FDE augmentation is empty.
const UNWIND_ONLY_EH_FRAME: &[u8] = &[
    0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, b'z', b'R', 0x00, 0x01, 0x78, 0x1e, 0x01,
    0x10, 0x0c, 0x1f, 0x00, 0x20, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0xe4, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x0e, 0x20,
    0x9e, 0x01, 0x9d, 0x02, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x00, 0x00,
    0xc0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xb0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x48, 0x0e, 0x50, 0x9e, 0x01, 0x9d, 0x02,
];

fn relocation(
    offset: u64,
    r_type: u8,
    r_pcrel: bool,
    r_length: u8,
    symbol: &str,
    symbol_address: u64,
) -> EhRelocation {
    EhRelocation {
        offset,
        r_type,
        r_pcrel,
        r_length,
        symbol: symbol.to_string(),
        symbol_address,
        symbol_section: match symbol {
            "_scoop_eh_personality" => None,
            "ltmp.eh_frame" | "ltmp.unwind_frame" => Some("__eh_frame".to_string()),
            "_scoop.eh_test" | "_scoop_callback_bridge_0" | "_scoop_callback_bridge_1" => {
                Some("__text".to_string())
            }
            _ => Some("__gcc_except_tab".to_string()),
        },
        symbol_is_undefined: symbol == "_scoop_eh_personality",
        symbol_is_local: matches!(
            symbol,
            "ltmp.eh_frame" | "ltmp.unwind_frame" | "GCC_except_table"
        ),
        symbol_is_text: matches!(
            symbol,
            "_scoop.eh_test" | "_scoop_callback_bridge_0" | "_scoop_callback_bridge_1"
        ),
    }
}

fn eh_frame() -> EhSection {
    EhSection {
        address: 0x380,
        bytes: EH_FRAME.to_vec(),
        relocations: vec![
            relocation(0x13, 7, true, 2, "_scoop_eh_personality", 0),
            relocation(0x24, 1, false, 3, "ltmp.eh_frame", 0x380),
            relocation(0x24, 0, false, 3, "_scoop.eh_test", 0x24),
            relocation(0x35, 1, false, 3, "ltmp.eh_frame", 0x380),
            relocation(0x35, 0, false, 3, "GCC_except_table", 0x134),
        ],
    }
}

fn unwind_only_eh_frame() -> EhSection {
    EhSection {
        address: 0x7540,
        bytes: UNWIND_ONLY_EH_FRAME.to_vec(),
        relocations: vec![
            relocation(0x1c, 1, false, 3, "ltmp.unwind_frame", 0x7540),
            relocation(0x1c, 0, false, 3, "_scoop_callback_bridge_0", 0x3414),
            relocation(0x40, 1, false, 3, "ltmp.unwind_frame", 0x7540),
            relocation(0x40, 0, false, 3, "_scoop_callback_bridge_1", 0x343c),
        ],
    }
}

fn mixed_eh_frame() -> EhSection {
    let mut frame = eh_frame();
    let shift = frame.bytes.len();
    let mut unwind_only = UNWIND_ONLY_EH_FRAME.to_vec();
    for field in [0x1c, 0x40] {
        let shifted_field = shift + field;
        let raw = i64::try_from(shifted_field)
            .expect("test frame offset fits i64")
            .checked_neg()
            .expect("test frame offset is positive");
        unwind_only[field..field + 8].copy_from_slice(&raw.to_le_bytes());
    }
    frame.bytes.extend(unwind_only);
    let shift = u64::try_from(shift).expect("test frame length fits u64");
    frame.relocations.extend([
        relocation(shift + 0x1c, 1, false, 3, "ltmp.eh_frame", 0x380),
        relocation(shift + 0x1c, 0, false, 3, "_scoop_callback_bridge_0", 0x140),
        relocation(shift + 0x40, 1, false, 3, "ltmp.eh_frame", 0x380),
        relocation(shift + 0x40, 0, false, 3, "_scoop_callback_bridge_1", 0x168),
    ]);
    frame
}

fn expected_eh() -> ExpectedEh {
    let mut function = ExpectedEhFunction::default();
    function.insert(EhActionKind::CatchAll, None, "catch invoke 1".to_string());
    function.insert(EhActionKind::CatchAll, None, "catch invoke 2".to_string());
    function.insert(EhActionKind::Cleanup, None, "cleanup invoke".to_string());
    ExpectedEh {
        functions: BTreeMap::from([("scoop.eh_test".to_string(), function)]),
    }
}

fn text_with_protected_calls() -> TextSection {
    let mut bytes = vec![0; 0x134];
    for offset in [0x24 + 0x34, 0x24 + 0x5c, 0x24 + 0xac] {
        bytes[offset..offset + 4].copy_from_slice(&0x9400_0000u32.to_le_bytes());
    }
    TextSection { address: 0, bytes }
}

#[test]
fn closed_lsda_accepts_catch_all_and_cleanup() {
    let observed = parse_lsda(CATCH_AND_CLEANUP, 0x110, PROFILE).expect("closed LSDA");
    assert_eq!(
        observed.actions,
        BTreeSet::from([EhActionKind::Cleanup, EhActionKind::CatchAll])
    );
}

#[test]
fn closed_lsda_rejects_truncated_and_overflowing_headers() {
    let truncated = &CATCH_AND_CLEANUP[..2];
    assert!(parse_lsda(truncated, 0x110, PROFILE).is_err());
    let overflowing = [
        0xff, 0x9b, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
    ];
    assert!(parse_lsda(&overflowing, 0x110, PROFILE).is_err());
}

#[test]
fn closed_lsda_rejects_unknown_encodings() {
    for (index, replacement) in [(0, 0x00), (1, 0x03), (3, 0x03)] {
        let mut bytes = CATCH_AND_CLEANUP.to_vec();
        bytes[index] = replacement;
        assert!(
            parse_lsda(&bytes, 0x110, PROFILE).is_err(),
            "encoding byte {index} was accepted"
        );
    }
}

#[test]
fn cleanup_only_lsda_omits_the_type_table_and_all_actions() {
    // LLVM emits no TType offset in this form. The final bytes align the
    // next exception table and are not a null catch-all type entry.
    let bytes = [0xff, 0xff, 0x01, 8, 0, 4, 16, 0, 4, 4, 0, 0, 0, 0];
    let observed = parse_lsda(&bytes, 32, PROFILE).expect("cleanup-only LSDA");
    assert_eq!(observed.actions, BTreeSet::from([EhActionKind::Cleanup]));
    assert_eq!(observed.protected_ranges.len(), 1);
    assert_eq!(observed.protected_ranges[0].range, 0..4);

    let mut action_without_type = bytes;
    action_without_type[7] = 1;
    assert!(parse_lsda(&action_without_type, 32, PROFILE).is_err());
    assert!(parse_lsda(&bytes[..11], 32, PROFILE).is_err());
    let mut trailing_action = bytes;
    trailing_action[12] = 1;
    assert!(parse_lsda(&trailing_action, 32, PROFILE).is_err());
}

#[test]
fn closed_lsda_rejects_overlapping_ranges_and_bad_landing_pads() {
    let mut overlapping = CATCH_AND_CLEANUP.to_vec();
    overlapping[9] = 0x20;
    assert!(parse_lsda(&overlapping, 0x110, PROFILE).is_err());

    let mut outside = CATCH_AND_CLEANUP.to_vec();
    outside[11] = 0x90;
    outside[12] = 0x02;
    assert!(parse_lsda(&outside, 0x110, PROFILE).is_err());
}

#[test]
fn closed_lsda_rejects_nonterminal_or_typed_actions() {
    for type_filter in [0x02, 0x7f] {
        let mut typed_or_filtered = CATCH_AND_CLEANUP.to_vec();
        typed_or_filtered[38] = type_filter;
        assert!(parse_lsda(&typed_or_filtered, 0x110, PROFILE).is_err());
    }

    let mut chained = CATCH_AND_CLEANUP.to_vec();
    chained[39] = 0x01;
    assert!(parse_lsda(&chained, 0x110, PROFILE).is_err());

    let mut out_of_range = CATCH_AND_CLEANUP.to_vec();
    out_of_range[13] = 0x7f;
    assert!(parse_lsda(&out_of_range, 0x110, PROFILE).is_err());
}

#[test]
fn closed_lsda_rejects_non_null_type_entry() {
    let mut typed_entry = CATCH_AND_CLEANUP.to_vec();
    typed_entry[40] = 1;
    assert!(parse_lsda(&typed_entry, 0x110, PROFILE).is_err());

    let mut extra_null_entry = CATCH_AND_CLEANUP.to_vec();
    extra_null_entry.splice(40..40, [0, 0, 0, 0]);
    extra_null_entry[2] += 4;
    assert!(parse_lsda(&extra_null_entry, 0x110, PROFILE).is_err());
}

#[test]
fn eh_frame_associates_personality_function_range_and_lsda() {
    let frame = eh_frame();
    let gcc = EhSection {
        address: 0x134,
        bytes: CATCH_AND_CLEANUP.to_vec(),
        relocations: Vec::new(),
    };
    verify_sections(
        Some(&frame),
        Some(&gcc),
        Some(&text_with_protected_calls()),
        &expected_eh(),
        &ObservedSafepoints::default(),
        PROFILE,
    )
    .expect("qualified CIE/FDE/LSDA graph");
}

#[test]
fn eh_graph_rejects_unassociated_lsda_prefix() {
    let frame = eh_frame();
    let mut bytes = vec![0; 4];
    bytes.extend_from_slice(CATCH_AND_CLEANUP);
    let gcc = EhSection {
        address: 0x130,
        bytes,
        relocations: Vec::new(),
    };
    assert!(
        verify_sections(
            Some(&frame),
            Some(&gcc),
            Some(&text_with_protected_calls()),
            &expected_eh(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );
}

#[test]
fn eh_graph_rejects_missing_lir_invoke_coverage() {
    let frame = eh_frame();
    let gcc = EhSection {
        address: 0x134,
        bytes: CATCH_AND_CLEANUP.to_vec(),
        relocations: Vec::new(),
    };
    let mut expected = expected_eh();
    expected
        .functions
        .get_mut("scoop.eh_test")
        .expect("test function")
        .insert(EhActionKind::CatchAll, None, "missing invoke".to_string());
    assert!(
        verify_sections(
            Some(&frame),
            Some(&gcc),
            Some(&text_with_protected_calls()),
            &expected,
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );
}

#[test]
fn eh_frame_rejects_wrong_personality_flags_and_reversed_pairs() {
    let mut wrong_personality = eh_frame();
    wrong_personality.relocations[0].r_pcrel = false;
    assert!(parse_eh_frame(&wrong_personality).is_err());

    let mut reversed = eh_frame();
    reversed.relocations.swap(1, 2);
    assert!(parse_eh_frame(&reversed).is_err());

    let mut non_text_function = eh_frame();
    non_text_function.relocations[2].symbol_is_text = false;
    assert!(parse_eh_frame(&non_text_function).is_err());
}

#[test]
fn eh_frame_rejects_truncation_and_bad_cie_references() {
    let mut truncated = eh_frame();
    truncated.bytes.pop();
    assert!(parse_eh_frame(&truncated).is_err());

    let mut bad_cie = eh_frame();
    bad_cie.bytes[32..36].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
    assert!(parse_eh_frame(&bad_cie).is_err());
}

#[test]
fn protected_lsda_ranges_must_contain_aarch64_calls() {
    let frame = eh_frame();
    let gcc = EhSection {
        address: 0x134,
        bytes: CATCH_AND_CLEANUP.to_vec(),
        relocations: Vec::new(),
    };
    let text = TextSection {
        address: 0,
        bytes: vec![0; 0x134],
    };
    assert!(
        verify_sections(
            Some(&frame),
            Some(&gcc),
            Some(&text),
            &expected_eh(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );
}

#[test]
fn function_symbols_and_managed_invoke_identities_are_exact() {
    let gcc = EhSection {
        address: 0x134,
        bytes: CATCH_AND_CLEANUP.to_vec(),
        relocations: Vec::new(),
    };

    let mut unprefixed = eh_frame();
    unprefixed.relocations[2].symbol = "scoop.eh_test".to_string();
    assert!(
        verify_sections(
            Some(&unprefixed),
            Some(&gcc),
            Some(&text_with_protected_calls()),
            &expected_eh(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );

    let mut unaligned = eh_frame();
    unaligned.relocations[2].symbol_address = 0x25;
    assert!(
        verify_sections(
            Some(&unaligned),
            Some(&gcc),
            Some(&text_with_protected_calls()),
            &expected_eh(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );

    let mut expected = expected_eh();
    for (invoke, id) in expected
        .functions
        .get_mut("scoop.eh_test")
        .expect("test function")
        .invokes
        .iter_mut()
        .zip([1, 2, 3])
    {
        invoke.safepoint = Some(id);
    }
    let observed = ObservedSafepoints {
        sites: BTreeMap::from([
            (
                1,
                ObservedSafepoint {
                    function_symbol: "_scoop.eh_test".to_string(),
                    call_pc: 0x58,
                },
            ),
            (
                2,
                ObservedSafepoint {
                    function_symbol: "_scoop.eh_test".to_string(),
                    call_pc: 0xd0,
                },
            ),
            (
                3,
                ObservedSafepoint {
                    function_symbol: "_scoop.eh_test".to_string(),
                    call_pc: 0x80,
                },
            ),
        ]),
    };
    assert!(
        verify_sections(
            Some(&eh_frame()),
            Some(&gcc),
            Some(&text_with_protected_calls()),
            &expected,
            &observed,
            PROFILE,
        )
        .is_err()
    );
}

#[test]
fn no_eh_functions_allow_absent_eh_sections() {
    verify_sections(
        None,
        None,
        None,
        &ExpectedEh::default(),
        &ObservedSafepoints::default(),
        PROFILE,
    )
    .expect("no EH sections are required without invoke unwind edges");
}

#[test]
fn no_eh_functions_allow_qualified_unwind_only_fdes() {
    let frame = unwind_only_eh_frame();
    let parsed = parse_eh_frame(&frame).expect("qualified unwind-only CFI");
    assert!(parsed.scoop_fdes.is_empty());
    assert_eq!(parsed.unwind_only_fdes.len(), 2);
    assert_eq!(
        parsed.unwind_only_fdes[0].function_symbol,
        "_scoop_callback_bridge_0"
    );
    verify_sections(
        Some(&frame),
        None,
        Some(&TextSection {
            address: 0,
            bytes: vec![0; 0x34ec],
        }),
        &ExpectedEh::default(),
        &ObservedSafepoints::default(),
        PROFILE,
    )
    .expect("ordinary unwind CFI is not a Scoop exception edge");
}

#[test]
fn scoop_eh_and_unwind_only_fdes_are_qualified_independently() {
    let frame = mixed_eh_frame();
    let parsed = parse_eh_frame(&frame).expect("qualified mixed CFI");
    assert_eq!(parsed.scoop_fdes.len(), 1);
    assert_eq!(parsed.unwind_only_fdes.len(), 2);

    let gcc = EhSection {
        address: 0x134,
        bytes: CATCH_AND_CLEANUP.to_vec(),
        relocations: Vec::new(),
    };
    let mut text = text_with_protected_calls();
    text.bytes.resize(0x218, 0);
    verify_sections(
        Some(&frame),
        Some(&gcc),
        Some(&text),
        &expected_eh(),
        &ObservedSafepoints::default(),
        PROFILE,
    )
    .expect("ordinary CFI does not change the complete-LIR Scoop EH manifest");
}

#[test]
fn unwind_only_cfi_still_rejects_unqualified_metadata() {
    let mut unsupported_cie = unwind_only_eh_frame();
    unsupported_cie.bytes[10] = b'P';
    assert!(parse_eh_frame(&unsupported_cie).is_err());

    let mut augmented_fde = unwind_only_eh_frame();
    augmented_fde.bytes[44] = 1;
    assert!(parse_eh_frame(&augmented_fde).is_err());

    let gcc = EhSection {
        address: 0x100,
        bytes: vec![0],
        relocations: Vec::new(),
    };
    assert!(
        verify_sections(
            Some(&unwind_only_eh_frame()),
            Some(&gcc),
            Some(&TextSection {
                address: 0,
                bytes: vec![0; 0x34ec],
            }),
            &ExpectedEh::default(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );

    assert!(
        verify_sections(
            Some(&eh_frame()),
            None,
            Some(&text_with_protected_calls()),
            &ExpectedEh::default(),
            &ObservedSafepoints::default(),
            PROFILE,
        )
        .is_err()
    );
}
