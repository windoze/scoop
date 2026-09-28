use crate::{
    DefaultAssignTargetV1, DefaultPatternV1, DefaultPatternViewV1, DefaultStatementKindV1,
    DefaultStatementV1, DefaultTryV1, DefaultWhenFallbackViewV1, DefaultWhenV1,
    OptionalDefaultStatementListViewV1,
};

use super::{
    AbruptOutcomes, DefaultLocalDataFlowSiteV1, DefaultLoopControlV1, DefinitionOwner,
    ExportDefaultLocalDataFlowValidationError, Flow, Region, Validator,
};

impl<A, E> Validator<'_, A, E>
where
    A: super::DefaultLocalDataFlowSemanticAuthority<E>,
{
    pub(super) fn validate_statements(
        &mut self,
        statements: &[DefaultStatementV1],
        mut flow: Flow,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        for statement in statements {
            let statement_reachable = reachable && flow.falls_through;
            let next = self.validate_statement(
                statement,
                std::mem::take(&mut flow.available),
                statement_reachable,
                region,
            )?;
            if statement_reachable {
                self.merge_abrupt_outcomes(&mut flow.abrupt, next.abrupt)?;
                flow.falls_through = next.falls_through;
            }
            flow.available = next.available;
        }
        if !reachable {
            flow.falls_through = false;
            flow.abrupt = AbruptOutcomes::default();
        }
        Ok(flow)
    }

    fn validate_statement(
        &mut self,
        statement: &DefaultStatementV1,
        mut available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        match statement.kind() {
            DefaultStatementKindV1::Expr(expression) => {
                self.validate_expression(expression, &available, reachable)?;
                Ok(Flow::falling_through(available))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::GenericDelegateEnsure(_) => {
                Ok(Flow::falling_through(available))
            }
            DefaultStatementKindV1::LocalFunction(function) => {
                self.validate_captures(function.captures(), &available, reachable)?;
                Ok(Flow::falling_through(available))
            }
            DefaultStatementKindV1::Return(value) => {
                if let Some(value) = value.as_ref() {
                    self.validate_expression(value, &available, reachable)?;
                }
                self.abrupt_flow(available, DefaultLoopControlV1::Return)
            }
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                self.validate_expression(init, &available, reachable)?;
                self.define_pattern(pattern, &mut available, region.owner)?;
                Ok(Flow::falling_through(available))
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.validate_expression(value, &available, reachable)?;
                self.validate_assignment_target(target, &mut available, reachable, region.owner)?;
                Ok(Flow::falling_through(available))
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                self.validate_expression(condition, &available, reachable)?;
                let child = Region { ..region };
                let then_available = self.copy_bits(&available)?;
                let then_flow = self.validate_statements(
                    then_body,
                    Flow::falling_through(then_available),
                    reachable,
                    child,
                )?;
                let else_flow = match else_body.view() {
                    OptionalDefaultStatementListViewV1::Present(statements) => {
                        let else_available = self.copy_bits(&available)?;
                        self.validate_statements(
                            statements,
                            Flow::falling_through(else_available),
                            reachable,
                            child,
                        )?
                    }
                    OptionalDefaultStatementListViewV1::Absent if reachable => {
                        Flow::falling_through(self.copy_bits(&available)?)
                    }
                    OptionalDefaultStatementListViewV1::Absent => Flow {
                        available: self.copy_bits(&available)?,
                        falls_through: false,
                        abrupt: AbruptOutcomes::default(),
                    },
                };
                self.merge_normal_definitions(available, vec![then_flow, else_flow])
            }
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => self.validate_while(
                condition_setup,
                condition,
                body,
                available,
                reachable,
                region,
            ),
            DefaultStatementKindV1::For(plan) => {
                self.validate_for(plan, available, reachable, region)
            }
            DefaultStatementKindV1::Break | DefaultStatementKindV1::Continue => {
                let control = if matches!(statement.kind(), DefaultStatementKindV1::Break) {
                    DefaultLoopControlV1::Break
                } else {
                    DefaultLoopControlV1::Continue
                };
                if region.loop_depth == 0 {
                    return Err(
                        ExportDefaultLocalDataFlowValidationError::LoopControlOutsideLoop {
                            control,
                        },
                    );
                }
                self.abrupt_flow(available, control)
            }
            DefaultStatementKindV1::When(value) => {
                self.validate_when(value, available, reachable, region)
            }
            DefaultStatementKindV1::Try(value) => {
                self.validate_try(value, available, reachable, region)
            }
            DefaultStatementKindV1::Throw(value) => {
                self.validate_expression(value, &available, reachable)?;
                self.abrupt_flow(available, DefaultLoopControlV1::Throw)
            }
        }
    }

    fn validate_assignment_target(
        &mut self,
        target: &DefaultAssignTargetV1,
        available: &mut [bool],
        reachable: bool,
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        match target {
            DefaultAssignTargetV1::Local { local } => self.require_mutable_assignment(
                local,
                DefaultLocalDataFlowSiteV1::Assignment,
                available,
                owner,
            ),
            DefaultAssignTargetV1::Global { .. }
            | DefaultAssignTargetV1::GenericDelegateStorage(_) => Ok(()),
            DefaultAssignTargetV1::Index { array, index } => {
                self.validate_expression(array, available, reachable)?;
                self.validate_expression(index, available, reachable)
            }
            DefaultAssignTargetV1::Field { receiver, .. } => {
                self.validate_expression(receiver, available, reachable)
            }
        }
    }

    fn define_pattern(
        &mut self,
        root: &DefaultPatternV1,
        available: &mut [bool],
        owner: DefinitionOwner,
    ) -> Result<bool, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut pending = Vec::new();
        scoop_wire::allocation::try_reserve(&mut pending, 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

        pending.push(root);
        let mut irrefutable = true;

        while let Some(pattern) = pending.pop() {
            match pattern.view() {
                DefaultPatternViewV1::Binding { local } => {
                    self.define_local(
                        local,
                        None,
                        None,
                        DefaultLocalDataFlowSiteV1::PatternBinding,
                        available,
                        owner,
                    )?;
                }
                DefaultPatternViewV1::Wildcard => {}
                DefaultPatternViewV1::Literal { .. } => irrefutable = false,
                DefaultPatternViewV1::Variant { fields, .. } => {
                    irrefutable = false;
                    self.push_pattern_fields(&mut pending, fields)?;
                }
                DefaultPatternViewV1::Tuple { elements } => {
                    self.push_patterns(&mut pending, elements)?;
                }
                DefaultPatternViewV1::Struct { fields, .. } => {
                    self.push_pattern_fields(&mut pending, fields)?;
                }
            }
        }
        Ok(irrefutable)
    }

    fn push_patterns<'body>(
        &mut self,
        pending: &mut Vec<&'body DefaultPatternV1>,
        patterns: &'body [DefaultPatternV1],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for pattern in patterns.iter().rev() {
            scoop_wire::allocation::try_reserve(pending, 1, self.path)
                .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

            pending.push(pattern);
        }
        Ok(())
    }

    fn push_pattern_fields<'body>(
        &mut self,
        pending: &mut Vec<&'body DefaultPatternV1>,
        fields: &'body [crate::DefaultPatternFieldV1],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for field in fields.iter().rev() {
            scoop_wire::allocation::try_reserve(pending, 1, self.path)
                .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

            pending.push(field.pattern());
        }
        Ok(())
    }

    fn validate_while(
        &mut self,
        condition_setup: &[DefaultStatementV1],
        condition: &crate::DefaultExpressionV1,
        body: &[DefaultStatementV1],
        available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        let incoming = self.copy_bits(&available)?;
        let loop_depth = region.loop_depth.checked_add(1).ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::Resource(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        let child = Region {
            loop_depth,

            ..region
        };
        let condition_flow = self.validate_statements(
            condition_setup,
            Flow::falling_through(available),
            reachable,
            child,
        )?;
        self.validate_expression(
            condition,
            &condition_flow.available,
            reachable && condition_flow.falls_through,
        )?;
        let body_available = self.copy_bits(&condition_flow.available)?;
        let body_flow = self.validate_statements(
            body,
            Flow::falling_through(body_available),
            reachable && condition_flow.falls_through,
            child,
        )?;

        let mut abrupt = condition_flow.abrupt;
        let setup_breaks = abrupt.take(DefaultLoopControlV1::Break).is_some();
        abrupt.take(DefaultLoopControlV1::Continue);
        if condition_flow.falls_through {
            let mut body_abrupt = body_flow.abrupt;
            body_abrupt.take(DefaultLoopControlV1::Break);
            body_abrupt.take(DefaultLoopControlV1::Continue);
            self.merge_abrupt_outcomes(&mut abrupt, body_abrupt)?;
        }
        Ok(Flow {
            available: incoming,
            falls_through: reachable && (condition_flow.falls_through || setup_breaks),
            abrupt,
        })
    }

    fn validate_when(
        &mut self,
        value: &DefaultWhenV1,
        available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        self.validate_expression(value.subject(), &available, reachable)?;
        let mut paths = Vec::new();
        scoop_wire::allocation::try_reserve(&mut paths, value.arms().len() + 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        let mut guard_abrupt = AbruptOutcomes::default();
        let mut next_reachable = reachable;
        let child = Region { ..region };

        for arm in value.arms() {
            let mut arm_available = self.copy_bits(&available)?;
            let irrefutable =
                self.define_pattern(arm.pattern(), &mut arm_available, region.owner)?;
            let arm_reachable = next_reachable;
            let (body_reachable, can_try_next, body_available, abrupt) =
                if let Some(guard) = arm.guard().as_ref() {
                    let guard_flow = self.validate_statements(
                        guard.setup(),
                        Flow::falling_through(arm_available),
                        arm_reachable,
                        child,
                    )?;
                    self.validate_expression(
                        guard.condition(),
                        &guard_flow.available,
                        arm_reachable && guard_flow.falls_through,
                    )?;
                    (
                        arm_reachable && guard_flow.falls_through,
                        !irrefutable || guard_flow.falls_through,
                        guard_flow.available,
                        guard_flow.abrupt,
                    )
                } else {
                    (
                        arm_reachable,
                        !irrefutable,
                        arm_available,
                        AbruptOutcomes::default(),
                    )
                };
            let body = self.validate_statements(
                arm.body(),
                Flow::falling_through(body_available),
                body_reachable,
                child,
            )?;
            self.merge_abrupt_outcomes(&mut guard_abrupt, abrupt)?;
            if body_reachable {
                paths.push(body);
            }
            next_reachable &= can_try_next;
        }

        match value.fallback().view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                let fallback_available = self.copy_bits(&available)?;
                let fallback = self.validate_statements(
                    statements,
                    Flow::falling_through(fallback_available),
                    next_reachable,
                    child,
                )?;
                if next_reachable {
                    paths.push(fallback);
                }
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { .. }
            | DefaultWhenFallbackViewV1::PatternMatrix { .. }
            | DefaultWhenFallbackViewV1::EnumPatternMatrix { .. } => {}
        }
        let mut merged = self.merge_normal_definitions(available, paths)?;
        self.merge_abrupt_outcomes(&mut merged.abrupt, guard_abrupt)?;
        Ok(merged)
    }

    fn validate_try(
        &mut self,
        value: &DefaultTryV1,
        available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        let child = Region { ..region };
        let mut paths = Vec::new();
        scoop_wire::allocation::try_reserve(&mut paths, value.catches().len() + 1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        let body_available = self.copy_bits(&available)?;
        paths.push(self.validate_statements(
            value.body(),
            Flow::falling_through(body_available),
            reachable,
            child,
        )?);
        for catch in value.catches() {
            let mut catch_available = self.copy_bits(&available)?;
            self.define_local(
                catch.local(),
                Some(catch.value_type()),
                Some(crate::CanonicalBooleanV1::False),
                DefaultLocalDataFlowSiteV1::Catch,
                &mut catch_available,
                region.owner,
            )?;
            paths.push(self.validate_statements(
                catch.body(),
                Flow::falling_through(catch_available),
                reachable,
                child,
            )?);
        }
        let merge_incoming = self.copy_bits(&available)?;
        let mut merged = self.merge_normal_definitions(merge_incoming, paths)?;
        if let OptionalDefaultStatementListViewV1::Present(finally) = value.finally_body().view() {
            let finally_reachable = merged.falls_through || !merged.abrupt.is_empty();
            let completion_input = if finally_reachable {
                match self.completion_intersection(&merged)? {
                    Some(state) => state,
                    None => self.copy_bits(&available)?,
                }
            } else {
                self.copy_bits(&available)?
            };
            let finally_available = self.copy_bits(&completion_input)?;
            let finally = self.validate_statements(
                finally,
                Flow::falling_through(finally_available),
                finally_reachable,
                child,
            )?;
            if finally_reachable {
                if finally.falls_through {
                    self.apply_finally_definitions(
                        &mut merged,
                        &completion_input,
                        &finally.available,
                    )?;
                    self.merge_abrupt_outcomes(&mut merged.abrupt, finally.abrupt)?;
                } else {
                    merged.falls_through = false;
                    merged.abrupt = finally.abrupt;
                }
            }
        }
        Ok(merged)
    }

    fn merge_normal_definitions(
        &mut self,
        incoming: Vec<bool>,
        paths: Vec<Flow>,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut abrupt = AbruptOutcomes::default();
        let mut available = None;
        for flow in paths {
            self.merge_abrupt_outcomes(&mut abrupt, flow.abrupt)?;
            if flow.falls_through {
                match &mut available {
                    None => available = Some(flow.available),
                    Some(current) => self.intersect_bits(current, &flow.available)?,
                }
            }
        }
        let Some(available) = available else {
            return Ok(Flow {
                available: incoming,
                falls_through: false,
                abrupt,
            });
        };
        Ok(Flow {
            available,
            falls_through: true,
            abrupt,
        })
    }

    fn completion_intersection(
        &mut self,
        flow: &Flow,
    ) -> Result<Option<Vec<bool>>, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut intersection = if flow.falls_through {
            Some(self.copy_bits(&flow.available)?)
        } else {
            None
        };
        for state in flow.abrupt.states() {
            match &mut intersection {
                None => intersection = Some(self.copy_bits(state)?),
                Some(current) => self.intersect_bits(current, state)?,
            }
        }
        Ok(intersection)
    }

    fn apply_finally_definitions(
        &mut self,
        flow: &mut Flow,
        input: &[bool],
        output: &[bool],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let mut additions = Vec::new();
        scoop_wire::allocation::try_reserve(&mut additions, output.len(), self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        for (before, after) in input.iter().zip(output) {
            additions.push(!before && *after);
        }
        if flow.falls_through {
            self.apply_additions(&mut flow.available, &additions)?;
        }
        for outcome in [
            DefaultLoopControlV1::Return,
            DefaultLoopControlV1::Throw,
            DefaultLoopControlV1::Break,
            DefaultLoopControlV1::Continue,
        ] {
            if let Some(state) = flow.abrupt.slot_mut(outcome) {
                self.apply_additions(state, &additions)?;
            }
        }
        Ok(())
    }

    fn apply_additions(
        &mut self,
        target: &mut [bool],
        additions: &[bool],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (slot, addition) in target.iter_mut().zip(additions) {
            *slot |= *addition;
        }
        Ok(())
    }
}
