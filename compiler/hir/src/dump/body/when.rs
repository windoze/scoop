use super::*;

pub(super) fn dump_when(
    module: &Module,
    locals: &Arena<Local>,
    when: &When,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!("{pad}when\n"));
    if let Some(subject) = when.subject() {
        dump_expr(module, locals, subject, indent + 1, out);
    } else {
        out.push_str(&format!("{pad}  no subject\n"));
    }
    for arm in when.arms() {
        out.push_str(&format!(
            "{}  arm {}{}\n",
            pad,
            match arm.condition {
                WhenConditionRef::Case(pattern) => dump_pattern(module, pattern),
                WhenConditionRef::Predicate(_) => "<predicate>".into(),
                WhenConditionRef::Always => "<fallback>".into(),
            },
            if arm.guard.is_some() {
                " if <guard>"
            } else {
                ""
            }
        ));
        if let WhenConditionRef::Predicate(predicate) = arm.condition {
            out.push_str(&format!("{pad}    condition setup\n"));
            dump_statements(module, locals, &predicate.setup, indent + 3, out);
            out.push_str(&format!("{pad}    condition\n"));
            dump_expr(module, locals, &predicate.condition, indent + 3, out);
        }
        if let Some(guard) = arm.guard {
            if !guard.setup.is_empty() {
                out.push_str(&format!("{}    guard setup\n", pad));
                dump_statements(module, locals, &guard.setup, indent + 3, out);
            }
            out.push_str(&format!("{}    guard condition\n", pad));
            dump_expr(module, locals, &guard.condition, indent + 3, out);
        }
        dump_statements(module, locals, arm.body, indent + 2, out);
    }

    match when {
        When::Subject {
            subject, fallback, ..
        } => dump_fallback(module, locals, fallback, indent, out, |proof| {
            let ty = type_name(module, subject.ty);
            match proof {
                ExhaustivenessProof::IrrefutableArm => format!("irrefutable {ty}"),
                ExhaustivenessProof::PatternMatrix => format!("pattern matrix for {ty}"),
                ExhaustivenessProof::EnumPatternMatrix => format!("enum pattern matrix for {ty}"),
            }
        }),
        When::Conditions { fallback, .. } => dump_fallback(
            module,
            locals,
            fallback,
            indent,
            out,
            |never| match *never {},
        ),
    }
}

fn dump_fallback<P>(
    module: &Module,
    locals: &Arena<Local>,
    fallback: &WhenFallback<P>,
    indent: usize,
    out: &mut String,
    describe: impl FnOnce(&P) -> String,
) {
    let pad = "  ".repeat(indent);
    match fallback {
        WhenFallback::Else(body) => {
            out.push_str(&format!("{pad}  else\n"));
            dump_statements(module, locals, body, indent + 2, out);
        }
        WhenFallback::Fallthrough => out.push_str(&format!("{pad}  fallthrough\n")),
        WhenFallback::Impossible(proof) => {
            out.push_str(&format!("{pad}  impossible <{}>\n", describe(proof)))
        }
    }
}
