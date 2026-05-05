use std::fmt::Write as _;
use std::path::Path;

use crate::mir::{InstanceKey, MaterializedMirPassView, SiteId};
use crate::ty::{EffectRow, TypeStore};

use super::{
    BodyEffectFacts, CallSiteTarget, CallableAbiKind, CallableEffectFacts, CaseSet, CaseTag,
    HandleArmEffectFacts, MaterializedEffectFacts, SiteEffectFacts,
};

pub fn render_materialized_effect_facts(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    pass_view: MaterializedMirPassView<'_>,
) -> String {
    let mut rendered = String::new();
    writeln!(&mut rendered, "MaterializedEffectFacts").unwrap();

    render_snapshot_binding(&mut rendered, facts, &pass_view, 0);
    render_step_schemas(&mut rendered, facts, types, 0);
    render_continuation_schemas(&mut rendered, facts, types, 0);
    render_callable_facts(&mut rendered, facts, types, &pass_view, 0);
    render_body_facts(&mut rendered, facts, types, &pass_view, 0);

    rendered
}

fn render_snapshot_binding(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    pass_view: &MaterializedMirPassView<'_>,
    indent: usize,
) {
    let binding = facts.snapshot_binding();
    write_line(out, indent, "snapshot_binding:");
    write_line(
        out,
        indent + 2,
        &format!("query_surface: {:?}", binding.query_surface()),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "opt_level: O{}",
            pass_view.materialized().opt_level().as_str()
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!("instance_count: {}", binding.instance_count()),
    );
    write_line(out, indent + 2, "canonical_body_fqns:");
    if binding.canonical_body_fqns().is_empty() {
        write_line(out, indent + 4, "<none>");
    } else {
        for fqn in binding.canonical_body_fqns() {
            write_line(out, indent + 4, &format!("- {fqn}"));
        }
    }
}

fn render_step_schemas(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    indent: usize,
) {
    write_line(out, indent, "step_schemas:");
    if facts.step_schemas().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    for (schema_id, schema) in facts.step_schemas() {
        write_line(
            out,
            indent + 2,
            &format!("{}:", format_step_schema_id(*schema_id)),
        );
        write_line(
            out,
            indent + 4,
            &format!(
                "invoke_args_tuple_ty: {}",
                format_type(types, schema.invoke_args_tuple_ty())
            ),
        );
        write_line(
            out,
            indent + 4,
            &format!("complete_ty: {}", format_type(types, schema.complete_ty())),
        );
        write_line(
            out,
            indent + 4,
            &format!(
                "continuation_obj_ty: {}",
                format_type(types, schema.continuation_obj_ty())
            ),
        );
        write_line(out, indent + 4, "cases:");
        if schema.cases().is_empty() {
            write_line(out, indent + 6, "<none>");
            continue;
        }

        for case in schema.cases() {
            write_line(
                out,
                indent + 6,
                &format!(
                    "- {}: op={} payload_tuple_ty={} continuation_schema={}",
                    format_case_tag(case.case_tag()),
                    format_instance_key(types, case.concrete_op_key().instance_key()),
                    format_type(types, case.payload_tuple_ty()),
                    format_continuation_schema_id(case.continuation_schema())
                ),
            );
        }
    }
}

fn render_continuation_schemas(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    indent: usize,
) {
    write_line(out, indent, "continuation_schemas:");
    if facts.continuation_schemas().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    for (schema_id, schema) in facts.continuation_schemas() {
        write_line(
            out,
            indent + 2,
            &format!("{}:", format_continuation_schema_id(*schema_id)),
        );
        write_line(
            out,
            indent + 4,
            &format!(
                "resume_tuple_ty: {}",
                format_type(types, schema.resume_tuple_ty())
            ),
        );
        write_line(
            out,
            indent + 4,
            &format!("answer_ty: {}", format_type(types, schema.answer_ty())),
        );
        write_line(
            out,
            indent + 4,
            &format!(
                "out_step_schema: {}",
                format_step_schema_id(schema.out_step_schema())
            ),
        );
        write_line(
            out,
            indent + 4,
            &format!("surface_ty: {}", format_type(types, schema.surface_ty())),
        );
    }
}

fn render_callable_facts(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    pass_view: &MaterializedMirPassView<'_>,
    indent: usize,
) {
    write_line(out, indent, "callable_facts:");
    if facts.callable_facts().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    for family in pass_view.instances() {
        let Some(callable_facts) = facts.callable_facts().get(family.key()) else {
            continue;
        };
        render_one_callable_facts(
            out,
            facts,
            types,
            family.root_fqn(),
            family.key(),
            callable_facts,
            indent + 2,
        );
    }
}

