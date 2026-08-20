//! Shared canonical-to-physical ABI validation for external Scoop callables.

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ScoopAbiArgument as CanonicalArgument,
    ScoopAbiReturn as CanonicalReturn,
};

use crate::{AbiArgument, AbiReturn, ScoopAbiSignature};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalScoopAbiMismatch {
    ArgumentCount { expected: usize, actual: usize },
    Argument { index: usize },
    Result,
}

pub(crate) fn validate_canonical_scoop_abi(
    canonical: &CanonicalScoopAbiFunctionSignature,
    physical: &ScoopAbiSignature,
) -> Result<(), CanonicalScoopAbiMismatch> {
    if physical.logical_argument_count() != canonical.arguments().len() {
        return Err(CanonicalScoopAbiMismatch::ArgumentCount {
            expected: canonical.arguments().len(),
            actual: physical.logical_argument_count(),
        });
    }
    for (index, (canonical, physical)) in canonical
        .arguments()
        .iter()
        .zip(physical.arguments())
        .enumerate()
    {
        if !argument_matches(*canonical, physical) {
            return Err(CanonicalScoopAbiMismatch::Argument { index });
        }
    }
    if !return_matches(canonical.result(), physical.result()) {
        return Err(CanonicalScoopAbiMismatch::Result);
    }
    Ok(())
}

fn argument_matches(canonical: CanonicalArgument, physical: &AbiArgument) -> bool {
    match (canonical, physical) {
        (CanonicalArgument::ElidedZst(expected), AbiArgument::ElidedZst(actual)) => {
            expected.byte_size() == actual.layout().size()
                && expected.alignment() == actual.layout().alignment()
        }
        (
            CanonicalArgument::Direct(expected),
            AbiArgument::Direct(crate::AbiDirectValue::Scalar(actual)),
        )
        | (CanonicalArgument::Indirect(expected), AbiArgument::Indirect(actual)) => {
            expected.byte_size() == actual.layout().size().get()
                && expected.alignment() == actual.layout().alignment()
        }
        (
            CanonicalArgument::DirectParts(expected, coercion),
            AbiArgument::Direct(crate::AbiDirectValue::DirectParts(actual)),
        ) => {
            expected.byte_size() == actual.value().layout().size().get()
                && expected.alignment() == actual.value().layout().alignment()
                && coercion == actual.coercion()
        }
        _ => false,
    }
}

fn return_matches(canonical: CanonicalReturn, physical: &AbiReturn) -> bool {
    match (canonical, physical) {
        (CanonicalReturn::UnitVoid, AbiReturn::UnitVoid) => true,
        (CanonicalReturn::ElidedZst(expected), AbiReturn::ElidedZst(actual)) => {
            expected.byte_size() == actual.layout().size()
                && expected.alignment() == actual.layout().alignment()
        }
        (
            CanonicalReturn::Direct(expected),
            AbiReturn::Direct(crate::AbiDirectValue::Scalar(actual)),
        )
        | (CanonicalReturn::Indirect(expected), AbiReturn::Indirect(actual)) => {
            expected.byte_size() == actual.layout().size().get()
                && expected.alignment() == actual.layout().alignment()
        }
        (
            CanonicalReturn::DirectParts(expected, coercion),
            AbiReturn::Direct(crate::AbiDirectValue::DirectParts(actual)),
        ) => {
            expected.byte_size() == actual.value().layout().size().get()
                && expected.alignment() == actual.value().layout().alignment()
                && coercion == actual.coercion()
        }
        _ => false,
    }
}
