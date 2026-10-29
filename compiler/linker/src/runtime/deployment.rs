use scoop_lir::{CBridgePlatformContractV1, CBridgeToolchainProfileV1, DarwinPackedVersionV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{LinkError, error};

/// The full target carries its OS baseline; Darwin also selects a minimum OS.
#[derive(Clone, Copy, Debug)]
pub(super) enum RuntimeDeployment {
    TargetBaseline,
    Darwin(DarwinPackedVersionV1),
}

impl RuntimeDeployment {
    pub fn for_toolchain(toolchain: &CBridgeToolchainProfileV1) -> Self {
        match toolchain.contract().platform() {
            CBridgePlatformContractV1::Linux { .. } | CBridgePlatformContractV1::Windows { .. } => {
                Self::TargetBaseline
            }
            CBridgePlatformContractV1::Darwin { deployment, .. } => {
                Self::Darwin(deployment.minimum_os())
            }
        }
    }

    pub fn check(self, toolchain: &CBridgeToolchainProfileV1) -> Result<(), LinkError> {
        match (self, Self::for_toolchain(toolchain)) {
            (Self::TargetBaseline, Self::TargetBaseline) => Ok(()),
            (Self::Darwin(required), Self::Darwin(selected)) if required <= selected => Ok(()),
            (Self::Darwin(required), Self::Darwin(selected)) => Err(error(format!(
                "runtime requires macOS {required}, but the selected deployment is {selected}"
            ))),
            _ => Err(error(
                "runtime deployment platform does not match the selected target",
            )),
        }
    }
}

impl WireEncode for RuntimeDeployment {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::TargetBaseline => 0,
            Self::Darwin(version) => u64::from(version.packed()),
        })
    }
}

impl WireDecode for RuntimeDeployment {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.u32()? {
            0 => Ok(Self::TargetBaseline),
            packed => Ok(Self::Darwin(
                DarwinPackedVersionV1::new(packed).expect("nonzero packed version"),
            )),
        }
    }
}
