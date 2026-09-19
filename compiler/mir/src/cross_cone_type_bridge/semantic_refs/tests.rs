use super::*;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod dispatch;
mod members;
mod objects;
mod types;
mod wire;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn expected(mut targets: Vec<MirTypeBridgeTargetV1>) -> Vec<MirTypeBridgeTargetV1> {
    targets.sort_unstable();
    targets.dedup();
    targets
}
