//! Stable occurrences borrowed directly from materialized executable bodies.

use scoop_wire::{WireError, WirePath};

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

        mut visitor: impl FnMut(ExecutableExpressionOccurrence<'a>) -> Result<(), E>,
    ) -> Result<(), ExecutableExpressionVisitError<E>> {
        let roots = roots::collect(self)?;
        for (root, body) in roots {
            let mut traversal = Traversal::new(self);
            body.schedule(&mut traversal)?;
            let mut expression_index = 0_u32;
            while let Some(item) = traversal.pending.pop() {
                match item {
                    Item::Expression(expression) => {
                        let position = ExecutableExpressionPosition {
                            root,
                            expression_index,
                        };
                        expression_index = expression_index
                            .checked_add(1)
                            .ok_or(StructureError::ExpressionIndexOverflow(root))?;
                        visitor(ExecutableExpressionOccurrence {
                            position,
                            expression,
                        })
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

struct Traversal<'a> {
    module: &'a Module,

    pending: Vec<Item<'a>>,

    path: WirePath,
}

impl<'a> Traversal<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,

            pending: Vec::new(),

            path: WirePath::root(),
        }
    }

    fn push(&mut self, item: Item<'a>) -> Result<(), StructureError> {
        scoop_wire::allocation::try_reserve(&mut self.pending, 1, &self.path)?;
        self.pending.push(item);
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
