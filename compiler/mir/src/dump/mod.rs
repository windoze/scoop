mod declarations;
mod expressions;
mod functions;
mod statements;
mod types;

use crate::{FunctionId, StringConstId};

pub use declarations::dump;
pub use types::type_name;

use expressions::{dump_call, dump_expr};
use statements::{block_number, dump_statements, dump_terminator};

fn function_ref(id: FunctionId) -> String {
    format!("@fn{}", id.into_raw().into_u32())
}

fn string_ref(id: StringConstId) -> String {
    format!("@str{}", id.into_raw().into_u32())
}
