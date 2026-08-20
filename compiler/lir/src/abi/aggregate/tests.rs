use super::*;

fn layout(size: u64, alignment: u64, fields: &[(u64, AbiCarrier)]) -> AbiAggregateLayout {
    AbiAggregateLayout {
        size,
        alignment,
        leaves: fields
            .iter()
            .map(|&(offset, carrier)| AbiScalarLeaf {
                offset,
                carrier,
                alignment: carrier.byte_size().min(8),
            })
            .collect(),
    }
}

fn carriers(
    value: &AbiAggregateLayout,
    target: LirTargetProfile,
    position: AbiValuePosition,
) -> Vec<AbiCarrier> {
    value
        .coercion(target, position)
        .unwrap()
        .parts()
        .iter()
        .map(|part| part.carrier())
        .collect()
}

#[test]
fn aggregate_carriers_match_independent_clang_probes() {
    let integer = AbiCarrier::Integer(64);
    let float = AbiCarrier::Float(FloatKind::F32);
    let double = AbiCarrier::Float(FloatKind::F64);
    let mixed = layout(16, 8, &[(0, double), (8, integer)]);
    let floats = layout(12, 4, &[(0, float), (4, float), (8, float)]);
    let hfa = layout(
        32,
        8,
        &[(0, double), (8, double), (16, double), (24, double)],
    );
    let packed = layout(9, 1, &[(0, AbiCarrier::Integer(8)), (1, double)]);
    for target in [
        LirTargetProfile::LINUX_X86_64_GNU,
        LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        assert_eq!(
            carriers(&mixed, target, AbiValuePosition::Argument),
            [double, integer]
        );
        assert_eq!(
            carriers(&floats, target, AbiValuePosition::Argument),
            [AbiCarrier::FloatPair, float]
        );
        assert!(hfa.coercion(target, AbiValuePosition::Result).is_none());
        assert!(
            packed
                .coercion(target, AbiValuePosition::Argument)
                .is_none()
        );
        assert_eq!(
            carriers(
                &layout(16, 16, &[(0, float)]),
                target,
                AbiValuePosition::Result
            ),
            [float]
        );
    }
    let target = LirTargetProfile::DARWIN_AARCH64;
    assert_eq!(
        carriers(&mixed, target, AbiValuePosition::Argument),
        [AbiCarrier::Array {
            element: AbiArrayElement::I64,
            count: 2
        }]
    );
    assert_eq!(
        carriers(&floats, target, AbiValuePosition::Argument),
        [AbiCarrier::Array {
            element: AbiArrayElement::F32,
            count: 3
        }]
    );
    assert_eq!(
        carriers(&hfa, target, AbiValuePosition::Result),
        [AbiCarrier::Array {
            element: AbiArrayElement::F64,
            count: 4
        }]
    );
    assert_eq!(
        carriers(&packed, target, AbiValuePosition::Argument),
        [AbiCarrier::Array {
            element: AbiArrayElement::I64,
            count: 2
        }]
    );
    assert_eq!(
        carriers(
            &layout(16, 16, &[(0, float)]),
            target,
            AbiValuePosition::Result
        ),
        [AbiCarrier::Integer(128)]
    );
}

#[test]
fn short_darwin_carrier_never_reads_past_exact_storage() {
    let bytes = layout(
        3,
        1,
        &[
            (0, AbiCarrier::Integer(8)),
            (1, AbiCarrier::Integer(8)),
            (2, AbiCarrier::Integer(8)),
        ],
    );
    let argument = bytes
        .coercion(LirTargetProfile::DARWIN_AARCH64, AbiValuePosition::Argument)
        .unwrap();
    let result = bytes
        .coercion(LirTargetProfile::DARWIN_AARCH64, AbiValuePosition::Result)
        .unwrap();
    assert_eq!(argument.parts()[0].carrier(), AbiCarrier::Integer(64));
    assert_eq!(argument.parts()[0].extent(), 3);
    assert_eq!(result.parts()[0].carrier(), AbiCarrier::Integer(24));
    assert_eq!(result.parts()[0].extent(), 3);
}

#[test]
fn sysv_exhaustion_rolls_back_the_entire_aggregate() {
    let integer = AbiCarrier::Integer(64);
    let double = AbiCarrier::Float(FloatKind::F64);
    let mixed = layout(16, 8, &[(0, double), (8, integer)])
        .coercion(
            LirTargetProfile::LINUX_X86_64_GNU,
            AbiValuePosition::Argument,
        )
        .unwrap();
    let pair = layout(16, 8, &[(0, double), (8, double)])
        .coercion(
            LirTargetProfile::LINUX_X86_64_GNU,
            AbiValuePosition::Argument,
        )
        .unwrap();
    for indirect_result in [false, true] {
        let mut registers = AbiArgumentRegisters::sysv(indirect_result);
        for _ in 0..6 - usize::from(indirect_result) {
            registers.scalar(integer);
        }
        assert!(!registers.aggregate(mixed));
        for _ in 0..4 {
            assert!(registers.aggregate(pair));
        }
        assert!(!registers.aggregate(pair));
    }
    let mut registers = AbiArgumentRegisters::sysv(false);
    for _ in 0..8 {
        registers.scalar(double);
    }
    assert!(!registers.aggregate(mixed));
    assert_eq!(registers.gpr, 6);
}
