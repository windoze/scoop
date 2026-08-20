use super::*;
use scoop_wire::{decode_canonical, encode};

mod dispatch;
mod members;
mod objects;
mod types;
mod wire;

fn expected(mut targets: Vec<MirTypeBridgeTargetV1>) -> Vec<MirTypeBridgeTargetV1> {
    targets.sort_unstable();
    targets.dedup();
    targets
}
