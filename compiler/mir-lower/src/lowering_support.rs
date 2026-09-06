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

/// Resolve one already-lowered core `Option` by exact enum identity. Shape is
/// deliberately irrelevant: a user enum with the same fields is not Option.
pub(super) fn option_core_for_type(
    module: &hir::Module,
    enums: &EnumRegistry,
    ty: &mir::Type,
) -> mir::OptionCore {
    let mir::Type::Enum(enum_id, _) = ty else {
        unreachable!("core Option values have an enum type")
    };
    enums
        .option_core(module, *enum_id)
        .expect("local-concrete HIR marks every core Option specialization")
}

/// The representation-independent identities needed to consume `Some`.
/// Raw declaration indices are accepted only at this enum-store boundary;
/// expression lowering receives checked, inseparable variant/field refs.
#[derive(Clone, Copy)]
pub(super) struct OptionSomeRefs {
    pub(super) variant: mir::MirVariantRef,
    pub(super) payload: mir::MirVariantFieldRef,
}

pub(super) fn option_some_refs_for_type(
    module: &hir::Module,
    enums: &EnumRegistry,
    ty: &mir::Type,
) -> OptionSomeRefs {
    let option = option_core_for_type(module, enums, ty);
    let variant = enums.variant_ref(option.enum_id(), option.some_variant());
    let payload = enums.variant_field_ref(variant, 0);
    OptionSomeRefs { variant, payload }
}
