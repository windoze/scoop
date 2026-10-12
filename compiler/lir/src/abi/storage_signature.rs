//! Consumer-local storage bindings for an already selected physical ABI.

use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, ScoopAbiArgument as Argument, ScoopAbiReturn as Return,
};

use crate::{
    AbiArgument, AbiDirectParts, AbiDirectValue, AbiReturn, AbiValue, AbiZst, CallingConvention,
    ExternalCallableBuildError as Error, ScoopAbiSignature,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiStorage {
    ZeroSized(AbiZst),
    NonZero(AbiValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiStorageReturn {
    UnitVoid,
    Value(AbiStorage),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbiStorageSignature {
    pub arguments: Vec<AbiStorage>,
    pub result: AbiStorageReturn,
}

impl AbiStorageSignature {
    pub(crate) fn bind(
        self,
        canonical: &CanonicalScoopAbiFunctionSignature,
        convention: CallingConvention,
    ) -> Result<ScoopAbiSignature, Error> {
        if self.arguments.len() != canonical.arguments().len() {
            return Err(Error::AbiArgumentCount {
                expected: canonical.arguments().len(),
                actual: self.arguments.len(),
            });
        }
        let arguments = canonical
            .arguments()
            .iter()
            .copied()
            .zip(self.arguments)
            .enumerate()
            .map(|(index, (expected, actual))| {
                bind_argument(expected, actual).ok_or(Error::AbiArgumentMismatch { index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result =
            bind_result(canonical.result(), self.result).ok_or(Error::AbiResultMismatch)?;
        Ok(ScoopAbiSignature::new(arguments, result, convention))
    }
}

fn bind_argument(expected: Argument, actual: AbiStorage) -> Option<AbiArgument> {
    let layout = expected.storage();
    match (expected, actual) {
        (Argument::ElidedZst(_), AbiStorage::ZeroSized(value))
            if layout.byte_size() == 0 && layout.alignment() == value.layout().alignment() =>
        {
            Some(AbiArgument::ElidedZst(value))
        }
        (_, AbiStorage::NonZero(value))
            if layout.byte_size() == value.layout().size().get()
                && layout.alignment() == value.layout().alignment() =>
        {
            Some(match expected {
                Argument::Direct(_) => AbiArgument::Direct(value.into()),
                Argument::Indirect(_) => AbiArgument::Indirect(value),
                Argument::DirectParts(_, coercion) => AbiArgument::Direct(
                    AbiDirectValue::DirectParts(AbiDirectParts::new(value, coercion).ok()?),
                ),
                Argument::ElidedZst(_) => return None,
            })
        }
        _ => None,
    }
}

fn bind_result(expected: Return, actual: AbiStorageReturn) -> Option<AbiReturn> {
    let argument = match (expected, actual) {
        (Return::UnitVoid, AbiStorageReturn::UnitVoid) => return Some(AbiReturn::UnitVoid),
        (Return::ElidedZst(storage), AbiStorageReturn::Value(value)) => {
            bind_argument(Argument::ElidedZst(storage), value)?
        }
        (Return::Direct(storage), AbiStorageReturn::Value(value)) => {
            bind_argument(Argument::Direct(storage), value)?
        }
        (Return::Indirect(storage), AbiStorageReturn::Value(value)) => {
            bind_argument(Argument::Indirect(storage), value)?
        }
        (Return::DirectParts(storage, coercion), AbiStorageReturn::Value(value)) => {
            bind_argument(Argument::DirectParts(storage, coercion), value)?
        }
        _ => return None,
    };
    Some(match argument {
        AbiArgument::ElidedZst(value) => AbiReturn::ElidedZst(value),
        AbiArgument::Direct(value) => AbiReturn::Direct(value),
        AbiArgument::Indirect(value) => AbiReturn::Indirect(value),
    })
}

#[cfg(test)]
mod tests;
