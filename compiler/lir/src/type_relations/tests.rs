use super::*;
use crate::{DecodedOptionalStrongTypeDescriptorRefV2, OptionalStrongTypeDescriptorRefV2};
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId};
use scoop_wire::{decode_canonical, encode};

type Decoded = TypeDescriptorRelations<DecodedOptionalStrongTypeDescriptorRefV2>;

#[test]
fn relations_wire_preserves_any_effect_and_ordered_operands() {
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let any = OptionalStrongTypeDescriptorRefV2::Absent;
    let local = OptionalStrongTypeDescriptorRefV2::Local(exact);
    let dependency = OptionalStrongTypeDescriptorRefV2::DependencyExternal {
        provider: ConeIdentity::CORE,
        exact,
    };
    for relation in [
        TypeDescriptorRelations::Absent,
        TypeDescriptorRelations::Signature {
            is_suspend: false,
            parameters: Vec::new(),
            result: any,
        },
        TypeDescriptorRelations::Signature {
            is_suspend: true,
            parameters: vec![any, local],
            result: dependency,
        },
        TypeDescriptorRelations::Interface {
            parents: vec![local, dependency],
        },
    ] {
        let bytes = encode(&relation).unwrap();
        let decoded: Decoded = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.runtime_kind(), relation.runtime_kind());
        assert_eq!(
            decoded.related_types().len(),
            relation.related_types().len()
        );
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
}

#[test]
fn relations_reject_incomplete_unknown_and_out_of_range_payloads() {
    for bytes in [
        &[0xa1, 0, 4][..],
        &[0xa1, 0, 1],
        &[0xa3, 0, 0, 1, 0x80, 2, 0xa2, 0, 1, 1, 0],
        &[0xa3, 0, 1, 1, 0x80],
        &[0xa3, 0, 1, 1, 0x81, 0xa2, 0, 1, 1, 0],
        &[0xa3, 0, 3, 1, 0x80, 2, 0xa2, 0, 1, 1, 0],
        &[0xa3, 0, 1, 1, 0x9b, 0, 0, 0, 1, 0, 0, 0, 0],
    ] {
        assert!(decode_canonical::<Decoded>(bytes).is_err());
    }
}
