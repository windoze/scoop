use scoop_identity::{ConeIdentity, SourceDeclarationKey};

use super::*;
use crate::{CallableInterfaceRecordV1, IntrinsicCallableContractError, intrinsic_function_kinds};

mod support;
use support::{Fixture, binders, replace_signature};

#[test]
fn every_shared_intrinsic_uses_typed_roles_for_core_and_ordinary_providers() {
    for origin in [ConeIdentity::CORE, test_support::ordinary_origin()] {
        let fixture = Fixture::new(origin);
        for kind in intrinsic_function_kinds().into_iter().filter(|kind| {
            !matches!(
                kind,
                crate::IntrinsicFunctionKind::Atomic(_)
                    | crate::IntrinsicFunctionKind::MaybeUninit(_)
            )
        }) {
            let (source, record) = fixture.callable(kind);
            assert_eq!(
                fixture
                    .surface
                    .validate_intrinsic_declaration(&source, &record),
                Ok(()),
                "{kind:?}"
            );
            let bytes = encode(&record).unwrap();
            let decoded: crate::DecodedCallableInterfaceRecordV1 =
                decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
        }
    }
}

#[test]
fn shared_intrinsic_rejects_owner_width_and_substitute_provider() {
    let int8 = integer(
        crate::IntegerKind::SIGNED_8,
        crate::NoGcIntegerOperation::UnaryPlus,
    );
    let int16 = integer(
        crate::IntegerKind::SIGNED_16,
        crate::NoGcIntegerOperation::UnaryPlus,
    );
    for origin in [ConeIdentity::CORE, test_support::ordinary_origin()] {
        let fixture = Fixture::new(origin);
        let (source8, record8) = fixture.callable(int8);
        let (source16, record16) = fixture.callable(int16);
        assert_eq!(
            fixture
                .surface
                .validate_intrinsic_declaration(&source16, &record8),
            Err(IntrinsicCallableContractError::Owner(int8))
        );
        let invalid = replace_signature(
            &record8,
            SignatureCallableShape::new(Effect::Ordinary, None, vec![], record16.result().clone()),
            int8,
        );
        assert_eq!(
            fixture
                .surface
                .validate_intrinsic_declaration(&source8, &invalid),
            Err(IntrinsicCallableContractError::Signature(int8))
        );
        let other = if origin == ConeIdentity::CORE {
            test_support::ordinary_origin()
        } else {
            ConeIdentity::CORE
        };
        let other = Fixture::new(other);
        assert_eq!(
            other
                .surface
                .validate_intrinsic_declaration(&source8, &record8),
            Err(IntrinsicCallableContractError::Owner(int8))
        );
    }
}

#[test]
fn shared_intrinsic_rejects_shift_parameter_and_result_substitutions() {
    let fixture = Fixture::new(test_support::ordinary_origin());
    let add = integer(
        crate::IntegerKind::SIGNED_8,
        crate::NoGcIntegerOperation::Add,
    );
    let shift = integer(
        crate::IntegerKind::SIGNED_8,
        crate::NoGcIntegerOperation::Shl,
    );
    let (source, record) = fixture.callable(add);
    let (_, shifted) = fixture.callable(shift);
    let invalid = replace_signature(
        &record,
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            shifted
                .parameters()
                .parameters()
                .iter()
                .map(|p| p.value_type().clone())
                .collect(),
            record.result().clone(),
        ),
        add,
    );
    assert_eq!(
        fixture
            .surface
            .validate_intrinsic_declaration(&source, &invalid),
        Err(IntrinsicCallableContractError::Signature(add))
    );

    let kind = IntrinsicFunctionKind::GcStats;
    let (source, record) = fixture.callable(kind);
    let unit = SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    );
    for signature in [
        SignatureCallableShape::new(Effect::Ordinary, None, vec![], unit.clone()),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            vec![unit.clone()],
            record.result().clone(),
        ),
        SignatureCallableShape::new(
            Effect::Ordinary,
            Some(unit),
            vec![],
            record.result().clone(),
        ),
        SignatureCallableShape::new(Effect::Suspend, None, vec![], record.result().clone()),
    ] {
        let invalid = replace_signature(&record, signature, kind);
        assert_eq!(
            fixture
                .surface
                .validate_intrinsic_declaration(&source, &invalid),
            Err(IntrinsicCallableContractError::Signature(kind))
        );
    }
}

#[test]
fn shared_intrinsic_rejects_own_binder_count_and_non_function_source() {
    let fixture = Fixture::new(test_support::ordinary_origin());
    let kind = IntrinsicFunctionKind::CoroutineStart;
    let (source, record) = fixture.callable(kind);
    let invalid = CallableInterfaceRecordV1::try_new(
        record.declaration(),
        record.owner(),
        binders(2),
        None,
        record.parameters().clone(),
        record.result().clone(),
        record.effects(),
        record.modality(),
        record.access(),
        crate::CanonicalPersistentIdsV1::empty(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        fixture
            .surface
            .validate_intrinsic_declaration(&source, &invalid),
        Err(IntrinsicCallableContractError::TypeParameters(kind))
    );
    let site = scoop_identity::SourceDeclarationSite::new(
        source.origin(),
        source.package().clone(),
        source.owners().clone(),
        source.scope().clone(),
    )
    .unwrap();
    let name = scoop_identity::CanonicalIdentifier::new("different").unwrap();
    let wrong_count = SourceDeclarationKey::function(site.clone(), name.clone(), 2, None, vec![]);
    assert_eq!(
        fixture
            .surface
            .validate_intrinsic_declaration(&wrong_count, &record),
        Err(IntrinsicCallableContractError::TypeParameters(kind))
    );
    let nominal =
        SourceDeclarationKey::nominal(site, name, scoop_identity::SourceNominalKind::Struct, 0);
    assert_eq!(
        fixture
            .surface
            .validate_intrinsic_declaration(&nominal, &record),
        Err(IntrinsicCallableContractError::CallableKind(kind))
    );
}

fn integer(
    kind: crate::IntegerKind,
    operation: crate::NoGcIntegerOperation,
) -> IntrinsicFunctionKind {
    IntrinsicFunctionKind::Integer(crate::IntegerIntrinsicKind::NoGcOperation { kind, operation })
}

#[test]
fn shared_intrinsic_cannot_substitute_another_declaration_for_a_fixed_role() {
    let fixture = Fixture::new(test_support::ordinary_origin());
    let (source, callable) = fixture.callable(IntrinsicFunctionKind::GcCollect);
    let kind = IntrinsicFunctionKind::CurrentSourceLocation;
    let (_, required) = fixture.callable(kind);
    let substitute = replace_signature(
        &callable,
        SignatureCallableShape::new(Effect::Ordinary, None, vec![], required.result().clone()),
        kind,
    );
    assert_eq!(
        fixture
            .surface
            .validate_intrinsic_declaration(&source, &substitute),
        Err(IntrinsicCallableContractError::FixedRole(kind))
    );
}
