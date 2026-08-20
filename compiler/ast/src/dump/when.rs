use super::{dump_block, dump_expr, dump_pattern, dump_type_ref};
use crate::{When, WhenArmCondition, WhenCondition, WhenSubject};

pub(super) fn dump_when(when: &When, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match &when.subject {
        WhenSubject::Absent => out.push_str(&format!("{pad}no subject\n")),
        WhenSubject::Expression(value) => dump_expr(value, indent, out),
        WhenSubject::Declaration(declaration) => {
            let ty = declaration
                .ty
                .as_ref()
                .map(|ty| format!(": {}", dump_type_ref(ty)))
                .unwrap_or_default();
            out.push_str(&format!(
                "{pad}subject val {}{ty}\n",
                dump_pattern(&declaration.target)
            ));
            dump_expr(&declaration.init, indent + 1, out);
        }
    }
    for arm in &when.arms {
        let guard = if arm.guard.is_some() {
            " if <guard>"
        } else {
            ""
        };
        match &arm.condition {
            WhenArmCondition::Case(pattern) => {
                out.push_str(&format!("{pad}arm {}{guard}\n", dump_pattern(pattern)))
            }
            WhenArmCondition::Else => out.push_str(&format!("{pad}else{guard}\n")),
            WhenArmCondition::Conditions(conditions) => {
                out.push_str(&format!("{pad}conditions{guard}\n"));
                for condition in conditions.iter() {
                    match condition {
                        WhenCondition::Expression(value) => dump_expr(value, indent + 1, out),
                        WhenCondition::Is { ty, negated, .. } => out.push_str(&format!(
                            "{pad}  {}is {}\n",
                            if *negated { "!" } else { "" },
                            dump_type_ref(ty)
                        )),
                        WhenCondition::In {
                            collection,
                            negated,
                            ..
                        } => {
                            out.push_str(&format!(
                                "{pad}  {}in\n",
                                if *negated { "!" } else { "" }
                            ));
                            dump_expr(collection, indent + 2, out);
                        }
                    }
                }
            }
        }
        dump_block(&arm.body, indent + 1, out);
    }
    if let Some(body) = &when.else_body {
        out.push_str(&format!("{pad}else\n"));
        dump_block(body, indent + 1, out);
    }
}
