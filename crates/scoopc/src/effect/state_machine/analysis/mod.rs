//! Effect / state-machine planning split into focused submodules.
//!
//! `mod.rs` owns the public `HandleStateMachinePlan` shell, the shared
//! type-id aliases and the cross-submodule re-exports that let siblings
//! reach each other through `use super::*;`.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::ast;
use crate::effect::analysis::{
    ContinuationEscapeFacts, ContinuationEscapeState, EffectAnalysisCtx, KnownLocalMetadata,
    collect_known_local_metadata_in_block, collect_known_local_metadata_in_expr,
    collect_known_local_metadata_in_fun, collect_known_local_metadata_in_handle,
    collect_known_local_metadata_in_handle_arm,
};
use crate::expr_facts::ExprFactResolver;
use crate::hir;
use crate::program_facts::ProgramFacts;
use crate::span::Span;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore};

mod arm_scope;
mod builder;
mod collect;
mod direct_step;
mod plan_state;
mod rewrite;
mod suspend_call;
mod suspend_paths;

// Cross-submodule re-exports: glob items from sibling files so each
// submodule can reach the others through `use super::*;`. The explicit
// re-exports below carry the public `pub(crate)` API surface previously
// declared in `super::mod.rs`.
pub(crate) use arm_scope::*;
pub(crate) use builder::*;
pub(crate) use collect::*;
#[cfg(feature = "llvm")]
pub(crate) use direct_step::build_ordinary_callee_suspend_plan_with_context;
pub(crate) use plan_state::*;
pub(crate) use rewrite::*;
pub(crate) use suspend_call::*;
pub(crate) use suspend_paths::*;

pub(crate) type PlanStateId = u32;
pub(crate) type SuspendSiteId = u32;
pub(crate) type ArmPlanId = u32;
pub(crate) type CleanupScopeId = u32;

pub(crate) type HandlePlanContext = EffectAnalysisCtx;

#[derive(Debug, Clone)]
pub(crate) struct HandleStateMachinePlan {
    pub(crate) handle_span: Span,
    pub(crate) result_ty: TypeId,
    pub(crate) entry_state: PlanStateId,
    pub(crate) states: Vec<PlanState>,
    pub(crate) suspend_sites: Vec<SuspendSitePlan>,
    pub(crate) arm_plans: Vec<ArmPlan>,
    pub(crate) cleanup_scopes: Vec<CleanupScopePlan>,
    pub(crate) frame_layout: FrameLayoutPlan,
    pub(crate) dispatch_plan: DispatchPlan,
    pub(crate) nested_handles: Vec<HandleStateMachinePlan>,
}

impl HandleStateMachinePlan {
    fn build_with_context(
        types: &TypeStore,
        handle: &hir::HandleExpr,
        context: &HandlePlanContext,
    ) -> Self {
        HandlePlanBuilder::new(types, handle, context).build()
    }

    pub(crate) fn arm_capture_locals(&self, arm_id: ArmPlanId) -> &[hir::SymbolId] {
        self.arm_plans
            .iter()
            .find(|arm| arm.id == arm_id)
            .map(|arm| arm.capture_locals.as_slice())
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub(crate) fn pretty_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        self.write_pretty_dump(types, 0, &mut out);
        out
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.handle_span.start
            ^ self.handle_span.end
            ^ self.result_ty.as_u32() as usize
            ^ self.entry_state as usize;
        for state in &self.states {
            acc ^= state.structural_signature();
        }
        for site in &self.suspend_sites {
            acc ^= site.structural_signature();
        }
        for arm in &self.arm_plans {
            acc ^= arm.structural_signature();
        }
        for scope in &self.cleanup_scopes {
            acc ^= scope.structural_signature();
        }
        acc ^= self.frame_layout.structural_signature();
        acc ^= self.dispatch_plan.structural_signature();
        for nested in &self.nested_handles {
            acc ^= nested.structural_signature();
        }
        acc
    }

    fn contains_suspend_subtree(&self) -> bool {
        !self.suspend_sites.is_empty()
            || self
                .nested_handles
                .iter()
                .any(Self::contains_suspend_subtree)
    }

    fn materializes_escape_continuation(&self) -> bool {
        self.states.iter().any(|state| {
            matches!(
                state.terminator,
                StateTerminator::ArmExit(ArmBodyExit::MaterializeContinuation)
            )
        }) || self
            .nested_handles
            .iter()
            .any(Self::materializes_escape_continuation)
    }