fn render_one_callable_facts(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    root_fqn: &str,
    key: &InstanceKey,
    callable_facts: &CallableEffectFacts,
    indent: usize,
) {
    write_line(out, indent, &format!("{root_fqn}:"));
    write_line(
        out,
        indent + 2,
        &format!("instance_key: {}", format_instance_key(types, key)),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "declared_row: {}",
            format_effect_row(types, callable_facts.declared_row())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "call_abi_kind: {}",
            format_callable_abi_kind(callable_facts.call_abi_kind())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "invoke_args_tuple_ty: {}",
            callable_facts
                .invoke_args_tuple_ty_opt()
                .map(|ty| format_type(types, ty))
                .unwrap_or_else(|| "<none>".to_string())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "step_schema: {}",
            callable_facts
                .body_step_schema()
                .map(format_step_schema_id)
                .unwrap_or_else(|| "<none>".to_string())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "resolved_outward_cases: {}",
            format_case_set(facts, types, callable_facts.resolved_outward_cases())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!("needs_reentry: {}", callable_facts.needs_reentry()),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "impl_plan: {}",
            format_impl_plan(
                facts,
                types,
                callable_facts.body_step_schema(),
                callable_facts.impl_plan()
            )
        ),
    );
}

fn render_body_facts(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    pass_view: &MaterializedMirPassView<'_>,
    indent: usize,
) {
    write_line(out, indent, "body_facts:");
    if facts.bodies().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    for family in pass_view.instances() {
        let Some(body_facts) = facts.body(family.key()) else {
            continue;
        };
        let callable_step_schema = facts
            .callable_facts()
            .get(family.key())
            .and_then(CallableEffectFacts::body_step_schema)
            .or_else(|| body_facts.local_control_step_schema())
            .or_else(|| infer_body_step_schema(body_facts));
        write_line(out, indent + 2, &format!("{}:", family.root_fqn()));
        write_line(out, indent + 4, "blocks:");
        if body_facts.blocks().is_empty() {
            write_line(out, indent + 6, "<none>");
        } else {
            for (block_id, block_facts) in body_facts.blocks() {
                write_line(out, indent + 6, &format!("bb{}:", block_id.as_u32()));
                write_line(
                    out,
                    indent + 8,
                    &format!(
                        "ambient_cases: {}",
                        format_case_set(facts, types, block_facts.ambient_cases())
                    ),
                );
                write_line(
                    out,
                    indent + 8,
                    &format!(
                        "outward_cases: {}",
                        format_case_set(facts, types, block_facts.outward_cases())
                    ),
                );
                write_line(
                    out,
                    indent + 8,
                    &format!(
                        "has_suspend_boundary: {}",
                        block_facts.has_suspend_boundary()
                    ),
                );
                write_line(
                    out,
                    indent + 8,
                    &format!("has_handle_boundary: {}", block_facts.has_handle_boundary()),
                );
            }
        }

        write_line(out, indent + 4, "sites:");
        if body_facts.sites().is_empty() {
            write_line(out, indent + 6, "<none>");
            continue;
        }

        for (site_id, site_facts) in body_facts.sites() {
            render_site_facts(
                out,
                facts,
                types,
                callable_step_schema,
                *site_id,
                site_facts,
                indent + 6,
            );
        }
    }
}

