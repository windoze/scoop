use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use crate::mir::{InstanceKey, SiteId};
use crate::stable_id::stable_dump_label;
use crate::ty::{EffectRow, TypeStore};

use super::{
    BodyEffectFacts, CallSiteTarget, CallableAbiKind, CallableEffectFacts, CaseSet, CaseTag,
    ContinuationSchemaId, HandleArmEffectFacts, MaterializedEffectFacts, SiteEffectFacts,
    StepSchemaId,
};

struct DumpCtx {
    step_labels: BTreeMap<StepSchemaId, String>,
    continuation_labels: BTreeMap<ContinuationSchemaId, String>,
    case_labels: BTreeMap<(StepSchemaId, CaseTag), String>,
}

impl DumpCtx {
    fn new(facts: &MaterializedEffectFacts, types: &TypeStore) -> Self {
        let mut step_owners = BTreeMap::<StepSchemaId, BTreeSet<String>>::new();
        for (instance, callable_facts) in facts.callable_facts() {
            let root_fqn = format_instance_key(types, instance);
            if let Some(step_schema) = callable_facts.body_step_schema() {
                step_owners
                    .entry(step_schema)
                    .or_default()
                    .insert(root_fqn.clone());
            }
            if let Some(local_control_step) = facts
                .body(instance)
                .and_then(BodyEffectFacts::local_control_step_schema)
            {
                step_owners
                    .entry(local_control_step)
                    .or_default()
                    .insert(format!("{root_fqn}::local_control"));
            }
        }

        let mut continuation_users = BTreeMap::<ContinuationSchemaId, BTreeSet<String>>::new();
        for (step_schema, schema) in facts.step_schemas() {
            let owners = owner_list(&step_owners, *step_schema).join(", ");
            for case in schema.cases() {
                continuation_users
                    .entry(case.continuation_schema())
                    .or_default()
                    .insert(format!(
                        "step_case owners=[{owners}] op={}",
                        format_instance_key(types, case.concrete_op_key().instance_key())
                    ));
            }
        }
        for (instance, body_facts) in facts.bodies() {
            let root_fqn = format_instance_key(types, instance);
            for (site_id, site_facts) in body_facts.sites() {
                let site_label = format_site_id(*site_id);
                match site_facts {
                    SiteEffectFacts::Perform(perform) => {
                        continuation_users
                            .entry(perform.captured_cont_schema())
                            .or_default()
                            .insert(format!("perform {root_fqn} {site_label}"));
                    }
                    SiteEffectFacts::Resume(resume) => {
                        continuation_users
                            .entry(resume.continuation_schema())
                            .or_default()
                            .insert(format!("resume {root_fqn} {site_label}"));
                    }
                    SiteEffectFacts::Handle(handle) => {
                        for arm in handle.arm_facts() {
                            continuation_users
                                .entry(arm.continuation_schema())
                                .or_default()
                                .insert(format!(
                                    "handle_arm {root_fqn} {site_label} handled={}",
                                    describe_case(
                                        facts,
                                        types,
                                        arm.handled_case(),
                                        Some(handle.handled_cases().schema()),
                                    )
                                ));
                        }
                    }
                    SiteEffectFacts::Call(_) | SiteEffectFacts::ClassCtor(_) => {}
                }
            }
        }

        let continuation_labels = facts
            .continuation_schemas()
            .iter()
            .map(|(schema_id, schema)| {
                let users = continuation_users
                    .get(schema_id)
                    .map(|users| users.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                let owners = owner_list(&step_owners, schema.out_step_schema()).join(", ");
                let canonical = format!(
                    "resume={}|answer={}|surface={}|out_step_owners=[{}]|users=[{}]",
                    format_type(types, schema.resume_tuple_ty()),
                    format_type(types, schema.answer_ty()),
                    format_type(types, schema.surface_ty()),
                    owners,
                    users.join(" | "),
                );
                (*schema_id, stable_dump_label("cont", &canonical))
            })
            .collect::<BTreeMap<_, _>>();

        let case_labels = facts
            .step_schemas()
            .iter()
            .flat_map(|(schema_id, schema)| {
                schema.cases().iter().map(|case| {
                    let owners = owner_list(&step_owners, *schema_id).join(", ");
                    let continuation = continuation_labels
                        .get(&case.continuation_schema())
                        .cloned()
                        .unwrap_or_else(|| "cont_missing".to_string());
                    let canonical = format!(
                        "step_owners=[{}]|op={}|payload={}|continuation={}",
                        owners,
                        format_instance_key(types, case.concrete_op_key().instance_key()),
                        format_type(types, case.payload_tuple_ty()),
                        continuation,
                    );
                    (
                        (*schema_id, case.case_tag()),
                        stable_dump_label("case", &canonical),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();

        let step_labels = facts
            .step_schemas()
            .iter()
            .map(|(schema_id, schema)| {
                let owners = owner_list(&step_owners, *schema_id).join(", ");
                let cases = schema
                    .cases()
                    .iter()
                    .map(|case| {
                        let label = case_labels
                            .get(&(*schema_id, case.case_tag()))
                            .cloned()
                            .unwrap_or_else(|| "case_missing".to_string());
                        format!(
                            "{}={}",
                            label,
                            format_instance_key(types, case.concrete_op_key().instance_key())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let canonical = format!(
                    "owners=[{}]|invoke={}|complete={}|continuation_obj={}|cases=[{}]",
                    owners,
                    format_type(types, schema.invoke_args_tuple_ty()),
                    format_type(types, schema.complete_ty()),
                    format_type(types, schema.continuation_obj_ty()),
                    cases,
                );
                (*schema_id, stable_dump_label("step", &canonical))
            })
            .collect::<BTreeMap<_, _>>();

        Self {
            step_labels,
            continuation_labels,
            case_labels,
        }
    }

    fn step_label(&self, step_schema: StepSchemaId) -> String {
        self.step_labels
            .get(&step_schema)
            .cloned()
            .unwrap_or_else(|| "step_missing".to_string())
    }

    fn continuation_label(&self, continuation_schema: ContinuationSchemaId) -> String {
        self.continuation_labels
            .get(&continuation_schema)
            .cloned()
            .unwrap_or_else(|| "cont_missing".to_string())
    }

    fn case_label(&self, step_schema: StepSchemaId, case_tag: CaseTag) -> String {
        self.case_labels
            .get(&(step_schema, case_tag))
            .cloned()
            .unwrap_or_else(|| "case_missing".to_string())
    }

    fn block_label(&self, instance: &InstanceKey, block_id: crate::mir::BasicBlockId) -> String {
        let _ = instance;
        format!("{block_id:?}")
    }

    fn site_label(&self, instance: &InstanceKey, site_id: SiteId) -> String {
        let _ = instance;
        format_site_id(site_id)
    }

    fn case_ref(
        &self,
        facts: &MaterializedEffectFacts,
        types: &TypeStore,
        tag: CaseTag,
        schema_id: Option<StepSchemaId>,
    ) -> String {
        let Some(schema_id) = schema_id.or_else(|| schema_id_for_tag(facts, tag)) else {
            return format!("case_missing={}", describe_case(facts, types, tag, None));
        };
        format!(
            "{}={}",
            self.case_label(schema_id, tag),
            describe_case(facts, types, tag, Some(schema_id))
        )
    }
}

fn owner_list(
    step_owners: &BTreeMap<StepSchemaId, BTreeSet<String>>,
    schema_id: StepSchemaId,
) -> Vec<String> {
    step_owners
        .get(&schema_id)
        .map(|owners| owners.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| vec!["unowned".to_string()])
}

fn describe_case(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    tag: CaseTag,
    schema_id: Option<StepSchemaId>,
) -> String {
    let Some(schema_id) = schema_id.or_else(|| schema_id_for_tag(facts, tag)) else {
        return "missing_case_schema".to_string();
    };
    let Some(schema) = facts.step_schemas().get(&schema_id) else {
        return "missing_step_schema".to_string();
    };
    let Some(case) = schema.cases().iter().find(|case| case.case_tag() == tag) else {
        return "missing_step_case".to_string();
    };
    format_instance_key(types, case.concrete_op_key().instance_key())
}

pub fn render_materialized_effect_facts(
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
) -> String {
    let ctx = DumpCtx::new(facts, types);
    let mut rendered = String::new();
    writeln!(&mut rendered, "MaterializedEffectFacts").unwrap();

    render_snapshot_binding(&mut rendered, facts, 0);
    render_step_schemas(&ctx, &mut rendered, facts, types, 0);
    render_continuation_schemas(&ctx, &mut rendered, facts, types, 0);
    render_callable_facts(&ctx, &mut rendered, facts, types, 0);
    render_body_facts(&ctx, &mut rendered, facts, types, 0);

    rendered
}

fn render_snapshot_binding(out: &mut String, facts: &MaterializedEffectFacts, indent: usize) {
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
        &format!("opt_level: O{}", binding.opt_level().as_str()),
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
    ctx: &DumpCtx,
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
        write_line(out, indent + 2, &format!("{}:", ctx.step_label(*schema_id)));
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
                    ctx.case_label(*schema_id, case.case_tag()),
                    format_instance_key(types, case.concrete_op_key().instance_key()),
                    format_type(types, case.payload_tuple_ty()),
                    ctx.continuation_label(case.continuation_schema())
                ),
            );
        }
    }
}

fn render_continuation_schemas(
    ctx: &DumpCtx,
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
            &format!("{}:", ctx.continuation_label(*schema_id)),
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
                ctx.step_label(schema.out_step_schema())
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
    ctx: &DumpCtx,
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    indent: usize,
) {
    write_line(out, indent, "callable_facts:");
    if facts.callable_facts().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    let mut callables = facts.callable_facts().iter().collect::<Vec<_>>();
    callables.sort_by(|(left, _), (right, _)| {
        format_instance_key(types, left).cmp(&format_instance_key(types, right))
    });
    for (key, callable_facts) in callables {
        let root_fqn = format_instance_key(types, key);
        render_one_callable_facts(
            ctx,
            out,
            facts,
            types,
            &root_fqn,
            key,
            callable_facts,
            indent + 2,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_one_callable_facts(
    ctx: &DumpCtx,
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
                .map(|schema| ctx.step_label(schema))
                .unwrap_or_else(|| "<none>".to_string())
        ),
    );
    write_line(
        out,
        indent + 2,
        &format!(
            "resolved_outward_cases: {}",
            format_case_set(ctx, facts, types, callable_facts.resolved_outward_cases())
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
                ctx,
                facts,
                types,
                callable_facts.body_step_schema(),
                callable_facts.impl_plan()
            )
        ),
    );
}

fn render_body_facts(
    ctx: &DumpCtx,
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    indent: usize,
) {
    write_line(out, indent, "body_facts:");
    if facts.bodies().is_empty() {
        write_line(out, indent + 2, "<none>");
        return;
    }

    let mut bodies = facts.bodies().iter().collect::<Vec<_>>();
    bodies.sort_by(|(left, _), (right, _)| {
        format_instance_key(types, left).cmp(&format_instance_key(types, right))
    });
    for (key, body_facts) in bodies {
        let callable_step_schema = facts
            .callable_facts()
            .get(key)
            .and_then(CallableEffectFacts::body_step_schema)
            .or_else(|| body_facts.local_control_step_schema())
            .or_else(|| infer_body_step_schema(body_facts));
        write_line(
            out,
            indent + 2,
            &format!("{}:", format_instance_key(types, key)),
        );
        write_line(out, indent + 4, "blocks:");
        if body_facts.blocks().is_empty() {
            write_line(out, indent + 6, "<none>");
        } else {
            for (block_id, block_facts) in body_facts.blocks() {
                let block_label = ctx.block_label(key, *block_id);
                write_line(out, indent + 6, &format!("{}:", block_label));
                write_line(
                    out,
                    indent + 8,
                    &format!(
                        "ambient_cases: {}",
                        format_case_set(ctx, facts, types, block_facts.ambient_cases())
                    ),
                );
                write_line(
                    out,
                    indent + 8,
                    &format!(
                        "outward_cases: {}",
                        format_case_set(ctx, facts, types, block_facts.outward_cases())
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
                ctx,
                out,
                facts,
                types,
                key,
                callable_step_schema,
                *site_id,
                site_facts,
                indent + 6,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_site_facts(
    ctx: &DumpCtx,
    out: &mut String,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    instance: &InstanceKey,
    current_step_schema: Option<super::StepSchemaId>,
    site_id: SiteId,
    site_facts: &SiteEffectFacts,
    indent: usize,
) {
    write_line(
        out,
        indent,
        &format!("{}:", ctx.site_label(instance, site_id)),
    );
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
                        .map(|schema| ctx.step_label(schema))
                        .unwrap_or_else(|| "<none>".to_string())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "resolved_cases: {}",
                    format_case_set(ctx, facts, types, call.resolved_cases())
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
                    format_case_set(ctx, facts, types, class_ctor.emitted_cases())
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
                    format_case_ref(
                        ctx,
                        facts,
                        types,
                        perform.emitted_case(),
                        current_step_schema
                    )
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
                    ctx.continuation_label(perform.captured_cont_schema())
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
                    ctx.continuation_label(resume.continuation_schema())
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
                    ctx.step_label(resume.out_step_schema())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "resolved_cases: {}",
                    format_case_set(ctx, facts, types, resume.resolved_cases())
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
                    format_case_set(ctx, facts, types, handle.handled_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "body_outward_cases: {}",
                    format_case_set(ctx, facts, types, handle.body_outward_cases())
                ),
            );
            write_line(
                out,
                indent + 2,
                &format!(
                    "finally_outward_cases: {}",
                    format_case_set(ctx, facts, types, handle.finally_outward_cases())
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
                        ctx,
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
    ctx: &DumpCtx,
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
            format_case_ref(ctx, facts, types, arm.handled_case(), handled_schema),
            format_type(types, arm.payload_tuple_ty()),
            ctx.continuation_label(arm.continuation_schema()),
            format_case_set(ctx, facts, types, arm.arm_outward_cases())
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
        CallSiteTarget::BodylessDirect { fqn } => format!("BodylessDirect({fqn})"),
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
    ctx: &DumpCtx,
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
            format_case_ref(ctx, facts, types, tag, schema_id)
        ),
    }
}

fn format_case_set(
    ctx: &DumpCtx,
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
        .map(|tag| format_case_ref(ctx, facts, types, *tag, Some(case_set.schema())))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn format_case_ref(
    ctx: &DumpCtx,
    facts: &MaterializedEffectFacts,
    types: &TypeStore,
    tag: CaseTag,
    schema_id: Option<super::StepSchemaId>,
) -> String {
    ctx.case_ref(facts, types, tag, schema_id)
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

fn format_site_id(site_id: SiteId) -> String {
    format!("{site_id:?}")
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
