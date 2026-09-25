use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExportBindingId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{DecodedReexportRouteV1, ReexportRouteResolutionError, ReexportRouteV1};

mod semantics;

pub use semantics::DependencyBindingWitnessSemanticValidationError;

/// A consumer-side source-name authorization proof.
///
/// This is semantically distinct from the route stored on a re-export
/// declaration even though v1 deliberately reuses that route's exact wire.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DependencyBindingWitnessV1 {
    route: ReexportRouteV1,
}

impl DependencyBindingWitnessV1 {
    pub const fn new(route: ReexportRouteV1) -> Self {
        Self { route }
    }

    pub const fn route(&self) -> &ReexportRouteV1 {
        &self.route
    }
}

impl WireEncode for DependencyBindingWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.route.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDependencyBindingWitnessV1 {
    route: DecodedReexportRouteV1,
}

impl DecodedDependencyBindingWitnessV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DependencyBindingWitnessV1, DependencyBindingWitnessResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExportBindingId, Error = E>,
    {
        self.route
            .resolve(resolver)
            .map(DependencyBindingWitnessV1::new)
            .map_err(DependencyBindingWitnessResolutionError::Route)
    }
}

impl WireEncode for DecodedDependencyBindingWitnessV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.route.encode(encoder)
    }
}

impl WireDecode for DecodedDependencyBindingWitnessV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedReexportRouteV1::decode(decoder).map(|route| Self { route })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDependencyBindingWitnessesV1 {
    witnesses: Vec<DependencyBindingWitnessV1>,
}

impl CanonicalDependencyBindingWitnessesV1 {
    pub fn try_new(
        mut witnesses: Vec<DependencyBindingWitnessV1>,
    ) -> Result<Self, DependencyBindingWitnessSetBuildError> {
        witnesses.sort_unstable();
        if let Some(witness) = witnesses
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0].clone())
        {
            return Err(DependencyBindingWitnessSetBuildError::Duplicate(witness));
        }
        Ok(Self { witnesses })
    }

    pub fn witnesses(&self) -> &[DependencyBindingWitnessV1] {
        &self.witnesses
    }

    pub fn is_empty(&self) -> bool {
        self.witnesses.is_empty()
    }
}

impl WireEncode for CanonicalDependencyBindingWitnessesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.witnesses.len() as u64)?;
        for witness in &self.witnesses {
            witness.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalDependencyBindingWitnessesV1 {
    witnesses: Vec<DecodedDependencyBindingWitnessV1>,
}

impl DecodedCanonicalDependencyBindingWitnessesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalDependencyBindingWitnessesV1, DependencyBindingWitnessSetValidationError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExportBindingId, Error = E>,
    {
        let mut witnesses = Vec::<DependencyBindingWitnessV1>::with_capacity(self.witnesses.len());
        for (index, witness) in self.witnesses.into_iter().enumerate() {
            let witness = witness.resolve(resolver).map_err(|error| {
                DependencyBindingWitnessSetValidationError::Witness { index, error }
            })?;
            if let Some(previous) = witnesses.last() {
                match previous.cmp(&witness) {
                    std::cmp::Ordering::Equal => {
                        return Err(DependencyBindingWitnessSetValidationError::Duplicate {
                            index,
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(
                            DependencyBindingWitnessSetValidationError::NonCanonicalOrder { index },
                        );
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            witnesses.push(witness);
        }
        Ok(CanonicalDependencyBindingWitnessesV1 { witnesses })
    }
}

impl WireEncode for DecodedCanonicalDependencyBindingWitnessesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.witnesses.len() as u64)?;
        for witness in &self.witnesses {
            witness.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalDependencyBindingWitnessesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedDependencyBindingWitnessV1::decode(decoder))
            .map(|witnesses| Self { witnesses })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyBindingWitnessSetBuildError {
    Duplicate(DependencyBindingWitnessV1),
}

impl fmt::Display for DependencyBindingWitnessSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(witness) => {
                write!(
                    formatter,
                    "duplicate dependency binding witness {witness:?}"
                )
            }
        }
    }
}

impl std::error::Error for DependencyBindingWitnessSetBuildError {}

#[derive(Debug)]
pub enum DependencyBindingWitnessResolutionError<E> {
    Route(ReexportRouteResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for DependencyBindingWitnessResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(error) => write!(formatter, "invalid dependency binding route: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DependencyBindingWitnessResolutionError<E>
{
}

#[derive(Debug)]
pub enum DependencyBindingWitnessSetValidationError<E> {
    Witness {
        index: usize,
        error: DependencyBindingWitnessResolutionError<E>,
    },
    Duplicate {
        index: usize,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for DependencyBindingWitnessSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Witness { index, error } => {
                write!(
                    formatter,
                    "invalid dependency binding witness {index}: {error}"
                )
            }
            Self::Duplicate { index } => {
                write!(
                    formatter,
                    "duplicate dependency binding witness at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical dependency binding witness order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DependencyBindingWitnessSetValidationError<E>
{
}

#[cfg(test)]
mod tests;
