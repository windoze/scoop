use super::NominalDeclarationReferenceError;
use crate::SourceNominalId;
use scoop_identity::{PersistentFunctionId, PersistentGenericFunctionId, PersistentPropertyId};
use scoop_wire::{Encoder, WireEncode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedSourceMemberRefV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Property(PersistentPropertyId),
}
impl WireEncode for NestedSourceMemberRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::GenericFunction(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Property(id) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNestedMemberRefsV1 {
    values: Vec<NestedSourceMemberRefV1>,
}
impl CanonicalNestedMemberRefsV1 {
    pub fn try_new(
        values: Vec<NestedSourceMemberRefV1>,
    ) -> Result<Self, NominalDeclarationReferenceError> {
        Ok(Self {
            values: canonicalize(values)?,
        })
    }
    pub fn values(&self) -> &[NestedSourceMemberRefV1] {
        &self.values
    }
    pub(crate) fn from_ordered(
        values: Vec<NestedSourceMemberRefV1>,
    ) -> Result<Self, NominalDeclarationReferenceError> {
        validate_order(&values)?;
        Ok(Self { values })
    }
}
impl WireEncode for CanonicalNestedMemberRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNestedNominalRefsV1 {
    values: Vec<SourceNominalId>,
}
impl CanonicalNestedNominalRefsV1 {
    pub fn try_new(values: Vec<SourceNominalId>) -> Result<Self, NominalDeclarationReferenceError> {
        Ok(Self {
            values: canonicalize(values)?,
        })
    }
    pub fn values(&self) -> &[SourceNominalId] {
        &self.values
    }
    pub(crate) fn from_ordered(
        values: Vec<SourceNominalId>,
    ) -> Result<Self, NominalDeclarationReferenceError> {
        validate_order(&values)?;
        Ok(Self { values })
    }
}
impl WireEncode for CanonicalNestedNominalRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

pub(super) fn canonicalize<T: WireEncode>(
    values: Vec<T>,
) -> Result<Vec<T>, NominalDeclarationReferenceError> {
    let mut keyed = values
        .into_iter()
        .map(|value| {
            Ok((
                scoop_wire::encode(&value).map_err(NominalDeclarationReferenceError::Encoding)?,
                value,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(NominalDeclarationReferenceError::Duplicate);
    }
    Ok(keyed.into_iter().map(|(_, value)| value).collect())
}
pub(super) fn validate_order<T: WireEncode>(
    values: &[T],
) -> Result<(), NominalDeclarationReferenceError> {
    let mut previous: Option<Vec<u8>> = None;
    for value in values {
        let key = scoop_wire::encode(value).map_err(NominalDeclarationReferenceError::Encoding)?;
        if let Some(previous) = previous {
            match previous.cmp(&key) {
                std::cmp::Ordering::Equal => {
                    return Err(NominalDeclarationReferenceError::Duplicate);
                }
                std::cmp::Ordering::Greater => {
                    return Err(NominalDeclarationReferenceError::NonCanonicalOrder);
                }
                std::cmp::Ordering::Less => {}
            }
        }
        previous = Some(key);
    }
    Ok(())
}
