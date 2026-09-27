use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;

const ALL_ROLES: [ExternalHirReferenceRoleV1; 9] = [
    ExternalHirReferenceRoleV1::ReexportTarget,
    ExternalHirReferenceRoleV1::SignatureDependency,
    ExternalHirReferenceRoleV1::AliasTarget,
    ExternalHirReferenceRoleV1::DefaultDependency,
    ExternalHirReferenceRoleV1::ConstType,
    ExternalHirReferenceRoleV1::ConcreteSelectedUse,
    ExternalHirReferenceRoleV1::InheritanceDependency,
    ExternalHirReferenceRoleV1::ExecutableTypeDependency,
    ExternalHirReferenceRoleV1::TemplateDependency,
];

#[test]
fn roles_have_frozen_unsigned_tags() {
    for (role, tag) in ALL_ROLES.into_iter().zip([1_u8, 2, 3, 4, 5, 6, 7, 8, 10]) {
        assert_eq!(encode(&role).unwrap(), vec![tag]);
    }

    for tag in [0_u64, 9, 11] {
        let error =
            decode_canonical::<ExternalHirReferenceRoleV1>(&encode(&Unsigned(tag)).unwrap())
                .unwrap_err();
        assert!(
            matches!(error.kind(), WireErrorKind::UnknownTag { tag: actual } if *actual == tag)
        );
    }
}

#[test]
fn producer_sorts_a_non_empty_role_set_and_rejects_duplicates() {
    let roles = CanonicalExternalHirReferenceRolesV1::try_new(vec![
        ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ExternalHirReferenceRoleV1::AliasTarget,
        ExternalHirReferenceRoleV1::ReexportTarget,
    ])
    .unwrap();

    assert_eq!(
        roles.roles(),
        &[
            ExternalHirReferenceRoleV1::ReexportTarget,
            ExternalHirReferenceRoleV1::AliasTarget,
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ]
    );
    assert!(roles.contains(ExternalHirReferenceRoleV1::AliasTarget));
    assert!(!roles.contains(ExternalHirReferenceRoleV1::ConstType));
    assert_eq!(
        CanonicalExternalHirReferenceRolesV1::try_new(Vec::new()),
        Err(ExternalHirReferenceRoleSetBuildError::Empty)
    );
    assert_eq!(
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::ConstType,
            ExternalHirReferenceRoleV1::ConstType,
        ]),
        Err(ExternalHirReferenceRoleSetBuildError::Duplicate(
            ExternalHirReferenceRoleV1::ConstType
        ))
    );
}

#[test]
fn all_roles_have_a_stable_canonical_wire() {
    let roles = CanonicalExternalHirReferenceRolesV1::try_new(ALL_ROLES.to_vec()).unwrap();

    assert_eq!(
        encode(&roles).unwrap(),
        b"\x89\x01\x02\x03\x04\x05\x06\x07\x08\x0a"
    );
    assert_eq!(decode_roles(&roles).validate().unwrap(), roles);
}

#[test]
fn reader_rejects_empty_duplicate_and_noncanonical_sets() {
    assert_eq!(
        decode_roles(&RoleSequence(Vec::new())).validate(),
        Err(ExternalHirReferenceRoleSetValidationError::Empty)
    );
    assert_eq!(
        decode_roles(&RoleSequence(vec![
            ExternalHirReferenceRoleV1::AliasTarget,
            ExternalHirReferenceRoleV1::AliasTarget,
        ]))
        .validate(),
        Err(ExternalHirReferenceRoleSetValidationError::Duplicate {
            index: 1,
            role: ExternalHirReferenceRoleV1::AliasTarget,
        })
    );
    assert_eq!(
        decode_roles(&RoleSequence(vec![
            ExternalHirReferenceRoleV1::DefaultDependency,
            ExternalHirReferenceRoleV1::SignatureDependency,
        ]))
        .validate(),
        Err(ExternalHirReferenceRoleSetValidationError::NonCanonicalOrder { index: 1 })
    );
}

struct Unsigned(u64);

impl WireEncode for Unsigned {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.0)
    }
}

struct RoleSequence(Vec<ExternalHirReferenceRoleV1>);

impl WireEncode for RoleSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for role in &self.0 {
            role.encode(encoder)?;
        }
        Ok(())
    }
}

fn decode_roles(value: &impl WireEncode) -> DecodedCanonicalExternalHirReferenceRolesV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}
