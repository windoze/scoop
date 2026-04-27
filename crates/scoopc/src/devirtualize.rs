use std::collections::HashSet;

use crate::hir;
use crate::itable::InterfaceIndex;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};
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
    let receiver_fqn = exact_receiver_fqn(receiver_ty, types, facts.known_receiver_subclasses)?;

    match kind {
        hir::DispatchCallKind::Virtual => {
            if let Some(slots) = facts.class_vtables.get(receiver_fqn)
                && let Some(slot) = slots.iter().find(|slot| {
                    slot.name == member_name && slot.params_len == explicit_arg_count as u32
                })
            {
                return Some(slot.impl_member_fqn.clone());
            }

            (receiver_fqn == owner_fqn).then(|| format!("{owner_fqn}.{member_name}"))
        }
        hir::DispatchCallKind::Interface => {
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
                .get(receiver_fqn)?
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

fn exact_receiver_fqn<'a>(
    receiver_ty: TypeId,
    types: &'a TypeStore,
    known_receiver_subclasses: &KnownReceiverSubclassIndex,
) -> Option<&'a str> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
        return None;
    };
    if known_receiver_subclasses.contains(&nominal.fqn) {
        return None;
    }
    Some(nominal.fqn.as_str())
}
