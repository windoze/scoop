use super::super::*;
use super::{dump_call, dump_expr, type_name};

pub(super) fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::Call(effect) => match effect {
                CallEffect::Unit(call) => dump_call(module, locals, call, None, indent, out),
                CallEffect::Value { destination, call } => {
                    dump_call(module, locals, call, Some(*destination), indent, out)
                }
            },
            StatementKind::ValDecl { local, init } => {
                let local = &locals[*local];
                let keyword = if local.mutable { "var" } else { "val" };
                out.push_str(&format!(
                    "{pad}{keyword} {}: {}\n",
                    local.name,
                    type_name(module, &local.ty)
                ));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}field_set {index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::AtomicFieldStore {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}atomic_store_release field={index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                out.push_str(&format!(
                    "{pad}array_set {}\n",
                    module.classes[*array_type].name
                ));
                dump_expr(module, locals, array, indent + 1, out);
                dump_expr(module, locals, index, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Assign { local, value } => {
                out.push_str(&format!("{pad}assign {}\n", locals[*local].name));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::GlobalAssign { global, value } => {
                out.push_str(&format!(
                    "{pad}global_assign {}\n",
                    module.globals[*global].name
                ));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Eh(eh) => match eh {
                EhStatement::LandingPad { cleanup } => {
                    out.push_str(&format!("{pad}landing_pad cleanup={cleanup}\n"));
                }
                EhStatement::BeginCatch => out.push_str(&format!("{pad}begin_catch\n")),
                EhStatement::EndCatch => out.push_str(&format!("{pad}end_catch\n")),
            },
        }
    }
}

pub(super) fn block_number(id: BlockId) -> u32 {
    id.into_raw().into_u32()
}

pub(super) fn dump_terminator(
    module: &Module,
    locals: &Arena<Local>,
    terminator: &Terminator,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    match terminator {
        Terminator::Goto(target) => {
            out.push_str(&format!("{pad}goto bb{}\n", block_number(*target)));
        }
        Terminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            out.push_str(&format!(
                "{pad}branch bb{} bb{}\n",
                block_number(*then_block),
                block_number(*else_block)
            ));
            dump_expr(module, locals, cond, indent + 1, out);
        }
        Terminator::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(module, locals, value, indent + 1, out);
            }
        }
        Terminator::Throw { exception, unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}throw{edge}\n"));
            dump_expr(module, locals, exception, indent + 1, out);
        }
        Terminator::Rethrow { unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}rethrow{edge}\n"));
        }
        Terminator::Resume => out.push_str(&format!("{pad}resume\n")),
        Terminator::Trap { message } => {
            out.push_str(&format!("{pad}trap @{}\n", module.strings[*message].symbol));
        }
        Terminator::Unreachable => out.push_str(&format!("{pad}unreachable\n")),
    }
}
