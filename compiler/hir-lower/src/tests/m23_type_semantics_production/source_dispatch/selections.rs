use super::*;
use hir::{
    InheritanceCallableDeclarationV1 as Callable, InheritanceSourceSlotSelectionV1 as Selection,
};

mod render;
mod shared;

const SELECTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/selections.scoop"
));
