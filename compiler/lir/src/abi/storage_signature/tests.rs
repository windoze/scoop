use super::*;
use crate::{AbiCarrier, AbiCoercion, AbiNonZeroLayout, AbiPart, LirType, RefScan};
use scoop_identity::{
    CanonicalScoopStorage, Effect, ExactCallableSignature, GcEffect, ScoopAbiValueShape,
    SourceNominalKind,
};

fn storage(name: &str, size: u64, shape: ScoopAbiValueShape) -> CanonicalScoopStorage {
    CanonicalScoopStorage::new(
        crate::tests::test_physical_exact(name, SourceNominalKind::Struct),
        size,
        std::num::NonZeroU64::new(size.min(8)).unwrap(),
        shape,
    )
}

fn local(ty: LirType, size: u64, alignment: u64) -> AbiStorage {
    AbiStorage::NonZero(
        AbiValue::new(
            ty,
            AbiNonZeroLayout::new(size, alignment).unwrap(),
            RefScan::None,
        )
        .unwrap(),
    )
}

fn canonical(arguments: Vec<Argument>, result: Return) -> CanonicalScoopAbiFunctionSignature {
    let result_type = result
        .storage()
        .map(|s| s.exact_type())
        .unwrap_or_else(|| crate::tests::test_physical_exact("unit", SourceNominalKind::Struct));
    CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            arguments.iter().map(|a| a.storage().exact_type()).collect(),
            result_type,
        ),
        arguments,
        result,
        GcEffect::Managed,
    )
    .unwrap()
}

#[test]
fn provider_sysv_sret_and_register_exhaustion_keep_the_recorded_indirect_argument() {
    let integer = storage("integer", 8, ScoopAbiValueShape::Scalar);
    let float = storage("double", 8, ScoopAbiValueShape::Scalar);
    let mixed = storage("mixed", 16, ScoopAbiValueShape::Aggregate);
    let large = storage("large", 32, ScoopAbiValueShape::Aggregate);
    // The provider used the first GPR for sret, then five integer arguments.
    // The mixed aggregate spills as a whole; the following Double stays direct.
    let mut arguments = vec![Argument::direct(integer).unwrap(); 5];
    arguments.extend([
        Argument::indirect(mixed).unwrap(),
        Argument::direct(float).unwrap(),
    ]);
    let provider = canonical(arguments, Return::indirect(large).unwrap());
    let mut arguments = vec![local(LirType::I64, 8, 8); 5];
    arguments.extend([
        local(LirType::Aggregate(vec![LirType::F64, LirType::I64]), 16, 8),
        local(LirType::F64, 8, 8),
    ]);
    let result = AbiStorageReturn::Value(local(LirType::Aggregate(vec![LirType::I64; 4]), 32, 8));
    let bound = AbiStorageSignature { arguments, result }
        .bind(&provider, CallingConvention::Cdecl)
        .unwrap();
    assert!(matches!(bound.arguments()[5], AbiArgument::Indirect(_)));
    assert!(matches!(
        bound.arguments()[6],
        AbiArgument::Direct(AbiDirectValue::Scalar(_))
    ));
    assert!(matches!(bound.result(), AbiReturn::Indirect(_)));
}

#[test]
fn provider_interface_sret_keeps_the_consumers_scan() {
    let interface = storage("interface", 16, ScoopAbiValueShape::Interface);
    let provider = canonical(
        vec![Argument::direct_parts(interface, AbiCoercion::interface()).unwrap()],
        Return::indirect(interface).unwrap(),
    );
    let value = AbiValue::new(
        LirType::Interface,
        AbiNonZeroLayout::new(16, 8).unwrap(),
        RefScan::References(vec![0]),
    )
    .unwrap();
    let bound = AbiStorageSignature {
        arguments: vec![AbiStorage::NonZero(value.clone())],
        result: AbiStorageReturn::Value(AbiStorage::NonZero(value.clone())),
    }
    .bind(&provider, CallingConvention::Cdecl)
    .unwrap();
    assert_eq!(bound.result(), &AbiReturn::Indirect(value));
    assert_eq!(bound.result().scan(), Some(&RefScan::References(vec![0])));
}

#[test]
fn provider_carriers_and_consumer_storage_bounds_are_preserved() {
    let bytes = CanonicalScoopStorage::new(
        crate::tests::test_physical_exact("bytes", SourceNominalKind::Struct),
        3,
        std::num::NonZeroU64::new(1).unwrap(),
        ScoopAbiValueShape::Aggregate,
    );
    let coercion = AbiCoercion::One(AbiPart::new(AbiCarrier::Integer(64), 0, 3, 1).unwrap());
    let provider = canonical(
        vec![Argument::direct_parts(bytes, coercion).unwrap()],
        Return::UnitVoid,
    );
    let input = AbiStorageSignature {
        arguments: vec![local(LirType::Aggregate(vec![LirType::I8; 3]), 3, 1)],
        result: AbiStorageReturn::UnitVoid,
    };
    let bound = input.bind(&provider, CallingConvention::Cdecl).unwrap();
    let AbiArgument::Direct(AbiDirectValue::DirectParts(parts)) = &bound.arguments()[0] else {
        panic!("provider carrier must remain a direct-parts argument");
    };
    assert_eq!(parts.coercion(), coercion);
    assert_eq!(
        parts.value().storage_type(),
        &LirType::Aggregate(vec![LirType::I8; 3])
    );
    assert_eq!(
        AbiStorageSignature {
            arguments: vec![local(LirType::I64, 8, 8)],
            result: AbiStorageReturn::UnitVoid,
        }
        .bind(&provider, CallingConvention::Cdecl),
        Err(Error::AbiArgumentMismatch { index: 0 }),
    );
    assert_eq!(
        AbiStorageSignature {
            arguments: vec![local(LirType::Aggregate(vec![LirType::I8; 3]), 3, 1)],
            result: AbiStorageReturn::Value(local(LirType::I64, 8, 8)),
        }
        .bind(&provider, CallingConvention::Cdecl),
        Err(Error::AbiResultMismatch),
    );
}
