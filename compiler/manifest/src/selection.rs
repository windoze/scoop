//! Target-dependent source inputs shared by both compiler entry points.

use scoop_identity::TargetProfileId;
use scoop_wire::{Encoder, WireEncode};

mod parse;
mod path;

pub(crate) use parse::{RawConditionalPath, parse_sources};
pub use path::ConeRelativePath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceSelection {
    Default,
    Explicit(Vec<ConditionalSourcePath>),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalSourcePath {
    path: ConeRelativePath,
    predicate: TargetPredicate,
}

impl ConditionalSourcePath {
    pub fn path(&self) -> &ConeRelativePath {
        &self.path
    }

    pub fn predicate(&self) -> &TargetPredicate {
        &self.predicate
    }
}

/// The closed target set is the canonical semantic form of a predicate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TargetPredicate {
    targets: Vec<TargetProfileId>,
}

impl TargetPredicate {
    pub fn matches(&self, target: TargetProfileId) -> bool {
        self.targets.contains(&target)
    }
}

impl WireEncode for SourceSelection {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Default => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(0)
            }
            Self::Explicit(paths) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                encoder.array(paths.len() as u64)?;
                let mut paths: Vec<_> = paths.iter().collect();
                paths.sort();
                for path in paths {
                    encoder.array(2)?;
                    encoder.text(path.path.as_str())?;
                    encoder.array(path.predicate.targets.len() as u64)?;
                    for target in &path.predicate.targets {
                        encoder.text(target.canonical_triple())?;
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
