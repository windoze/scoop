use super::*;

impl<'a> Traversal<'a, '_> {
    pub(super) fn statement(&mut self, statement: &'a Statement) -> Result<(), StructureError> {
        match &statement.kind {
            StatementKind::Expr(value) | StatementKind::Throw(value) => {
                self.push(Item::Expression(value))
            }
            StatementKind::Return { value } => {
                if let Some(value) = value {
                    self.push(Item::Expression(value))?;
                }
                Ok(())
            }
            StatementKind::ValDecl { pattern, init } => {
                self.push(Item::Expression(init))?;
                self.push(Item::Pattern(pattern))
            }
            StatementKind::Assign { target, value } => {
                self.push(Item::Expression(value))?;
                match target {
                    AssignTarget::Index { array, index } => {
                        self.push(Item::Expression(index))?;
                        self.push(Item::Expression(array))
                    }
                    AssignTarget::Field { receiver, .. } => self.push(Item::Expression(receiver)),
                    AssignTarget::Local(_)
                    | AssignTarget::Global(_)
                    | AssignTarget::SingletonPublishedRoot(_) => Ok(()),
                }
            }
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                if let Some(body) = else_body {
                    self.statements(body)?;
                }
                self.statements(then_body)?;
                self.push(Item::Expression(cond))
            }
            StatementKind::While {
                condition_setup,
                cond,
                body,
                ..
            } => {
                self.statements(body)?;
                self.push(Item::Expression(cond))?;
                self.statements(condition_setup)
            }
            StatementKind::When(when) => {
                if let WhenFallback::Else(body) = &when.fallback {
                    self.statements(body)?;
                }
                for arm in when.arms.iter().rev() {
                    self.statements(&arm.body)?;
                    if let Some(guard) = &arm.guard {
                        self.push(Item::Expression(&guard.condition))?;
                        self.statements(&guard.setup)?;
                    }
                    self.push(Item::Pattern(&arm.pattern))?;
                }
                self.push(Item::Expression(&when.subject))
            }
            StatementKind::Try(value) => {
                if let Some(body) = &value.finally_body {
                    self.statements(body)?;
                }
                for catch in value.catches.iter().rev() {
                    self.statements(&catch.body)?;
                }
                self.statements(&value.body)
            }
            StatementKind::InitializationEnsure(_)
            | StatementKind::LocalFunction(_)
            | StatementKind::Break { .. }
            | StatementKind::Continue { .. } => Ok(()),
        }
    }

    pub(super) fn pattern(&mut self, pattern: &'a Pattern) -> Result<(), StructureError> {
        match pattern {
            Pattern::Binding { .. } | Pattern::Wildcard => Ok(()),
            Pattern::Literal { value, .. } => self.push(Item::Expression(value)),
            Pattern::Variant { fields, .. } | Pattern::Struct { fields, .. } => {
                for (_, value) in fields.iter().rev() {
                    self.push(Item::Pattern(value))?;
                }
                Ok(())
            }
            Pattern::Tuple(values) => {
                for value in values.iter().rev() {
                    self.push(Item::Pattern(value))?;
                }
                Ok(())
            }
        }
    }
}
