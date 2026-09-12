use super::*;

/// `main` writes `"hello, world"` (core's managed `write` extern)
/// then calls `helper()`, which writes `"!"`.
fn hello_world() -> mir::Module {
    let mut b = Builder::new();
    let hello = b.string("hello, world");
    let bang = b.string("!");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let helper = b.user_fn(
        "helper",
        Arena::new(),
        vec![call_stmt(extern_call(write, vec![string_expr(bang)]))],
    );
    let main = b.main(
        Arena::new(),
        vec![
            call_stmt(extern_call(write, vec![string_expr(hello)])),
            call_stmt(user_call(helper)),
        ],
    );
    b.finish(main)
}

mod control_flow;
mod layouts;
mod module;
mod operators;
mod runtime_calls;
