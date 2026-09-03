use super::*;

pub(super) fn lower_gc_effect(effect: hir::GcEffect) -> mir::GcEffect {
    match effect {
        hir::GcEffect::Managed => mir::GcEffect::Managed,
        hir::GcEffect::NoGc => mir::GcEffect::NoGc,
    }
}

/// Whether a function is an abstract class method. HIR carries this
/// explicitly, including for `Unit`-returning methods.
pub(super) fn is_abstract_bodiless(function: &hir::Function) -> bool {
    function
        .method
        .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
}

/// Class ids (HIR) ordered base-before-derived (single inheritance:
/// depth in the base chain; ties keep declaration order).
pub(super) fn topo_class_order(module: &hir::Module) -> Vec<hir::ClassId> {
    fn depth(module: &hir::Module, id: hir::ClassId) -> usize {
        match module.classes[id].base_class() {
            Some(base) => depth(module, base) + 1,
            None => 0,
        }
    }
    let mut order: Vec<hir::ClassId> = module.classes.iter().map(|(id, _)| id).collect();
    order.sort_by_key(|&id| depth(module, id));
    order
}

/// `mir::TableSlot` is not `Clone`; both payloads are `Copy`.
pub(super) fn clone_slots(slots: &[mir::TableSlot]) -> Vec<mir::TableSlot> {
    slots
        .iter()
        .map(|slot| match slot {
            mir::TableSlot::Function(id) => mir::TableSlot::Function(*id),
            mir::TableSlot::Runtime(function) => mir::TableSlot::Runtime(*function),
        })
        .collect()
}

/// `mir::Field` is not `Clone`.
pub(super) fn clone_fields(fields: &[mir::Field]) -> Vec<mir::Field> {
    fields
        .iter()
        .map(|field| mir::Field {
            name: field.name.clone(),
            ty: field.ty.clone(),
        })
        .collect()
}

/// The declaration indices of `Option`'s `Some` / `None` variants.
/// hir-lower guarantees scoop.core defines a suitable `Option`.
pub(super) fn option_variants(module: &hir::Module) -> (u32, u32) {
    let (some, none) = module.option_variants;
    (some.into_raw(), none.into_raw())
}
