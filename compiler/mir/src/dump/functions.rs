use super::super::*;
use super::{block_number, dump_statements, dump_terminator, function_ref, type_name};

pub(super) fn dump_functions(module: &Module, out: &mut String) {
    for &id in &module.top_level {
        let function = &module.functions[id];
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, &p.ty)))
            .collect();
        out.push_str(&format!(
            "  fun {} {}({}) -> {}{}\n",
            function.name,
            function_ref(id),
            params.join(", "),
            type_name(module, &function.return_ty),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        dump_body(module, &function.body, out);
    }
    for (id, hook) in module.release_hooks.iter() {
        out.push_str(&format!(
            "  release_hook rh{} {} <no-gc>\n",
            id.into_raw(),
            module.classes[hook.owner].name
        ));
        dump_body(module, &hook.code.body, out);
    }
}

fn dump_body(module: &Module, body: &Body, out: &mut String) {
    for (block_id, block) in body.blocks.iter() {
        let loop_header_poll = if body
            .loop_header_polls
            .iter()
            .any(|target| target.header() == block_id)
        {
            " <loop-header-poll>"
        } else {
            ""
        };
        let unwind = block
            .unwind
            .map(|target| format!(" unwind bb{}", block_number(target)))
            .unwrap_or_default();
        out.push_str(&format!(
            "    bb{} {}{}{}\n",
            block_number(block_id),
            block.name,
            loop_header_poll,
            unwind
        ));
        dump_statements(module, &body.locals, &block.statements, 3, out);
        dump_terminator(module, &body.locals, &block.terminator, 3, out);
    }
}
