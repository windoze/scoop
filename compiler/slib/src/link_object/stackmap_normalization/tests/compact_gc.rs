use super::*;
use crate::link_object::compact_gc::codec;

fn record(
    name: &str,
    pc: u32,
    stack: u64,
    roots: &[(u16, i32)],
) -> VerifiedNormalizedStackmapRecordV1 {
    let owner = callable_body(name);
    let role = SafepointSiteRole::ManagedPoll;
    let site =
        PersistentSafepointSiteId::from_key(&SafepointSiteKey::new(owner, role, pc)).unwrap();
    let id = SafepointId::derive(site).unwrap().get();
    let expected = ExpectedStackmapSemanticsV1 {
        owner,
        site,
        role,
        safepoint_id: id,
        root_pair_count: roots.len() as u32,
    };
    let mut locations = vec![constant(8, 0); 3];
    for &(register, offset) in roots {
        let root = ProvisionalLlvmStackmapLocationV3::new(3, 8, register, offset);
        locations.extend([root, root]);
    }
    normalize_record(
        StackmapFrame::WindowsAmd64VirtualUnwind,
        expected,
        &[],
        ProvisionalLlvmStackmapRecordV3::new(
            ProvisionalLlvmStackmapRecordHeaderV3::new(id, owner, pc, stack, 0),
            locations,
            Vec::new(),
        ),
    )
    .unwrap()
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[test]
fn compact_gc_preserves_root_order_multiplicity_and_signed_offsets() {
    let roots = [(7, 16), (6, -8), (7, 16)];
    let records = [
        record("body", 20, 264, &[(6, i32::MIN), (6, i32::MAX & !7)]),
        record("body", 12, 264, &[(6, -8), (7, 16), (7, 16)]),
        record("body", 4, 264, &roots),
        record("body", 16, 264, &[]),
        record("body", 8, 264, &roots),
    ];
    let bytes = codec::encode(StackmapFrame::WindowsAmd64VirtualUnwind, records.iter()).unwrap();
    assert_eq!(bytes.len(), 160);
    assert_eq!(
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        0x5343_4f4f_5047_434d
    );
    assert_eq!(
        (word(&bytes, 8), word(&bytes, 12)),
        (scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1, 32)
    );
    assert_eq!((word(&bytes, 24), word(&bytes, 28)), (5, 12));
    for (index, (pc, layout)) in [(4, 0), (8, 0), (12, 4), (16, 8), (20, 9)]
        .into_iter()
        .enumerate()
    {
        let start = 32 + index * 16;
        let expected = records
            .iter()
            .find(|record| record.canonical().instruction_offset() == pc)
            .unwrap();
        assert_eq!(
            u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()),
            expected.canonical().safepoint_id()
        );
        assert_eq!(
            (word(&bytes, start + 8), word(&bytes, start + 12)),
            (pc, layout)
        );
    }
    assert_eq!(
        bytes[112..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>(),
        [
            3,
            16,
            (-8i32 as u32) | 1,
            16,
            3,
            (-8i32 as u32) | 1,
            16,
            16,
            0,
            2,
            0x8000_0001,
            0x7fff_fff9
        ]
    );
}

#[test]
fn compact_gc_requires_one_nonempty_callable_with_unique_pcs() {
    let a = record("a", 4, 264, &[]);
    let b = record("b", 8, 264, &[]);
    let c = record("a", 8, 280, &[]);
    for records in [vec![], vec![&a, &a], vec![&a, &b], vec![&a, &c]] {
        assert!(
            codec::encode(
                StackmapFrame::WindowsAmd64VirtualUnwind,
                records.into_iter()
            )
            .is_err()
        );
    }
    let bytes = codec::encode(StackmapFrame::WindowsAmd64VirtualUnwind, [&a].into_iter()).unwrap();
    assert_eq!(bytes.len(), 56);
    assert_eq!(&bytes[48..], &[0; 8]);
}
