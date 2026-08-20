use super::super::*;
use super::{dump_expr, function_ref, type_name};

pub(super) fn dump_call(
    module: &Module,
    locals: &Arena<Local>,
    call: &Call,
    destination: Option<LocalId>,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let (callee, kind) = match &call.target {
        CallTarget::Direct(callee) => (callee_name(module, *callee), "direct".to_string()),
        CallTarget::Virtual { callee, slot, .. } => (
            callee_name(module, (*callee).into()),
            format!("virtual[{slot}]"),
        ),
        CallTarget::Interface {
            callee,
            interface,
            slot,
            ..
        } => (
            callee_name(module, (*callee).into()),
            format!(
                "interface {}[{slot}]",
                type_name(module, &Type::Interface(*interface))
            ),
        ),
        CallTarget::Closure { function_type, .. } => (
            format!(
                "<closure:function_type{}>",
                function_type.into_raw().into_u32()
            ),
            format!(
                "closure[function_type{}]",
                function_type.into_raw().into_u32()
            ),
        ),
        CallTarget::FunctionBridge { function_type, .. } => (
            format!(
                "<function_bridge:function_type{}>",
                function_type.into_raw().into_u32()
            ),
            format!(
                "function_bridge[function_type{}]",
                function_type.into_raw().into_u32()
            ),
        ),
    };
    match destination {
        Some(local) => out.push_str(&format!(
            "{pad}call {}: {} = {callee} {kind}\n",
            locals[local].name,
            type_name(module, &locals[local].ty)
        )),
        None => out.push_str(&format!("{pad}call {callee} {kind}\n")),
    }
    for arg in call.operands() {
        dump_expr(module, locals, arg, indent + 1, out);
    }
}

fn callee_name(module: &Module, callee: Callee) -> String {
    match callee {
        Callee::User(id) => function_ref(id),
        Callee::Monomorphized(id) => function_ref(module.meta.instances[id].function),
        Callee::Extern(id) => format!(
            "extern{} @{}",
            id.into_raw(),
            module.extern_functions[id].native_symbol
        ),
        Callee::External(id) => format!("external{}", u32::from(id.into_raw())),
        Callee::Runtime(function) => format!("@{}", function.symbol()),
    }
}
