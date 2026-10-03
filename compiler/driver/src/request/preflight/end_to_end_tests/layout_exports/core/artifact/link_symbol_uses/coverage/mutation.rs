//! Corrupt one schema-valid final projection, preserving its original bytes.

use super::*;

pub(super) fn capability(case: Case) -> scoop_identity::CapabilityId {
    match case {
        Case::Members(Surface::Identity, _) | Case::ImageField(_) | Case::EntryBranch => {
            slib::lir_link_identity_closure_capability()
        }
        Case::Members(Surface::Ordinary, _) | Case::Digest(Surface::Ordinary) => {
            slib::lir_cross_cone_link_closure_capability()
        }
        Case::Members(Surface::Shape, _) | Case::Digest(Surface::Shape) => {
            slib::lir_cross_cone_layout_link_closure_capability()
        }
        Case::Digest(Surface::Identity) => panic!("identity closure has no coverage digest"),
    }
}

pub(super) fn apply(bytes: &[u8], case: Case) -> Vec<u8> {
    let mut changed = bytes.to_vec();
    match case {
        Case::Members(surface, change) => {
            let range = match surface {
                Surface::Identity => wire::field_range(bytes, 6),
                Surface::Ordinary | Surface::Shape => nested(bytes, 3, 1),
            };
            let mut records = wire::array_parts(&bytes[range.clone()])
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>();
            assert!(records.len() >= 2);
            match change {
                Change::Missing => {
                    records.remove(0);
                }
                Change::Extra => {
                    let mut extra = records[0].clone();
                    *extra.last_mut().unwrap() ^= 1;
                    records.push(extra);
                }
                Change::Duplicate => records.insert(0, records[0].clone()),
                Change::Order => records.swap(0, 1),
                Change::Fingerprint => *records[0].last_mut().unwrap() ^= 1,
            }
            changed.splice(range, wire::encode_array_records(&records));
        }
        Case::Digest(_) => changed[nested(bytes, 3, 2).end - 1] ^= 1,
        Case::ImageField(field) => changed[nested(bytes, 7, field).end - 1] ^= 1,
        Case::EntryBranch => return entry_branch(bytes),
    }
    changed
}

fn nested(bytes: &[u8], outer: u64, inner: u64) -> std::ops::Range<usize> {
    let outer = wire::field_range(bytes, outer);
    let inner = wire::field_range(&bytes[outer.clone()], inner);
    (outer.start + inner.start)..(outer.start + inner.end)
}

fn entry_branch(bytes: &[u8]) -> Vec<u8> {
    let current = &bytes[wire::field_range(bytes, 8)];
    let library = [0xa1, 0, 1];
    if current != library {
        return wire::replace_field(bytes, 8, &library);
    }
    // Build the other branch using actual typed owner bytes. The projection
    // is schema-valid but a library must never acquire an executable owner.
    let mut executable = vec![0xa2, 0, 2, 1, 0xa3];
    for (field, source) in [(1, 1), (2, 2), (3, 5)] {
        executable.push(field);
        executable.extend_from_slice(&bytes[nested(bytes, 7, source)]);
    }
    wire::replace_field(bytes, 8, &executable)
}
