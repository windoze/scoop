//! Explicit MIR pass pipeline driver.
//!
//! The materializer builds the raw monomorphic snapshot. This module owns the
//! post-materialization pass schedule and the shared mutation context for pass
//! artifacts, so individual passes do not decide artifact revisions or summary
//! refresh policy independently.

use std::collections::HashSet;

use scoopc_mir_facts::pipeline::MirPassKind;

use super::{
    Body, FunDecl, InstanceKey, LocalId, MaterializedEscapeFacts, MaterializedMir, Operand, Rvalue,
    Statement, StatementKind, TerminatorKind, summarize_pass_rewritten_fun,
};

/// Run the canonical MIR pass pipeline over an already materialized snapshot.
pub(crate) fn run_mir_pass_pipeline(materialized: &mut MaterializedMir) {
    MirPassPipeline.run(materialized);
}

struct MirPassPipeline;

impl MirPassPipeline {
    fn run(&self, materialized: &mut MaterializedMir) {
        let rewrite_passes_enabled = materialized
            .opt_level()
            .enables_summary_driven_mir_inlining();
        let mut context = MirPassPipelineContext::new(materialized);

        context.run_scheduled_pass(
            MirPassKind::SummaryDrivenInlining,
            rewrite_passes_enabled,
            super::inline::run_summary_driven_inlining,
        );
        context.run_scheduled_pass(
            MirPassKind::EscapeAnalysis,
            true,
            super::escape::run_escape_analysis,
        );
        let closure_result = context.run_scheduled_pass(
            MirPassKind::ClosureSimplification,
            rewrite_passes_enabled,
            super::closure_simplify::run_non_escaping_closure_simplification,
        );
        if closure_result.changed_bodies > 0 {
            context.run_scheduled_pass(
                MirPassKind::EscapeAnalysis,
                true,
                super::escape::run_escape_analysis,
            );
        }
    }
}

/// Shared pass-artifact mutation context for one materialized MIR pipeline run.
pub(crate) struct MirPassPipelineContext<'a> {
    materialized: &'a mut MaterializedMir,
    active_pass: Option<ActivePass>,
}

impl<'a> MirPassPipelineContext<'a> {
    fn new(materialized: &'a mut MaterializedMir) -> Self {
        Self {
            materialized,
            active_pass: None,
        }
    }

    pub(crate) fn materialized(&self) -> &MaterializedMir {
        self.materialized
    }

    fn run_scheduled_pass(
        &mut self,
        pass: MirPassKind,
        enabled: bool,
        run: impl FnOnce(&mut Self),
    ) -> MirPassStepResult {
        let input_revision = self.materialized.pass_artifacts().current_revision();
        if !enabled {
            self.materialized.pass_artifacts_mut().record_pipeline_run(
                super::MaterializedMirPassRunRecord::disabled(pass, input_revision),
            );
            return MirPassStepResult::default();
        }

        let output_revision = input_revision + 1;
        self.active_pass = Some(ActivePass::new(pass.clone(), output_revision));
        run(self);
        let active = self
            .active_pass
            .take()
            .expect("enabled MIR pass should leave an active pass record");
        let result = active.result();
        let pass_artifacts = self.materialized.pass_artifacts_mut();
        pass_artifacts.finish_pipeline_revision(output_revision);
        pass_artifacts.record_pipeline_run(super::MaterializedMirPassRunRecord::enabled(
            pass,
            input_revision,
            output_revision,
            result.changed_bodies,
            result.changed_summaries,
            result.produced_escape_facts,
        ));
        result
    }

    pub(crate) fn publish_instance_rewrite(&mut self, key: InstanceKey, fun: FunDecl) {
        let previous_summary = {
            let pass_view = self.materialized.pass_view();
            pass_view
                .instance(&key)
                .map(|family| family.summary().clone())
        };
        let summary =
            summarize_pass_rewritten_fun(&fun, &self.materialized.types, previous_summary.as_ref());
        let revision = self.active_revision();
        let pass_artifacts = self.materialized.pass_artifacts_mut();
        pass_artifacts.replace_callable_body_in_revision(fun, revision);
        pass_artifacts.set_instance_summary_in_revision(key, summary, revision);
        let active = self.active_pass_mut();
        active.changed_bodies += 1;
        active.changed_summaries += 1;
    }

    pub(crate) fn publish_caller_rewrite(&mut self, fun: FunDecl) {
        let revision = self.active_revision();
        self.materialized
            .pass_artifacts_mut()
            .replace_callable_body_in_revision(fun, revision);
        self.active_pass_mut().changed_bodies += 1;
    }

    pub(crate) fn publish_escape_facts(&mut self, facts: MaterializedEscapeFacts) {
        let revision = self.active_revision();
        self.materialized
            .pass_artifacts_mut()
            .set_escape_facts_in_revision(facts, revision);
        self.active_pass_mut().produced_escape_facts = true;
    }

    fn active_revision(&self) -> u32 {
        self.active_pass
            .as_ref()
            .expect("MIR pass artifact mutation must happen inside an active pass")
            .revision
    }

    fn active_pass_mut(&mut self) -> &mut ActivePass {
        self.active_pass
            .as_mut()
            .expect("MIR pass artifact mutation must happen inside an active pass")
    }
}

