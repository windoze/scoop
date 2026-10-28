//! Collapse redundant interface views before physical table lookup lowering.

use std::collections::HashSet;

use la_arena::Arena;

use super::{graph::index, mir};

mod cleanup;
mod state;

pub(super) fn run(module: &mut mir::Module, active: &[bool]) -> Vec<bool> {
    let mut changed = vec![false; active.len()];
    for (id, function) in module.functions.iter_mut() {
        if !active[index(id)] {
            continue;
        }
        let mut addressed = HashSet::new();
        let mut has_views = false;
        for (_, block) in function.body.blocks.iter() {
            mir::visit_block_exprs(block, &mut |expression| {
                if let mir::ExprKind::AddressOf { local, .. } = expression.kind {
                    addressed.insert(local);
                }
                has_views |= matches!(expression.kind, mir::ExprKind::Retype { .. })
                    && matches!(expression.ty, mir::Type::Interface(_));
            });
        }
        if !has_views {
            continue;
        }
        let mut rewriter = Rewriter {
            locals: &function.body.locals,
            available: state::Available::default(),
            addressed: &addressed,
            changed: false,
        };
        for (_, block) in function.body.blocks.iter_mut() {
            rewriter.available = state::Available::default();
            for statement in &mut block.statements {
                mir::visit_statement_exprs_mut(statement, &mut |expression| {
                    rewriter.expression(expression);
                });
                rewriter
                    .available
                    .statement(statement, rewriter.locals, rewriter.addressed);
            }
            mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expression| {
                rewriter.expression(expression);
            });
        }
        changed[index(id)] = rewriter.changed;
        changed[index(id)] |= cleanup::unused(&mut function.body, &addressed);
    }
    changed
}

struct Rewriter<'a> {
    locals: &'a Arena<mir::Local>,
    available: state::Available,
    addressed: &'a HashSet<mir::LocalId>,
    changed: bool,
}

impl Rewriter<'_> {
    fn expression(&mut self, expression: &mut mir::Expr) {
        if !view_type(&expression.ty) {
            return;
        }
        let mir::ExprKind::Retype { operand, .. } = &mut expression.kind else {
            return;
        };
        // An existing interface carries a table that can project directly to
        // its parents. Erasing that view would reintroduce a dynamic lookup.
        let mut source = operand.as_ref();
        while source.ty == mir::Type::Any
            && let mir::ExprKind::Retype { operand: inner, .. } = &source.kind
            && reference_type(&inner.ty)
        {
            source = inner;
        }
        if !std::ptr::eq(source, operand.as_ref()) {
            **operand = source.clone();
            self.changed = true;
        }
        if let Some(root) = self.available.source(operand, self.addressed)
            && !matches!(operand.kind, mir::ExprKind::Local(local) if local == root)
        {
            **operand = mir::Expr::local(root, self.locals[root].ty.clone());
            self.changed = true;
        }
        if expression.ty == operand.ty {
            *expression = (**operand)
                .clone()
                .with_debug_origin(expression.debug_origin.clone());
            self.changed = true;
        }
    }
}

fn view_type(ty: &mir::Type) -> bool {
    matches!(ty, mir::Type::Any | mir::Type::Interface(_))
}

fn reference_type(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::Any
            | mir::Type::Interface(_)
            | mir::Type::Class(_)
            | mir::Type::String
            | mir::Type::Function(_)
    )
}

fn pure_local(expression: &mir::Expr) -> Option<mir::LocalId> {
    if !reference_type(&expression.ty) {
        return None;
    }
    match &expression.kind {
        mir::ExprKind::Local(local) => Some(*local),
        mir::ExprKind::Retype { operand, .. } if expression.ty == mir::Type::Any => {
            pure_local(operand)
        }
        _ => None,
    }
}
