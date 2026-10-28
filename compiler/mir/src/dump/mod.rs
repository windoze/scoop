mod calls;
mod declarations;
mod expressions;
mod functions;
mod statements;
mod types;

use crate::{FunctionId, ImmortalConstId};

pub use declarations::dump;
pub use types::type_name;

use calls::dump_call;
use expressions::dump_expr;
use statements::{block_number, dump_statements, dump_terminator};

fn function_ref(id: FunctionId) -> String {
    format!("@fn{}", id.into_raw().into_u32())
}

fn byte_array_ref(id: ImmortalConstId) -> String {
    format!("@bytes{}", id.into_raw().into_u32())
}

fn string_ref(id: ImmortalConstId) -> String {
    format!("@str{}", id.into_raw().into_u32())
}
