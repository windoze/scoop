//! MIR-owned dispatch devirtualization pass.
//!
//! HIR lowering preserves language-level virtual/interface dispatch contracts.
//! This pass is the single ordinary optimization owner that rewrites exact
//! receiver dispatch calls into direct calls on canonical pass artifacts.

use std::collections::{HashMap, HashSet};

use crate::hir;
use crate::span::Span;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::pass_pipeline::MirPassPipelineContext;
use super::{
    BasicBlockId, CallArg, CallKind, DispatchMetadata, FunDecl, InstanceKey, MaterializedMir,
    Operand, Rvalue, StatementKind,
};

pub type KnownReceiverSubclassIndex = HashSet<String>;

pub fn collect_known_receiver_subclasses(
    direct_supertypes: &hir::DirectSupertypesIndex,
) -> KnownReceiverSubclassIndex {
    let mut out = HashSet::new();
    for super_fqns in direct_supertypes.values() {
        for super_fqn in super_fqns {
            out.insert(super_fqn.clone());
        }
    }
    out
}

struct DispatchTargetFacts<'a> {
    known_receiver_subclasses: &'a KnownReceiverSubclassIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    interfaces: &'a crate::itable::InterfaceIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DispatchDevirtualizationTargetKey {
    caller_fqn: String,
    block: BasicBlockId,
    span: Span,
    target_fqn: String,
}

impl DispatchDevirtualizationTargetKey {
    pub fn new(
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
pub struct DispatchDevirtualizationFacts {
    known_receiver_subclasses: KnownReceiverSubclassIndex,
    class_vtables: crate::vtable::ClassVtableIndex,
    interfaces: crate::itable::InterfaceIndex,
    class_itables: crate::itable::ClassItableIndex,
    canonical_targets_by_site: HashMap<DispatchDevirtualizationTargetKey, String>,
    canonical_targets_by_fqn: HashMap<String, String>,
}

impl DispatchDevirtualizationFacts {
    pub fn new(
        known_receiver_subclasses: KnownReceiverSubclassIndex,
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
        let target = try_devirtualize_dispatch_target(
            lookup.kind,
            &lookup.dispatch.owner_fqn,
            &lookup.dispatch.member_name,
            lookup.explicit_arg_count,
            lookup.dispatch.receiver_ty,
            lookup.types,
            DispatchTargetFacts {
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

    pub fn virtual_method_slot(
        &self,
        dispatch: &DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Option<u32> {
        let mut candidates = self
            .class_vtables
            .get(&dispatch.owner_fqn)?
            .iter()
            .filter(|slot| {
                slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
            });
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first.slot)
    }

    pub fn interface_method_slot(
        &self,
        dispatch: &DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Option<(u64, u32)> {
        let iface = self.interfaces.get(&dispatch.owner_fqn)?;
        let mut candidates = iface.method_slots.iter().filter(|slot| {
            slot.member_fqn == dispatch.member_fqn && slot.params_len == explicit_arg_count as u32
        });
        let first = candidates.next()?;
        candidates
            .next()
            .is_none()
            .then_some((iface.interface_id, first.slot))
    }
}

fn try_devirtualize_dispatch_target(
    kind: hir::DispatchCallKind,
    owner_fqn: &str,
    member_name: &str,
    explicit_arg_count: usize,
    receiver_ty: TypeId,
    types: &TypeStore,
    facts: DispatchTargetFacts<'_>,
) -> Option<String> {
    match kind {
        hir::DispatchCallKind::Virtual => {
            if let Some(owner_slots) = facts.class_vtables.get(owner_fqn)
                && !owner_slots.iter().any(|slot| {
                    slot.name == member_name && slot.params_len == explicit_arg_count as u32
                })
            {
                return Some(format!("{owner_fqn}.{member_name}"));
            }

            let receiver_fqn =
                exact_receiver_fqn(receiver_ty, types, facts.known_receiver_subclasses)?;
            if let Some(slots) = facts.class_vtables.get(receiver_fqn.as_str())
                && let Some(slot) = slots.iter().find(|slot| {
                    slot.name == member_name && slot.params_len == explicit_arg_count as u32
                })
            {
                return Some(slot.impl_member_fqn.clone());
            }

            (receiver_fqn == owner_fqn).then(|| format!("{owner_fqn}.{member_name}"))
        }
        hir::DispatchCallKind::Interface => {
            let receiver_fqn =
                exact_receiver_fqn(receiver_ty, types, facts.known_receiver_subclasses)?;
            let iface = facts.interfaces.get(owner_fqn)?;
            let mut slots = iface.method_slots.iter().filter(|slot| {
                slot.name == member_name && slot.params_len == explicit_arg_count as u32
            });
            let slot = slots.next()?;
            if slots.next().is_some() {
                return None;
            }

            let targets = facts
                .class_itables
                .get(receiver_fqn.as_str())?
                .iter()
                .filter(|entry| entry.interface_fqn == owner_fqn)
                .filter_map(|entry| entry.method_impl_fqns.get(slot.slot as usize).cloned())
                .collect::<HashSet<_>>();
            if targets.len() != 1 {
                return None;
            }
            targets.into_iter().next()
        }
    }
}

fn exact_receiver_fqn(
    receiver_ty: TypeId,
    types: &TypeStore,
    known_receiver_subclasses: &KnownReceiverSubclassIndex,
) -> Option<String> {
    match types.kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            if known_receiver_subclasses.contains(&nominal.fqn) {
                return None;
            }
            Some(nominal.fqn.clone())
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.clone()),
        TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool".to_string()),
        TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char".to_string()),
        TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64".to_string()),
        TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32".to_string()),
        TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int".to_string()),
        TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt".to_string()),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(format!("scoop.core.Int{bits}")),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(format!("scoop.core.UInt{bits}")),
        _ => None,
    }
}

struct DispatchTargetLookup<'a> {
    kind: hir::DispatchCallKind,
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
                stable_template_key: None,
                stable_instance_key: None,
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
