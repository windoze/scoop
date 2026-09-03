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
            Some((base, _)) => depth(module, *base) + 1,
            None => 0,
        }
    }
    let mut order: Vec<hir::ClassId> = module.classes.iter().map(|(id, _)| id).collect();
    order.sort_by_key(|&id| depth(module, id));
    order
}

/// The evaluation statements and values initializing `hir_id`'s flattened
/// fields (base prefix first), given `own` — the already materialized values
/// for the class's own constructor parameters. Each delegation owns a local
/// namespace and its source-order evaluation plan.
pub(super) fn flattened_ctor_args(
    lowerer: &mut BodyLowerer,
    module: &hir::Module,
    hir_id: hir::ClassId,
    own: Vec<smir::Expr>,
) -> (Vec<smir::Statement>, Vec<smir::Expr>) {
    let previous_constructor_params = std::mem::take(&mut lowerer.constructor_param_map);
    for (field, value) in module.classes[hir_id]
        .declared_constructor()
        .iter()
        .zip(&own)
    {
        lowerer
            .constructor_param_map
            .insert(field.parameter, value.clone());
    }

    let (statements, mut out) = match module.classes[hir_id].base_class() {
        Some((base, delegation)) => {
            let previous_locals = std::mem::take(&mut lowerer.local_map);
            for (source, local) in delegation.locals.iter() {
                let ty = lowerer.lower_type(local.ty);
                let target = lowerer.locals.alloc(mir::Local {
                    name: local.name.clone(),
                    ty,
                    mutable: local.mutable,
                });
                lowerer.local_map.insert(source, target);
            }
            let statements = lowerer.lower_statements(&delegation.statements);
            let base_own = delegation
                .args
                .iter()
                .map(|expr| lowerer.lower_expr(expr))
                .collect();
            lowerer.local_map = previous_locals;
            let (mut inherited_statements, inherited) =
                flattened_ctor_args(lowerer, module, *base, base_own);
            let mut statements = statements;
            statements.append(&mut inherited_statements);
            (statements, inherited)
        }
        None => (Vec::new(), Vec::new()),
    };
    out.extend(own);
    lowerer.constructor_param_map = previous_constructor_params;
    (statements, out)
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
