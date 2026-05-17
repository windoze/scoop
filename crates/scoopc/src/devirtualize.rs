use std::collections::HashSet;

use crate::hir;
use crate::itable::InterfaceIndex;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use crate::vtable::ClassVtableIndex;

pub(crate) type KnownReceiverSubclassIndex = HashSet<String>;

pub(crate) fn collect_known_receiver_subclasses(
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

pub(crate) struct DispatchTargetFacts<'a> {
    pub(crate) known_receiver_subclasses: &'a KnownReceiverSubclassIndex,
    pub(crate) class_vtables: &'a ClassVtableIndex,
    pub(crate) interfaces: &'a InterfaceIndex,
    pub(crate) class_itables: &'a crate::itable::ClassItableIndex,
}

pub(crate) fn try_devirtualize_dispatch_target(
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
