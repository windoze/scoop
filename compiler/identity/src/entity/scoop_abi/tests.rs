use std::num::NonZeroU64;

use scoop_wire::encode;

use super::{
    CanonicalScoopAbiFunctionSignature, CanonicalScoopStorage, ScoopAbiArgument, ScoopAbiError,
    ScoopAbiReturn, ScoopAbiValueShape,
};
use crate::{ConeIdentity, Effect, ExactCallableSignature, GcEffect, PersistentExactTypeId};

#[test]
fn passing_constructors_reject_wrong_size_and_shape() {
    let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
    let zero = storage(exact, 0, ScoopAbiValueShape::Aggregate);
    let scalar = storage(exact, 8, ScoopAbiValueShape::Scalar);
    let aggregate = storage(exact, 8, ScoopAbiValueShape::Aggregate);

    assert_eq!(
        ScoopAbiArgument::direct(zero),
        Err(ScoopAbiError::ExpectedNonZeroSize)
    );
    assert_eq!(
        ScoopAbiArgument::indirect(scalar),
        Err(ScoopAbiError::PassingShapeMismatch)
    );
    assert_eq!(
        ScoopAbiReturn::elided_zst(aggregate),
        Err(ScoopAbiError::ExpectedZeroSize)
    );
}

#[test]
fn signature_rejects_exact_type_mismatch() {
    let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
    let result = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![parameter], result);
    let argument =
        ScoopAbiArgument::direct(storage(result, 8, ScoopAbiValueShape::Scalar)).unwrap();
    let result = ScoopAbiReturn::direct(storage(result, 8, ScoopAbiValueShape::Scalar)).unwrap();

    assert_eq!(
        CanonicalScoopAbiFunctionSignature::new(
            signature,
            vec![argument],
            result,
            GcEffect::Managed,
        ),
        Err(ScoopAbiError::ArgumentExactTypeMismatch)
    );
}

#[test]
fn extension_receiver_is_the_first_logical_abi_argument() {
    let receiver = PersistentExactTypeId(ConeIdentity::CORE.0);
    let parameter = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, Some(receiver), vec![parameter], parameter);
    let receiver_argument =
        ScoopAbiArgument::direct(storage(receiver, 8, ScoopAbiValueShape::Scalar)).unwrap();
    let parameter_argument =
        ScoopAbiArgument::direct(storage(parameter, 8, ScoopAbiValueShape::Scalar)).unwrap();
    let result = ScoopAbiReturn::direct(storage(parameter, 8, ScoopAbiValueShape::Scalar)).unwrap();

    let canonical = CanonicalScoopAbiFunctionSignature::new(
        signature.clone(),
        vec![receiver_argument, parameter_argument],
        result,
        GcEffect::Managed,
    )
    .unwrap();

    assert_eq!(canonical.signature(), &signature);
    assert_eq!(canonical.arguments().len(), 2);
}

#[test]
fn canonical_scoop_signature_has_fixed_wire_vector() {
    let parameter = PersistentExactTypeId(ConeIdentity::CORE.0);
    let result_exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![parameter], result_exact);
    let argument =
        ScoopAbiArgument::direct(storage(parameter, 8, ScoopAbiValueShape::Scalar)).unwrap();
    let result_abi =
        ScoopAbiReturn::direct(storage(result_exact, 8, ScoopAbiValueShape::Scalar)).unwrap();
    let signature = CanonicalScoopAbiFunctionSignature::new(
        signature,
        vec![argument],
        result_abi,
        GcEffect::NoGc,
    )
    .unwrap();

    assert_eq!(
        hex(&encode(&signature).unwrap()),
        format!(
            "a401a4010102a1000103815820{parameter}045820{result_exact}0281a2000201a4015820{parameter}02080308040103a2000301a4015820{result_exact}0208030804010402"
        )
    );
}

fn storage(
    exact_type: PersistentExactTypeId,
    byte_size: u64,
    shape: ScoopAbiValueShape,
) -> CanonicalScoopStorage {
    CanonicalScoopStorage::new(exact_type, byte_size, NonZeroU64::new(8).unwrap(), shape)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
