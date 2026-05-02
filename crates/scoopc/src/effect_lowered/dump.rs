use std::fmt::Write;

use crate::effect_facts::{CaseTag, ImplPlan};

use super::{LateLoweredCallable, LateLoweredProgram};

/// 渲染 late-lowered program 的稳定文本格式。
pub fn render_late_lowered_program(program: &LateLoweredProgram) -> String {
    let mut rendered = String::new();
    writeln!(&mut rendered, "LateLoweredProgram").unwrap();
    writeln!(&mut rendered, "  callable_count: {}", program.len()).unwrap();
    writeln!(&mut rendered, "  callables:").unwrap();
    if program.is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
        return rendered;
    }
    for callable in program.callables() {
        render_callable(&mut rendered, callable);
    }
    rendered
}

fn render_callable(rendered: &mut String, callable: &LateLoweredCallable) {
    writeln!(rendered, "    - root: {}", callable.root_fqn()).unwrap();
    writeln!(
        rendered,
        "      step_schema: s{}",
        callable.step_schema().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      impl_plan: {}",
        render_impl_plan(callable.impl_plan())
    )
    .unwrap();
    writeln!(
        rendered,
        "      needs_reentry: {}",
        callable.needs_reentry()
    )
    .unwrap();
    writeln!(
        rendered,
        "      resolved_outward_cases: {}",
        render_cases(callable.resolved_outward_cases())
    )
    .unwrap();
}

fn render_impl_plan(plan: ImplPlan) -> String {
    match plan {
        ImplPlan::NoOutward => "NoOutward".to_string(),
        ImplPlan::SingleCase(tag) => format!("SingleCase(c{})", tag.as_u32()),
        ImplPlan::CanonicalFull => "CanonicalFull".to_string(),
    }
}

fn render_cases(cases: &[CaseTag]) -> String {
    if cases.is_empty() {
        return "[]".to_string();
    }
    let rendered = cases
        .iter()
        .map(|tag| format!("c{}", tag.as_u32()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}
