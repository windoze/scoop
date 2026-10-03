//! Schema-valid candidates deliberately disagree with the original object inputs.

use super::*;

pub(super) fn production(bytes: &[u8], case: Case, seed: &[u8]) -> Vec<u8> {
    let (field, replacement) = match case {
        Case::Distribution => (
            1,
            encode(&slib::ArtifactDistributionClassV1::LocalExecutableRoot).unwrap(),
        ),
        Case::OutputBranch => (2, executable_branch()),
        Case::ManifestCode | Case::MatchingWrongCode => {
            let range = wire::field_range(bytes, 7);
            let mut fingerprint = bytes[range].to_vec();
            *fingerprint.last_mut().unwrap() ^= 1;
            (7, fingerprint)
        }
        Case::NativeContractExtra
        | Case::NativeContractMissing
        | Case::NativeContractDuplicate
        | Case::NativeContractOrder
        | Case::NativeContractFingerprint => {
            let range = wire::field_range(bytes, 8);
            let mut records = wire::array_parts(&bytes[range])
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>();
            match case {
                Case::NativeContractExtra => records.push(seed.to_vec()),
                Case::NativeContractMissing => {
                    records.remove(0);
                }
                Case::NativeContractDuplicate => records.insert(0, records[0].clone()),
                Case::NativeContractOrder => records.swap(0, 1),
                Case::NativeContractFingerprint => {
                    let range = wire::field_range(&records[0], 3);
                    records[0][range.end - 1] ^= 1;
                }
                _ => unreachable!(),
            }
            (8, wire::encode_array_records(&records))
        }
        Case::NativeLibraryExtra => {
            let record = scoop_identity::CborIdentityRecord::from_key(
                scoop_identity::NativeLinkRequirementKey::target_default(
                    scoop_identity::CanonicalNativeLibraryName::new("layout_code_extra").unwrap(),
                ),
            )
            .unwrap();
            let range = wire::field_range(bytes, 9);
            let mut records = wire::array_parts(&bytes[range])
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>();
            records.push(encode(&record).unwrap());
            (9, wire::encode_array_records(&records))
        }
        Case::OuterCode => return bytes.to_vec(),
    };
    wire::replace_field(bytes, field, &replacement)
}

fn executable_branch() -> Vec<u8> {
    // A full executable projection decodes, but cannot describe this library.
    let mut bytes = vec![0xa2, 0, 2, 1, 0xa6];
    for field in 1..=6 {
        bytes.extend_from_slice(&[field, 0x58, 32]);
        bytes.extend_from_slice(&[0; 32]);
    }
    bytes
}
