use super::*;
use scoop_toolchain::NativeInputMetadata;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize)]
pub(super) struct Dependencies {
    inputs: Vec<NativeInputMetadata>,
}

impl Dependencies {
    pub fn collect(paths: BTreeSet<PathBuf>) -> Result<Self, RuntimeBuildError> {
        Ok(Self {
            inputs: paths
                .iter()
                .map(|p| NativeInputMetadata::read(p).map_err(error))
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn input_paths(&self) -> Result<Vec<PathBuf>, RuntimeBuildError> {
        self.inputs
            .iter()
            .map(|p| p.path().map_err(error))
            .collect()
    }

    pub fn is_current(&self) -> Result<bool, RuntimeBuildError> {
        for input in &self.inputs {
            if !input.is_current().map_err(error)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn input_key(&self, base: Digest256) -> Result<Digest256, RuntimeBuildError> {
        Ok(scoop_wire::sha256(
            &serde_json::to_vec(&(base.as_array(), self)).map_err(error)?,
        ))
    }
}