    /// Return `true` iff this handle may propagate suspension/effect dispatch
    /// to its enclosing state machine rather than resolving everything within
    /// its own dispatch loop.
    ///
    /// Self-contained nested handles such as `try { k.resume(...) } catch`
    /// still contain internal suspend sites, but they do not require the
    /// enclosing `when` / block / outer handle to split around them.
    fn may_suspend_outward(&self) -> bool {
        self.materializes_escape_continuation()
            || self
                .suspend_sites
                .iter()
                .any(SuspendSitePlan::may_suspend_outward)
            || self
                .arm_plans
                .iter()
                .any(|arm| arm.body_may_suspend_outward)
            || self.nested_handles.iter().any(Self::may_suspend_outward)
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, types: &TypeStore, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        out.push_str(&format!(
            "{pad}handle span={:?} result={} entry=s{}\n",
            self.handle_span,
            types.display(self.result_ty),
            self.entry_state
        ));

        out.push_str(&format!("{pad}dispatch:\n"));
        for entry in &self.dispatch_plan.entries {
            let arm_ids = entry
                .arm_ids
                .iter()
                .map(|id| format!("arm{id}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("{pad}  {} => [{}]\n", entry.op_fqn, arm_ids));
        }

        out.push_str(&format!("{pad}frame-layout:\n"));
        out.push_str(&format!(
            "{pad}  state_slot=yes resume_payload=yes cleanup_flag={} one_shot_flag={}\n",
            yes_no(self.frame_layout.has_cleanup_flag),
            yes_no(self.frame_layout.has_one_shot_flag)
        ));
        if self.frame_layout.lifted_locals.is_empty() && self.frame_layout.arm_binders.is_empty() {
            out.push_str(&format!("{pad}  slots=[]\n"));
        } else {
            for slot in &self.frame_layout.lifted_locals {
                out.push_str(&format!(
                    "{pad}  lifted {}:{}\n",
                    slot.display_name(),
                    types.display(slot.ty)
                ));
            }
            for slot in &self.frame_layout.arm_binders {
                out.push_str(&format!(
                    "{pad}  binder arm{} {}:{}\n",
                    slot.owner_arm.unwrap_or(0),
                    slot.display_name(),
                    types.display(slot.ty)
                ));
            }
        }

        out.push_str(&format!("{pad}arms:\n"));
        for arm in &self.arm_plans {
            let binders = if arm.binder_slots.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "[{}]",
                    arm.binder_slots
                        .iter()
                        .map(|slot| format!("{}:{}", slot.display_name(), types.display(slot.ty)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            out.push_str(&format!(
                "{pad}  arm{} op={} effect={} body_entry=s{}\n",
                arm.id,
                arm.op_fqn,
                types.display(arm.effect_ty),
                arm.body_entry_state,
            ));
            out.push_str(&format!("{pad}    binders={binders}\n"));
            let captures = render_symbol_list(&arm.capture_locals, &self.frame_layout.slots);
            out.push_str(&format!("{pad}    captures={captures}\n"));
        }

        out.push_str(&format!("{pad}cleanup-scopes:\n"));
        if self.cleanup_scopes.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for scope in &self.cleanup_scopes {
                out.push_str(&format!(
                    "{pad}  cleanup{} kind={} entry=s{} exit=s{} note={}\n",
                    scope.id,
                    scope.kind.label(),
                    scope.entry_state,
                    scope.exit_state,
                    scope.note
                ));
            }
        }

        out.push_str(&format!("{pad}suspend-sites:\n"));
        if self.suspend_sites.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for site in &self.suspend_sites {
                let available =
                    render_symbol_list(&site.available_locals, &self.frame_layout.slots);
                let captures = render_symbol_list(&site.capture_locals, &self.frame_layout.slots);
                let matching = site
                    .matching_arms
                    .iter()
                    .map(|id| format!("arm{id}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{pad}  site{} kind={} span={:?} owner=s{} resume=s{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.span,
                    site.owner_state,
                    site.resume_target,
                    matching
                ));
                out.push_str(&format!("{pad}    available=[{available}]\n"));
                out.push_str(&format!("{pad}    captures=[{captures}]\n"));
                if let Some(escape_resume_target) = site.escape_resume_target {
                    out.push_str(&format!(
                        "{pad}    escape-resume=s{}\n",
                        escape_resume_target
                    ));
                }
                if let Some(detail) = site.kind.detail() {
                    out.push_str(&format!("{pad}    detail={detail}\n"));
                }
                if let Some(source_path) = &site.source_path {
                    out.push_str(&format!("{pad}    path={}\n", source_path.label()));
                }
                if let Some(resume_path) = &site.resume_path {
                    out.push_str(&format!("{pad}    resume-path={}\n", resume_path.label()));
                }
                if site.kind.is_continuation_resume_boundary() {
                    out.push_str(&format!(
                        "{pad}    continuation-escape={}\n",
                        site.continuation_escape.label()
                    ));
                }
            }
        }

        out.push_str(&format!("{pad}states:\n"));
        for state in &self.states {
            out.push_str(&format!("{pad}  s{} {}:\n", state.id, state.label));
            for action in &state.actions {
                out.push_str(&format!(
                    "{pad}    {}\n",
                    action.label(&self.frame_layout.slots, types)
                ));
            }
            out.push_str(&format!(
                "{pad}    terminator={}\n",
                state.terminator.label()
            ));
        }

        out.push_str(&format!("{pad}nested-handles:\n"));
        if self.nested_handles.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for (idx, nested) in self.nested_handles.iter().enumerate() {
                out.push_str(&format!("{pad}  nested#{idx}\n"));
                nested.write_pretty_dump(types, indent + 4, out);
            }
        }
    }
}
