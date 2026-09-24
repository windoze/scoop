//! Stable occurrences borrowed directly from materialized executable bodies.

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::*;

mod errors;
mod expressions;
mod roots;
mod statements;
use errors::ExecutableExpressionStructureError as StructureError;
pub use errors::{ExecutableExpressionStructureError, ExecutableExpressionVisitError};

/// The ordinal is a structural position within this exact materialization,
/// independent of declaration/arena order. It is not a persistent entity id.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExecutableExpressionPosition {
    pub root: CallableMaterialization,
    pub expression_index: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ExecutableExpressionOccurrence<'a> {
    pub position: ExecutableExpressionPosition,
    pub expression: &'a Expr,
}

impl Module {
    /// Visits every expression in each materialized body, in structural
    /// preorder. Closure captures are evaluated in their enclosing body;
    /// their function bodies have their own materialization roots.
    pub fn visit_executable_expressions<'a, E>(
        &'a self,
        meter: &mut BudgetMeter,
        mut visitor: impl FnMut(ExecutableExpressionOccurrence<'a>, &mut BudgetMeter) -> Result<(), E>,
    ) -> Result<(), ExecutableExpressionVisitError<E>> {
        let roots = roots::collect(self, meter)?;
        for (root, body) in roots {
            let mut traversal = Traversal::new(self, meter);
            body.schedule(&mut traversal)?;
            let mut expression_index = 0_u32;
            while let Some((item, depth)) = traversal.pending.pop() {
                traversal
                    .meter
                    .check_semantic_depth(depth, &traversal.path)?;
                traversal.meter.charge_work(1, &traversal.path)?;
                traversal.meter.charge_nodes(1, &traversal.path)?;
                traversal.depth = depth;
                match item {
                    Item::Expression(expression) => {
                        let position = ExecutableExpressionPosition {
                            root,
                            expression_index,
                        };
                        expression_index = expression_index
                            .checked_add(1)
                            .ok_or(StructureError::ExpressionIndexOverflow(root))?;
                        visitor(
                            ExecutableExpressionOccurrence {
                                position,
                                expression,
                            },
                            traversal.meter,
                        )
                        .map_err(ExecutableExpressionVisitError::Visitor)?;
                        traversal.expression(expression)?;
                    }
                    Item::Statement(statement) => traversal.statement(statement)?,
                    Item::Pattern(pattern) => traversal.pattern(pattern)?,
                }
            }
        }
        Ok(())
    }
}

enum Item<'a> {
    Expression(&'a Expr),
    Statement(&'a Statement),
    Pattern(&'a Pattern),
}

struct Traversal<'a, 'm> {
    module: &'a Module,
    meter: &'m mut BudgetMeter,
    pending: Vec<(Item<'a>, u64)>,
    depth: u64,
    path: WirePath,
}

impl<'a, 'm> Traversal<'a, 'm> {
    fn new(module: &'a Module, meter: &'m mut BudgetMeter) -> Self {
        Self {
            module,
            meter,
            pending: Vec::new(),
            depth: 0,
            path: WirePath::root(),
        }
    }

    fn push(&mut self, item: Item<'a>) -> Result<(), StructureError> {
        let depth = self
            .depth
            .checked_add(1)
            .ok_or(StructureError::DepthOverflow)?;
        self.meter.check_semantic_depth(depth, &self.path)?;
        self.meter.charge_edges(1, &self.path)?;
        self.meter
            .charge_owned_bytes(std::mem::size_of::<(Item<'_>, u64)>() as u64, &self.path)?;
        self.meter
            .try_reserve_collection_slots(&mut self.pending, 1, &self.path)?;
        self.pending.push((item, depth));
        Ok(())
    }

    fn expressions(&mut self, values: &'a [Expr]) -> Result<(), StructureError> {
        for value in values.iter().rev() {
            self.push(Item::Expression(value))?;
        }
        Ok(())
    }

    fn statements(&mut self, values: &'a [Statement]) -> Result<(), StructureError> {
        for value in values.iter().rev() {
            self.push(Item::Statement(value))?;
        }
        Ok(())
    }

    fn captures(&mut self, captures: &'a [Capture]) -> Result<(), StructureError> {
        for capture in captures.iter().rev() {
            self.push(Item::Expression(&capture.source))?;
        }
        Ok(())
    }
}
