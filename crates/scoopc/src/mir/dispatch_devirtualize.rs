//! MIR-owned dispatch devirtualization pass.
//!
//! HIR lowering preserves language-level virtual/interface dispatch contracts.
//! This pass is the single ordinary optimization owner that rewrites exact
//! receiver dispatch calls into direct calls on canonical pass artifacts.

use std::collections::HashMap;

use crate::span::Span;
use crate::ty::TypeStore;

use super::pass_pipeline::MirPassPipelineContext;
use super::{
    BasicBlockId, CallArg, CallKind, DispatchMetadata, FunDecl, InstanceKey, MaterializedMir,
    Operand, Rvalue, StatementKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DispatchDevirtualizationTargetKey {
    caller_fqn: String,
    block: BasicBlockId,
    span: Span,
    target_fqn: String,
}

impl DispatchDevirtualizationTargetKey {
    pub(crate) fn new(
        caller_fqn: impl Into<String>,
        block: BasicBlockId,
        span: Span,
        target_fqn: impl Into<String>,
    ) -> Self {
        Self {
            caller_fqn: caller_fqn.into(),
            block,
            span,
            target_fqn: target_fqn.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DispatchDevirtualizationFacts {
    known_receiver_subclasses: crate::devirtualize::KnownReceiverSubclassIndex,
    class_vtables: crate::vtable::ClassVtableIndex,
    interfaces: crate::itable::InterfaceIndex,
    class_itables: crate::itable::ClassItableIndex,
    canonical_targets_by_site: HashMap<DispatchDevirtualizationTargetKey, String>,
    canonical_targets_by_fqn: HashMap<String, String>,
}

impl DispatchDevirtualizationFacts {
    pub(crate) fn new(
        known_receiver_subclasses: crate::devirtualize::KnownReceiverSubclassIndex,
        class_vtables: crate::vtable::ClassVtableIndex,
        interfaces: crate::itable::InterfaceIndex,
        class_itables: crate::itable::ClassItableIndex,
        canonical_targets_by_site: HashMap<DispatchDevirtualizationTargetKey, String>,
        canonical_targets_by_fqn: HashMap<String, String>,
    ) -> Self {
        Self {
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            canonical_targets_by_site,
            canonical_targets_by_fqn,
        }
    }

    fn target_for_dispatch(&self, lookup: DispatchTargetLookup<'_>) -> Option<String> {
        let target = crate::devirtualize::try_devirtualize_dispatch_target(
            lookup.kind,
            &lookup.dispatch.owner_fqn,
            &lookup.dispatch.member_name,
            lookup.explicit_arg_count,
            lookup.dispatch.receiver_ty,
            lookup.types,
            crate::devirtualize::DispatchTargetFacts {
                known_receiver_subclasses: &self.known_receiver_subclasses,
                class_vtables: &self.class_vtables,
                interfaces: &self.interfaces,
                class_itables: &self.class_itables,
            },
        )?;
        if target.is_empty() {
            return None;
        }
        let site_key = DispatchDevirtualizationTargetKey::new(
            lookup.caller_fqn,
            lookup.block,
            lookup.span,
            &target,
        );
        Some(
            self.canonical_targets_by_site
                .get(&site_key)
                .or_else(|| self.canonical_targets_by_fqn.get(&target))
                .cloned()
                .unwrap_or(target),
        )
    }
}

struct DispatchTargetLookup<'a> {
    kind: crate::hir::DispatchCallKind,
    caller_fqn: &'a str,
    block: BasicBlockId,
    span: Span,
    dispatch: &'a DispatchMetadata,
    explicit_arg_count: usize,
    types: &'a TypeStore,
}

struct DevirtualizationFunction {
    key: InstanceKey,
    fun: FunDecl,
}

struct DevirtualizationSnapshot<'a> {
    materialized: &'a MaterializedMir,
    functions: Vec<DevirtualizationFunction>,
    caller_candidates: Vec<FunDecl>,
}

/// Rewrite exact-receiver virtual/interface calls into direct calls on pass artifacts.
pub(crate) fn run_dispatch_devirtualization(context: &mut MirPassPipelineContext<'_>) {
    let (instance_rewrites, caller_rewrites) = {
        let snapshot = DevirtualizationSnapshot::from_materialized(context.materialized());
        let mut instance_rewrites = Vec::new();
        let mut caller_rewrites = Vec::new();

        for function in &snapshot.functions {
            if let Some(rewritten) = rewrite_callable_body_once(&function.fun, &snapshot) {
                instance_rewrites.push((function.key.clone(), rewritten));
            }
        }

        for fun in &snapshot.caller_candidates {
            if let Some(rewritten) = rewrite_callable_body_once(fun, &snapshot) {
                caller_rewrites.push(rewritten);
            }
        }

        (instance_rewrites, caller_rewrites)
    };

    for (key, fun) in instance_rewrites {
        context.publish_instance_rewrite(key, fun);
    }
    for fun in caller_rewrites {
        context.publish_caller_rewrite(fun);
    }
}

impl<'a> DevirtualizationSnapshot<'a> {
    fn from_materialized(materialized: &'a MaterializedMir) -> Self {
        let pass_view = materialized.pass_view();
        let functions = pass_view
            .instances()
            .filter_map(|family| {
                let fun = family.root_body()?.clone();
                Some(DevirtualizationFunction {
                    key: family.key().clone(),
                    fun,
                })
            })
            .collect::<Vec<_>>();
        let caller_candidates = materialized
            .caller_side_pass_candidate_bodies()
            .iter()
            .filter_map(|raw_fun| {
                if pass_view.owner_of_callable(&raw_fun.fqn).is_some() {
                    return None;
                }
                if pass_view.callable_body_is_overridden(&raw_fun.fqn) {
                    return pass_view.callable(&raw_fun.fqn).cloned();
                }
                Some(raw_fun.clone())
            })
            .collect::<Vec<_>>();
        Self {
            materialized,
            functions,
            caller_candidates,
        }
    }

    fn target_for_virtual(
        &self,
        caller_fqn: &str,
        block: BasicBlockId,
        span: Span,
        dispatch: &DispatchMetadata,
        arg_count: usize,
    ) -> Option<String> {
        self.materialized
            .dispatch_devirtualization_facts()
            .target_for_dispatch(DispatchTargetLookup {
                kind: crate::hir::DispatchCallKind::Virtual,
                caller_fqn,
                block,
                span,
                dispatch,
                explicit_arg_count: arg_count,
                types: &self.materialized.types,
            })
    }

    fn target_for_interface(
        &self,
        caller_fqn: &str,
        block: BasicBlockId,
        span: Span,
        dispatch: &DispatchMetadata,
        arg_count: usize,
    ) -> Option<String> {
        self.materialized
            .dispatch_devirtualization_facts()
            .target_for_dispatch(DispatchTargetLookup {
                kind: crate::hir::DispatchCallKind::Interface,
                caller_fqn,
                block,
                span,
                dispatch,
                explicit_arg_count: arg_count,
                types: &self.materialized.types,
            })
    }
}

fn rewrite_callable_body_once(
    fun: &FunDecl,
    snapshot: &DevirtualizationSnapshot<'_>,
) -> Option<FunDecl> {
    let mut rewritten = fun.clone();
    let body = rewritten.body.as_mut()?;
    let mut changed = false;

    for (block_index, block) in body.blocks.iter_mut().enumerate() {
        let block_id = BasicBlockId::from_raw(block_index as u32);
        for stmt in &mut block.stmts {
            let StatementKind::Assign { value, .. } = &mut stmt.kind else {
                continue;
            };
            let Rvalue::Call { kind, args, .. } = value else {
                continue;
            };

            let rewrite = match kind {
                CallKind::Virtual { receiver, dispatch } => snapshot
                    .target_for_virtual(&fun.fqn, block_id, stmt.span, dispatch, args.len())
                    .map(|target| (receiver.clone(), target)),
                CallKind::Interface { receiver, dispatch } => snapshot
                    .target_for_interface(&fun.fqn, block_id, stmt.span, dispatch, args.len())
                    .map(|target| (receiver.clone(), target)),
                _ => None,
            };

            let Some((receiver, target_fqn)) = rewrite else {
                continue;
            };
            *args = dispatch_direct_call_args(stmt.span, &receiver, args);
            *kind = CallKind::Direct {
                callee_fqn: target_fqn,
            };
            changed = true;
        }
    }

    changed.then_some(rewritten)
}

fn dispatch_direct_call_args(
    call_span: crate::span::Span,
    receiver: &Operand,
    args: &[CallArg],
) -> Vec<CallArg> {
    let mut direct_args = Vec::with_capacity(args.len() + 1);
    direct_args.push(CallArg {
        span: call_span,
        name: None,
        value: receiver.clone(),
    });
    direct_args.extend(args.iter().cloned());
    direct_args
}
