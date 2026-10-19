//! Require every reference use to have an explicit value replacement.

use super::{
    control::ControlFlow,
    index, mir,
    origins::{
        AllocationId, AllocationKind, Projection, Site, Sources, local_alias, statement_alias,
    },
};

pub(super) enum Replacement {
    Value {
        candidate: AllocationId,
        projection: Projection,
    },
    Boolean(bool),
}

pub(super) struct Plan {
    pub sources: Sources,
    pub removable: Vec<bool>,
    control: ControlFlow,
}

impl Plan {
    pub fn new(module: &mir::Module, function: &mir::Function) -> Self {
        let control = ControlFlow::new(&function.body);
        let sources = Sources::new(function, &control);
        let mut removable = vec![true; sources.allocations.len()];
        for origins in &sources.locals {
            if origins.unique().is_none() {
                for &id in &origins.candidates {
                    removable[index(id)] = false;
                }
            }
        }
        for id in control.loop_carried(&function.body, &sources) {
            removable[index(id)] = false;
        }
        let mut plan = Self {
            sources,
            removable,
            control,
        };
        let mut reads = vec![0_usize; plan.removable.len()];
        let mut replaced = reads.clone();
        for (id, block) in function.body.blocks.iter() {
            if !plan.control.reachable(id) {
                continue;
            }
            for (statement, node) in block.statements.iter().enumerate() {
                let site = Site {
                    block: id,
                    statement,
                };
                if let Some(local) = statement_alias(node) {
                    for &candidate in &plan.sources.locals[index(local)].candidates {
                        replaced[index(candidate)] += 1;
                    }
                }
                mir::visit_statement_exprs(node, &mut |expr| {
                    plan.count(module, expr, site, &mut reads, &mut replaced);
                });
            }
            let site = Site {
                block: id,
                statement: block.statements.len(),
            };
            mir::visit_terminator_exprs(&block.terminator, &mut |expr| {
                plan.count(module, expr, site, &mut reads, &mut replaced);
            });
        }
        for (id, removable) in plan.removable.iter_mut().enumerate() {
            *removable &= reads[id] == replaced[id];
        }
        plan
    }

    fn count(
        &mut self,
        module: &mir::Module,
        expr: &mir::Expr,
        site: Site,
        reads: &mut [usize],
        replaced: &mut [usize],
    ) {
        if let Some((candidate, _)) = self.replacement(module, expr, site) {
            replaced[index(candidate)] += 1;
        }
        if let mir::ExprKind::Local(local) = expr.kind {
            for &candidate in &self.sources.locals[index(local)].candidates {
                reads[index(candidate)] += 1;
                self.removable[index(candidate)] &= self
                    .control
                    .dominates(self.sources.allocations[candidate].site, site);
            }
        }
    }

    pub fn replacement(
        &self,
        module: &mir::Module,
        expr: &mir::Expr,
        site: Site,
    ) -> Option<(AllocationId, Replacement)> {
        let operand = match &expr.kind {
            mir::ExprKind::Unbox(operand) | mir::ExprKind::IsInstance { operand, .. } => operand,
            mir::ExprKind::ClosureCapture { closure, .. } => closure,
            _ => return None,
        };
        let candidate = self.sources.locals[index(local_alias(operand)?)].unique()?;
        let creation = &self.sources.allocations[candidate];
        if !self.control.dominates(creation.site, site) {
            return None;
        }
        let replacement = match (&creation.kind, &expr.kind) {
            (AllocationKind::Box(payload), mir::ExprKind::Unbox(_)) if expr.ty == payload.ty => {
                Replacement::Value {
                    candidate,
                    projection: Projection::BoxPayload,
                }
            }
            (AllocationKind::Box(payload), mir::ExprKind::IsInstance { check_ty, .. }) => {
                Replacement::Boolean(instance_test(module, &payload.ty, check_ty)?)
            }
            (
                AllocationKind::Closure { class, captures },
                mir::ExprKind::ClosureCapture {
                    class: expected,
                    index,
                    ..
                },
            ) if class == expected
                && captures
                    .iter()
                    .any(|capture| capture.field() == *index && capture.value().ty == expr.ty) =>
            {
                Replacement::Value {
                    candidate,
                    projection: Projection::Capture(*index),
                }
            }
            (AllocationKind::Closure { class, .. }, mir::ExprKind::IsInstance { check_ty, .. })
                if **check_ty == mir::Type::Any
                    || **check_ty
                        == mir::Type::Function(module.closure_classes[*class].function_type) =>
            {
                Replacement::Boolean(true)
            }
            _ => return None,
        };
        Some((candidate, replacement))
    }
}

/// Reuse concrete box descriptors, their parent chain and complete itables.
/// In particular, an integral enum's parent integer is not its exact payload.
fn instance_test(module: &mir::Module, payload: &mir::Type, target: &mir::Type) -> Option<bool> {
    if target == payload || *target == mir::Type::Any {
        return Some(true);
    }
    let source = module
        .meta
        .boxed_types
        .iter()
        .find(|boxed| boxed.payload() == payload)?
        .class();
    let target = match target {
        mir::Type::Interface(interface) => {
            return Some(
                module.classes[source]
                    .itables
                    .iter()
                    .any(|table| table.interface == *interface),
            );
        }
        mir::Type::Class(class) => *class,
        mir::Type::String | mir::Type::Function(_) => return Some(false),
        _ => module
            .meta
            .boxed_types
            .iter()
            .find(|boxed| boxed.payload() == target)?
            .class(),
    };
    let mut source = Some(source);
    while let Some(class) = source {
        if class == target {
            return Some(true);
        }
        source = module.classes[class].base_class();
    }
    Some(false)
}
