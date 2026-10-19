use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DecodedSignatureTypeKey, SignatureTypeKey};
use crate::{PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId};

/// Alpha-normalized function conditions. Source order and diagnostic spans
/// are deliberately absent from declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureConditionKey<T = SignatureTypeKey> {
    Member(super::MemberRequirementKey<T>),
    Companion { subject: T },
    Value { subject: T },
    Ref { subject: T },
    Upper { subject: T, bound: T },
    ExcludedInterface { subject: T, bound: T },
}

impl<T: WireEncode> WireEncode for SignatureConditionKey<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        if let Self::Member(requirement) = self {
            encoder.map(2)?;
            encoder.field(0)?;
            encoder.unsigned(6)?;
            encoder.field(1)?;
            return requirement.encode(encoder);
        }
        let (tag, subject, bound) = match self {
            Self::Member(_) => unreachable!("member requirement was encoded"),
            Self::Companion { subject } => (5, subject, None),
            Self::Value { subject } => (1, subject, None),
            Self::Ref { subject } => (2, subject, None),
            Self::Upper { subject, bound } => (3, subject, Some(bound)),
            Self::ExcludedInterface { subject, bound } => (4, subject, Some(bound)),
        };
        encoder.map(2 + u64::from(bound.is_some()))?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        encoder.field(1)?;
        subject.encode(encoder)?;
        if let Some(bound) = bound {
            encoder.field(2)?;
            bound.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for SignatureConditionKey<DecodedSignatureTypeKey> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 | 2 | 5 | 6 => 2,
            3 | 4 => 3,
            _ => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        if tag == 6 {
            return decoder
                .field(1, super::MemberRequirementKey::decode)
                .map(Self::Member);
        }
        let subject = decoder.field(1, DecodedSignatureTypeKey::decode)?;
        match tag {
            5 => Ok(Self::Companion { subject }),
            1 => Ok(Self::Value { subject }),
            2 => Ok(Self::Ref { subject }),
            3 => Ok(Self::Upper {
                subject,
                bound: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            }),
            4 => Ok(Self::ExcludedInterface {
                subject,
                bound: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            }),
            _ => unreachable!("the condition tag was checked"),
        }
    }
}

impl SignatureConditionKey<DecodedSignatureTypeKey> {
    pub(super) fn resolve<R, E>(self, resolver: &mut R) -> Result<SignatureConditionKey, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        Ok(match self {
            Self::Member(requirement) => SignatureConditionKey::Member(
                requirement.try_map(|ty| ty.clone().resolve(resolver))?,
            ),
            Self::Companion { subject } => SignatureConditionKey::Companion {
                subject: subject.resolve(resolver)?,
            },
            Self::Value { subject } => SignatureConditionKey::Value {
                subject: subject.resolve(resolver)?,
            },
            Self::Ref { subject } => SignatureConditionKey::Ref {
                subject: subject.resolve(resolver)?,
            },
            Self::Upper { subject, bound } => SignatureConditionKey::Upper {
                subject: subject.resolve(resolver)?,
                bound: bound.resolve(resolver)?,
            },
            Self::ExcludedInterface { subject, bound } => {
                SignatureConditionKey::ExcludedInterface {
                    subject: subject.resolve(resolver)?,
                    bound: bound.resolve(resolver)?,
                }
            }
        })
    }
}