#[derive(Debug)]
struct ActivePass {
    revision: u32,
    changed_bodies: usize,
    changed_summaries: usize,
    produced_escape_facts: bool,
}

impl ActivePass {
    fn new(_pass: MirPassKind, revision: u32) -> Self {
        Self {
            revision,
            changed_bodies: 0,
            changed_summaries: 0,
            produced_escape_facts: false,
        }
    }

    fn result(&self) -> MirPassStepResult {
        MirPassStepResult {
            changed_bodies: self.changed_bodies,
            changed_summaries: self.changed_summaries,
            produced_escape_facts: self.produced_escape_facts,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct MirPassStepResult {
    changed_bodies: usize,
    changed_summaries: usize,
    produced_escape_facts: bool,
}

/// Cleanup shape requested by a pass before publishing a rewritten body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MirPassCleanupMode {
    InlineArtifacts,
    ClosureArtifacts,
}

/// Apply the narrow cleanup that is part of publishing a pass-rewritten body.
pub(crate) fn cleanup_pass_rewritten_body(body: &mut Body, mode: MirPassCleanupMode) -> bool {
    let mut changed = false;
    loop {
        let used = collect_used_locals(body);
        let mut removed_any = false;
        for block in &mut body.blocks {
            let old_stmts = std::mem::take(&mut block.stmts);
            block.stmts = old_stmts
                .into_iter()
                .filter(|stmt| {
                    if dead_removable_assignment(stmt, &used, mode) {
                        removed_any = true;
                        false
                    } else {
                        true
                    }
                })
                .collect();
        }
        if !removed_any {
            break;
        }
        changed = true;
    }
    changed
}

fn dead_removable_assignment(
    stmt: &Statement,
    used: &HashSet<LocalId>,
    mode: MirPassCleanupMode,
) -> bool {
    let StatementKind::Assign { target, value } = &stmt.kind else {
        return false;
    };
    !used.contains(target) && rvalue_is_dead_removable(value, mode)
}

fn rvalue_is_dead_removable(value: &Rvalue, mode: MirPassCleanupMode) -> bool {
    match mode {
        MirPassCleanupMode::InlineArtifacts => {
            matches!(value, Rvalue::TopLevelRef(_) | Rvalue::MakeClosure { .. })
        }
        MirPassCleanupMode::ClosureArtifacts => {
            matches!(
                value,
                Rvalue::MakeClosure { .. } | Rvalue::Use(_) | Rvalue::Transport { .. }
            )
        }
    }
}

fn collect_used_locals(body: &Body) -> HashSet<LocalId> {
    let mut out = HashSet::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            collect_statement_uses(stmt, &mut out);
        }
        collect_terminator_uses(&block.terminator.kind, &mut out);
    }
    out
}

fn collect_statement_uses(stmt: &Statement, out: &mut HashSet<LocalId>) {
    match &stmt.kind {
        StatementKind::Nop | StatementKind::Todo(_) => {}
        StatementKind::Assign { value, .. } => collect_rvalue_uses(value, out),
        StatementKind::StoreMember {
            receiver,
            value,
            continuation_route,
            ..
        } => {
            collect_operand_use(receiver, out);
            collect_operand_use(value, out);
            if let super::StoredContinuationRoutePublication::Unique(route) = continuation_route {
                out.insert(route.source_local);
            }
        }
        StatementKind::StoreTopLevelVar { value, .. } => collect_operand_use(value, out),
    }
}

fn collect_rvalue_uses(value: &Rvalue, out: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Transport { value: operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        } => collect_operand_use(operand, out),
        Rvalue::MemberAccess { receiver, .. } => collect_operand_use(receiver, out),
        Rvalue::Call { kind, args, .. } => {
            collect_call_kind_uses(kind, out);
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_operand_use(element, out);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_operand_use(&field.value, out);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let super::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_operand_use(value, out);
                }
            }
        }
        Rvalue::MakeClosure { env, .. } => collect_operand_use(env, out),
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => {}
    }
}

fn collect_call_kind_uses(kind: &super::CallKind, out: &mut HashSet<LocalId>) {
    match kind {
        super::CallKind::Direct { .. } => {}
        super::CallKind::Closure { callee, .. }
        | super::CallKind::FunValue { callee }
        | super::CallKind::FunPtr { callee } => {
            collect_operand_use(callee, out);
        }
        super::CallKind::Virtual { receiver, .. } | super::CallKind::Interface { receiver, .. } => {
            collect_operand_use(receiver, out);
        }
        super::CallKind::Resume { continuation, .. } => collect_operand_use(continuation, out),
    }
}

fn collect_terminator_uses(kind: &TerminatorKind, out: &mut HashSet<LocalId>) {
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_operand_use(value, out);
            }
        }
        TerminatorKind::CondBr { cond, .. } => collect_operand_use(cond, out),
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, out);
            }
        }
        TerminatorKind::Goto { .. }
        | TerminatorKind::ResumeUnwind
        | TerminatorKind::Handle { .. }
        | TerminatorKind::Unreachable
        | TerminatorKind::Todo(_) => {}
    }
}

fn collect_operand_use(operand: &Operand, out: &mut HashSet<LocalId>) {
    if let Operand::Local(local) = operand {
        out.insert(*local);
    }
}
