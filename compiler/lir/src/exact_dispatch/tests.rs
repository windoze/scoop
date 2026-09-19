use scoop_identity::{Effect, ExactCallableSignature, GcEffect};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    DecodedCanonicalExactDispatchExportsV1, DecodedExactDispatchExportV1,
    StrongTypeDispatchCallableRefV2,
};

mod fixtures;
use fixtures::*;

mod replay;
mod table;
mod wire;

mod projection;