fn render_site_facts(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    current_step_schema: Option<super::StepSchemaId>,
    site_id: SiteId,
    site_facts: &SiteEffectFacts,
    indent: usize,
) {
    write_line(out, indent, &format!("site{}:", site_id.as_u32()));
    match site_facts {
        SiteEffectFacts::Call(call) => {
            write_line(out, indent + 2, "kind: Call");
            write_line(out, indent + 2, &format!("call_kind: {:?}", call.kind()));
            write_line(
                out,
                indent + 2,
                &format!("target_mode: {:?}", call.target_mode()),
            );
            write_line(
                out,
                indent + 2,
                &format!("target: {}", format_call_site_target(types, call.target())),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "callee_abi_kind: {}",
                    format_callable_abi_kind(call.callee_abi_kind())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "invoke_args_tuple_ty: {}",
                    format_type(types, call.invoke_args_tuple_ty())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "callee_schema: {}",
                    call.callee_step_schema()
                        .map(format_step_schema_id)
                        .unwrap_or_else(|| "<none>".to_string())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "resolved_cases: {}",
                    format_case_set(facts, types, call.resolved_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!("precision: {:?}", call.precision()),
            );
        }
        SiteEffectFacts::ClassCtor(class_ctor) => {
            write_line(out, indent + 2, "kind: ClassCtor");
            write_line(
                out,
                indent + 2,
                &format!(
                    "emitted_cases: {}",
                    format_case_set(facts, types, class_ctor.emitted_cases())
                ),
            );
        }
        SiteEffectFacts::Perform(perform) => {
            write_line(out, indent + 2, "kind: Perform");
            write_line(
                out,
                indent + 2,
                &format!(
                    "emitted_case: {}",
                    format_case_ref(facts, types, perform.emitted_case(), current_step_schema)
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "payload_tuple_ty: {}",
                    format_type(types, perform.payload_tuple_ty())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "captured_cont_schema: {}",
                    format_continuation_schema_id(perform.captured_cont_schema())
                ),
            );
        }
        SiteEffectFacts::Resume(resume) => {
            write_line(out, indent + 2, "kind: Resume");
            write_line(
                out,
                indent + 2,
                &format!(
                    "continuation_schema: {}",
                    format_continuation_schema_id(resume.continuation_schema())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "resume_tuple_ty: {}",
                    format_type(types, resume.resume_tuple_ty())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!("answer_ty: {}", format_type(types, resume.answer_ty())),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "out_step_schema: {}",
                    format_step_schema_id(resume.out_step_schema())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "resolved_cases: {}",
                    format_case_set(facts, types, resume.resolved_cases())
                ),
            );
        }
        SiteEffectFacts::Handle(handle) => {
            write_line(out, indent + 2, "kind: Handle");
            write_line(
                out,
                indent + 2,
                &format!("result_ty: {}", format_type(types, handle.result_ty())),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "handled_cases: {}",
                    format_case_set(facts, types, handle.handled_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "body_outward_cases: {}",
                    format_case_set(facts, types, handle.body_outward_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "finally_outward_cases: {}",
                    format_case_set(facts, types, handle.finally_outward_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "nested_handle_classification: {:?}",
                    handle.nested_handle_classification()
                ),
            );
            write_line(out, indent + 2, "arm_facts:");
            if handle.arm_facts().is_empty() {
                write_line(out, indent + 4, "<none>");
            } else {
                for arm in handle.arm_facts() {
                    render_handle_arm_facts(
                        out,
                        facts,
                        types,
                        Some(handle.handled_cases().schema()),
                        arm,
                        indent + 4,
                    );
                }
            }
        }
    }
}

fn infer_body_step_schema(body_facts: &BodyEffectFacts) -> Option<super::StepSchemaId> {
    for block in body_facts.blocks().values() {
        if !block.ambient_cases().is_empty() {
            return Some(block.ambient_cases().schema());
        }
        if !block.outward_cases().is_empty() {
            return Some(block.outward_cases().schema());
        }
    }
    for site in body_facts.sites().values() {
        match site {
            SiteEffectFacts::Call(call) if !call.resolved_cases().is_empty() => {
                return Some(call.resolved_cases().schema());
            }
            SiteEffectFacts::Resume(resume) if !resume.resolved_cases().is_empty() => {
                return Some(resume.resolved_cases().schema());
            }
            SiteEffectFacts::ClassCtor(class_ctor) if !class_ctor.emitted_cases().is_empty() => {
                return Some(class_ctor.emitted_cases().schema());
            }
            SiteEffectFacts::Handle(handle) if !handle.handled_cases().is_empty() => {
                return Some(handle.handled_cases().schema());
            }
            SiteEffectFacts::Handle(handle) if !handle.body_outward_cases().is_empty() => {
                return Some(handle.body_outward_cases().schema());
            }
            SiteEffectFacts::Handle(handle) if !handle.finally_outward_cases().is_empty() => {
                return Some(handle.finally_outward_cases().schema());
            }
            SiteEffectFacts::Call(_)
            | SiteEffectFacts::ClassCtor(_)
            | SiteEffectFacts::Perform(_)
            | SiteEffectFacts::Resume(_)
            | SiteEffectFacts::Handle(_) => {}
        }
    }
    None
}

fn render_handle_arm_facts(
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    handled_schema: Option<super::StepSchemaId>,
    arm: &HandleArmEffectFacts,
    indent: usize,
) {
    write_line(
        out,
        indent,
        &format!(
            "- handled_case: {} payload_tuple_ty={} continuation_schema={} arm_outward_cases={}",
            format_case_ref(facts, types, arm.handled_case(), handled_schema),
            format_type(types, arm.payload_tuple_ty()),
            format_continuation_schema_id(arm.continuation_schema()),
            format_case_set(facts, types, arm.arm_outward_cases())
        ),
    );
}

fn format_call_site_target(types: &TypeStore, target: &CallSiteTarget) -> String {
    match target {
        CallSiteTarget::KnownInstance(key) => {
            format!("KnownInstance({})", format_instance_key(types, key))
        }
        CallSiteTarget::CandidateSet(keys) => {
            let mut rendered = keys
                .iter()
                .map(|key| format_instance_key(types, key))
                .collect::<Vec<_>>();
            rendered.sort();
            format!("CandidateSet([{}])", rendered.join(", "))
        }
        CallSiteTarget::DynamicFallback => "DynamicFallback".to_string(),
    }
}

fn format_callable_abi_kind(kind: CallableAbiKind) -> &'static str {
    match kind {
        CallableAbiKind::Plain => "Plain",
        CallableAbiKind::EffectStep => "EffectStep",
    }
}

fn format_impl_plan(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    schema_id: Option<super::StepSchemaId>,
    plan: super::ImplPlan,
) -> String {
    match plan {
        super::ImplPlan::NoOutward => "NoOutward".to_string(),
        super::ImplPlan::CanonicalFull => "CanonicalFull".to_string(),
        super::ImplPlan::SingleCase(tag) => format!(
            "SingleCase({})",
            format_case_ref(facts, types, tag, schema_id)
        ),
    }
}

fn format_case_set(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    case_set: &CaseSet,
) -> String {
    if case_set.is_empty() {
        return "[]".to_string();
    }

    let rendered = case_set
        .tags()
        .iter()
        .map(|tag| format_case_ref(facts, types, *tag, Some(case_set.schema())))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn format_case_ref(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    tag: CaseTag,
    schema_id: Option<super::StepSchemaId>,
) -> String {
    let Some(schema_id) = schema_id.or_else(|| schema_id_for_tag(facts, tag)) else {
        return format!("{}=<missing-schema>", format_case_tag(tag));
    };
    let Some(schema) = facts.step_schemas().get(&schema_id) else {
        return format!(
            "{}=<missing-{}>",
            format_case_tag(tag),
            format_step_schema_id(schema_id)
        );
    };
    let Some(case) = schema.cases().iter().find(|case| case.case_tag() == tag) else {
        return format!(
            "{}=<missing-case-in-{}>",
            format_case_tag(tag),
            format_step_schema_id(schema_id)
        );
    };
    format!(
        "{}={}",
        format_case_tag(tag),
        format_instance_key(types, case.concrete_op_key().instance_key())
    )
}

fn schema_id_for_tag(facts: &MaterializedEffectFacts, tag: CaseTag) -> Option<super::StepSchemaId> {
    facts.step_schemas().iter().find_map(|(schema_id, schema)| {
        schema
            .cases()
            .iter()
            .any(|case| case.case_tag() == tag)
            .then_some(*schema_id)
    })
}

fn format_effect_row(types: &TypeStore, row: &EffectRow) -> String {
    match row.terms.as_slice() {
        [] => "Pure".to_string(),
        [term] => format_type(types, *term),
        terms => format!(
            "({})",
            terms
                .iter()
                .map(|term| format_type(types, *term))
                .collect::<Vec<_>>()
                .join(" + ")
        ),
    }
}

fn format_instance_key(types: &TypeStore, key: &InstanceKey) -> String {
    let mut args = key
        .type_args
        .iter()
        .map(|ty| format_type(types, *ty))
        .collect::<Vec<_>>();
    args.extend(
        key.eff_args
            .iter()
            .map(|row| format!("eff {}", format_effect_row(types, row))),
    );
    if args.is_empty() {
        key.template.fqn.clone()
    } else {
        format!("{}<{}>", key.template.fqn, args.join(", "))
    }
}

fn format_step_schema_id(id: super::StepSchemaId) -> String {
    format!("step_schema#{}", id.as_u32())
}

fn format_continuation_schema_id(id: super::ContinuationSchemaId) -> String {
    format!("continuation_schema#{}", id.as_u32())
}

fn format_case_tag(tag: CaseTag) -> String {
    format!("case#{}", tag.as_u32())
}

fn format_type(types: &TypeStore, ty: crate::ty::TypeId) -> String {
    normalize_display_text(types.display(ty).to_string())
}

fn normalize_display_text(text: String) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().and_then(Path::parent) else {
        return text;
    };
    let prefix = format!("{}/", workspace_root.display());
    text.replace(&prefix, "")
}

fn write_line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push(' ');
    }
    out.push_str(text);
    out.push('\n');
}
