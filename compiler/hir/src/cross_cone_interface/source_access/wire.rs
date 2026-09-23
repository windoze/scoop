use super::{Domain, SourceAccessConstraintV1, SourceAccessDomainV1};
use crate::{DecodedSourceNominalId, SourceNominalIdResolver};
use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedSourceIdentity, PersistentIdResolver,
    SourceIdentityResolutionError,
};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

pub trait SourceAccessDomainResolver<E>:
    SourceNominalIdResolver<E> + PersistentIdResolver<ConeIdentity, Error = E>
{
}
impl<R, E> SourceAccessDomainResolver<E> for R where
    R: SourceNominalIdResolver<E> + PersistentIdResolver<ConeIdentity, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceAccessConstraintV1 {
    Cone(DecodedPersistentId<ConeIdentity>),
    File(DecodedSourceIdentity),
    LexicalOwner(DecodedSourceNominalId),
    SubclassesOf(DecodedSourceNominalId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceAccessDomainV1 {
    Empty,
    Conjunction(Vec<DecodedSourceAccessConstraintV1>),
}

impl DecodedSourceAccessDomainV1 {
    pub fn resolve<R: SourceAccessDomainResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SourceAccessDomainV1, SourceAccessDomainResolutionError<E>> {
        meter.check_semantic_depth(2, path)?;
        meter.charge_work(1, path)?;
        meter.charge_nodes(1, path)?;
        let Self::Conjunction(constraints) = self else {
            return Ok(SourceAccessDomainV1::empty());
        };
        meter.check_table_entries(constraints.len() as u64, path)?;
        meter.charge_edges(constraints.len() as u64, path)?;
        let mut resolved: Vec<SourceAccessConstraintV1> = Vec::new();
        meter.try_reserve_collection_slots(&mut resolved, constraints.len(), path)?;
        for (index, constraint) in constraints.into_iter().enumerate() {
            meter.charge_work(1, path)?;
            meter.charge_nodes(1, path)?;
            if let DecodedSourceAccessConstraintV1::File(source) = &constraint {
                let bytes = source.logical_path_byte_len() as u64;
                meter.check_semantic_leaf(bytes, path)?;
                meter.charge_owned_bytes(bytes, path)?;
                meter.charge_work(bytes, path)?;
            }
            let constraint = constraint.resolve(resolver)?;
            if let Some(previous) = resolved.last() {
                let ordering = previous.cmp(&constraint);
                if ordering == std::cmp::Ordering::Equal {
                    return Err(SourceAccessDomainResolutionError::Duplicate { index });
                }
                if ordering == std::cmp::Ordering::Greater {
                    return Err(SourceAccessDomainResolutionError::NonCanonicalOrder { index });
                }
            }
            resolved.push(constraint);
        }
        Ok(SourceAccessDomainV1(Domain::Conjunction(resolved)))
    }
}

impl DecodedSourceAccessConstraintV1 {
    fn resolve<R: SourceAccessDomainResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceAccessConstraintV1, SourceAccessDomainResolutionError<E>> {
        match self {
            Self::Cone(id) => resolver
                .resolve(id)
                .map(SourceAccessConstraintV1::Cone)
                .map_err(SourceAccessDomainResolutionError::Identity),
            Self::File(source) => source
                .resolve(resolver)
                .map(SourceAccessConstraintV1::File)
                .map_err(SourceAccessDomainResolutionError::Source),
            Self::LexicalOwner(owner) => owner
                .resolve(resolver)
                .map(SourceAccessConstraintV1::LexicalOwner)
                .map_err(SourceAccessDomainResolutionError::Identity),
            Self::SubclassesOf(owner) => owner
                .resolve(resolver)
                .map(SourceAccessConstraintV1::SubclassesOf)
                .map_err(SourceAccessDomainResolutionError::Identity),
        }
    }
}

impl WireDecode for DecodedSourceAccessConstraintV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        match d.field(0, Decoder::unsigned)? {
            1 => d.field(1, DecodedPersistentId::decode).map(Self::Cone),
            2 => d.field(1, DecodedSourceIdentity::decode).map(Self::File),
            3 => d
                .field(1, DecodedSourceNominalId::decode)
                .map(Self::LexicalOwner),
            5 => d
                .field(1, DecodedSourceNominalId::decode)
                .map(Self::SubclassesOf),
            tag => Err(unknown_tag(d, tag)),
        }
    }
}

impl WireDecode for DecodedSourceAccessDomainV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        let tag = d.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            tag => return Err(unknown_tag(d, tag)),
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                d.path().clone(),
                Some(d.position()),
            ));
        }
        if tag == 1 {
            Ok(Self::Empty)
        } else {
            d.field(1, |d| {
                d.decode_array(|d, _| DecodedSourceAccessConstraintV1::decode(d))
            })
            .map(Self::Conjunction)
        }
    }
}

macro_rules! encode_constraint {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                let (tag, value): (u64, &dyn WireEncode) = match self {
                    Self::Cone(value) => (1, value),
                    Self::File(value) => (2, value),
                    Self::LexicalOwner(value) => (3, value),
                    Self::SubclassesOf(value) => (5, value),
                };
                e.map(2)?;
                e.field(0)?;
                e.unsigned(tag)?;
                e.field(1)?;
                value.encode(e)
            }
        }
    };
}
encode_constraint!(SourceAccessConstraintV1);
encode_constraint!(DecodedSourceAccessConstraintV1);

fn encode_domain<T: WireEncode>(
    constraints: Option<&[T]>,
    e: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(if constraints.is_some() { 2 } else { 1 })?;
    e.field(0)?;
    e.unsigned(if constraints.is_some() { 2 } else { 1 })?;
    if let Some(constraints) = constraints {
        e.field(1)?;
        e.array(constraints.len() as u64)?;
        for constraint in constraints {
            constraint.encode(e)?;
        }
    }
    Ok(())
}
impl WireEncode for SourceAccessDomainV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            Domain::Empty => encode_domain::<SourceAccessConstraintV1>(None, e),
            Domain::Conjunction(constraints) => encode_domain(Some(constraints.as_slice()), e),
        }
    }
}
impl WireEncode for DecodedSourceAccessDomainV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Empty => encode_domain::<DecodedSourceAccessConstraintV1>(None, e),
            Self::Conjunction(constraints) => encode_domain(Some(constraints.as_slice()), e),
        }
    }
}

fn unknown_tag(d: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        d.path().clone(),
        Some(d.position()),
    )
}

#[derive(Debug, Eq, PartialEq)]
pub enum SourceAccessDomainResolutionError<E> {
    Identity(E),
    Source(SourceIdentityResolutionError<E>),
    Duplicate { index: usize },
    NonCanonicalOrder { index: usize },
    Resource(WireError),
}
impl<E> From<WireError> for SourceAccessDomainResolutionError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for SourceAccessDomainResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => write!(f, "invalid source access identity: {error}"),
            Self::Source(error) => write!(f, "invalid source access file: {error}"),
            Self::Duplicate { index } => write!(f, "duplicate source access constraint at {index}"),
            Self::NonCanonicalOrder { index } => {
                write!(f, "noncanonical source access constraint at {index}")
            }
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for SourceAccessDomainResolutionError<E> {}
