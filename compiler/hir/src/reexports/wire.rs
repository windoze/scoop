use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, PersistentExportBindingId, PersistentIdResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CanonicalReexportRoutesV1, ReexportRouteBuildError, ReexportRouteHopV1, ReexportRouteV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedReexportRouteHopV1 {
    exporter: DecodedPersistentId<ConeIdentity>,
    binding: DecodedPersistentId<PersistentExportBindingId>,
}

impl DecodedReexportRouteHopV1 {
    fn resolve<R, E>(self, resolver: &mut R) -> Result<ReexportRouteHopV1, E>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExportBindingId, Error = E>,
    {
        let exporter = <R as PersistentIdResolver<ConeIdentity>>::resolve(resolver, self.exporter)?;
        let binding = <R as PersistentIdResolver<PersistentExportBindingId>>::resolve(
            resolver,
            self.binding,
        )?;
        Ok(ReexportRouteHopV1::new(exporter, binding))
    }
}

impl WireEncode for DecodedReexportRouteHopV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exporter.encode(encoder)?;
        encoder.field(2)?;
        self.binding.encode(encoder)
    }
}

impl WireDecode for DecodedReexportRouteHopV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            exporter: decoder.field(1, DecodedPersistentId::decode)?,
            binding: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedReexportRouteV1 {
    immediate_provider: DecodedPersistentId<ConeIdentity>,
    hops: Vec<DecodedReexportRouteHopV1>,
}

impl DecodedReexportRouteV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ReexportRouteV1, ReexportRouteResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExportBindingId, Error = E>,
    {
        let immediate_provider =
            <R as PersistentIdResolver<ConeIdentity>>::resolve(resolver, self.immediate_provider)
                .map_err(ReexportRouteResolutionError::ImmediateProvider)?;
        let mut hops = Vec::with_capacity(self.hops.len());
        for (index, hop) in self.hops.into_iter().enumerate() {
            hops.push(
                hop.resolve(resolver)
                    .map_err(|error| ReexportRouteResolutionError::Hop { index, error })?,
            );
        }
        ReexportRouteV1::try_new(immediate_provider, hops)
            .map_err(ReexportRouteResolutionError::InvalidRoute)
    }
}

impl WireEncode for DecodedReexportRouteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.immediate_provider.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.hops.len() as u64)?;
        for hop in &self.hops {
            hop.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedReexportRouteV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            immediate_provider: decoder.field(1, DecodedPersistentId::decode)?,
            hops: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedReexportRouteHopV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalReexportRoutesV1 {
    routes: Vec<DecodedReexportRouteV1>,
}

impl DecodedCanonicalReexportRoutesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalReexportRoutesV1, ReexportRouteSetValidationError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentExportBindingId, Error = E>,
    {
        if self.routes.is_empty() {
            return Err(ReexportRouteSetValidationError::Empty);
        }

        let mut routes = Vec::<ReexportRouteV1>::with_capacity(self.routes.len());
        for (index, route) in self.routes.into_iter().enumerate() {
            let route = route
                .resolve(resolver)
                .map_err(|error| ReexportRouteSetValidationError::Route { index, error })?;
            if let Some(previous) = routes.last() {
                match previous.cmp(&route) {
                    std::cmp::Ordering::Equal => {
                        return Err(ReexportRouteSetValidationError::Duplicate { index });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ReexportRouteSetValidationError::NonCanonicalOrder { index });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            routes.push(route);
        }
        Ok(CanonicalReexportRoutesV1 { routes })
    }
}

impl WireEncode for DecodedCanonicalReexportRoutesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.routes.len() as u64)?;
        for route in &self.routes {
            route.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalReexportRoutesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedReexportRouteV1::decode(decoder))
            .map(|routes| Self { routes })
    }
}

#[derive(Debug)]
pub enum ReexportRouteResolutionError<E> {
    ImmediateProvider(E),
    Hop { index: usize, error: E },
    InvalidRoute(ReexportRouteBuildError),
}

impl<E: fmt::Display> fmt::Display for ReexportRouteResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImmediateProvider(error) => {
                write!(formatter, "invalid immediate re-export provider: {error}")
            }
            Self::Hop { index, error } => {
                write!(formatter, "invalid re-export route hop {index}: {error}")
            }
            Self::InvalidRoute(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ReexportRouteResolutionError<E> {}

#[derive(Debug)]
pub enum ReexportRouteSetValidationError<E> {
    Empty,
    Route {
        index: usize,
        error: ReexportRouteResolutionError<E>,
    },
    Duplicate {
        index: usize,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ReexportRouteSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a re-export binding has no source route"),
            Self::Route { index, error } => {
                write!(formatter, "invalid re-export route {index}: {error}")
            }
            Self::Duplicate { index } => {
                write!(formatter, "duplicate re-export route at index {index}")
            }
            Self::NonCanonicalOrder { index } => {
                write!(
                    formatter,
                    "non-canonical re-export route order at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ReexportRouteSetValidationError<E> {}
