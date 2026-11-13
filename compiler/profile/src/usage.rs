//! Coverage and actual consumers of one feedback compilation.

use std::collections::BTreeMap;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeedbackConsumer {
    Inline,
    StringFusion,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseSummary {
    pub matched_functions: usize,
    pub matched_positions: usize,
    pub partial_positions: usize,
    pub missing_positions: usize,
    pub changed_positions: usize,
    pub consumers: BTreeMap<FeedbackConsumer, usize>,
}

impl UseSummary {
    pub fn used(&self) -> bool {
        self.consumers.values().any(|&positions| positions != 0)
    }

    pub fn encode_optional(
        value: Option<&Self>,
        e: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(u64::from(value.is_some()))?;
        if let Some(value) = value {
            value.encode(e)?;
        }
        Ok(())
    }

    pub fn decode_optional(d: &mut Decoder<'_>) -> Result<Option<Self>, WireError> {
        match d.array()? {
            0 => Ok(None),
            1 => Self::decode(d).map(Some),
            _ => Err(invalid(d)),
        }
    }
}

impl WireEncode for UseSummary {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(6)?;
        for value in [
            self.matched_functions,
            self.matched_positions,
            self.partial_positions,
            self.missing_positions,
            self.changed_positions,
        ] {
            e.unsigned(value as u64)?;
        }
        e.map(self.consumers.len() as u64)?;
        for (kind, count) in &self.consumers {
            e.unsigned(match kind {
                FeedbackConsumer::Inline => 1,
                FeedbackConsumer::StringFusion => 2,
            })?;
            e.unsigned(*count as u64)?;
        }
        Ok(())
    }
}

impl WireDecode for UseSummary {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        if d.array()? != 6 {
            return Err(invalid(d));
        }
        let mut value = Self {
            matched_functions: count(d)?,
            matched_positions: count(d)?,
            partial_positions: count(d)?,
            missing_positions: count(d)?,
            changed_positions: count(d)?,
            consumers: BTreeMap::new(),
        };
        for _ in 0..d.map()? {
            let kind = match d.unsigned()? {
                1 => FeedbackConsumer::Inline,
                2 => FeedbackConsumer::StringFusion,
                _ => return Err(invalid(d)),
            };
            if value.consumers.insert(kind, count(d)?).is_some() {
                return Err(invalid(d));
            }
        }
        Ok(value)
    }
}

fn count(d: &mut Decoder<'_>) -> Result<usize, WireError> {
    usize::try_from(d.unsigned()?).map_err(|_| invalid(d))
}

fn invalid(d: &Decoder<'_>) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::IntegerOutOfRange,
        d.path().clone(),
        Some(d.position()),
    )
}
