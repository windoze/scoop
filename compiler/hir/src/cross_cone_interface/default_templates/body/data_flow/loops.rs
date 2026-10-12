use super::*;
use crate::{DefaultExpressionV1, DefaultStatementV1, LoopKind};

impl Validator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_loop(
        &mut self,
        kind: LoopKind,
        condition_setup: &[DefaultStatementV1],
        condition: &DefaultExpressionV1,
        body: &[DefaultStatementV1],
        available: Vec<bool>,
        reachable: bool,
        region: Region,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError> {
        let incoming = self.copy_bits(&available)?;
        let loop_depth = region.loop_depth.checked_add(1).ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::Resource(WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        let child = Region { loop_depth };
        let mut condition_input = Flow::falling_through(available);
        let mut abrupt = AbruptOutcomes::default();
        let mut body_breaks = false;
        if kind == LoopKind::DoWhile {
            let mut body_flow =
                self.validate_statements(body, condition_input, reachable, child)?;
            body_breaks = body_flow
                .abrupt
                .remove(&ControlOutcome::Break(region.loop_depth))
                .is_some();
            let continues = body_flow
                .abrupt
                .remove(&ControlOutcome::Continue(region.loop_depth));
            if let Some(available) = continues {
                if body_flow.falls_through {
                    self.intersect_bits(&mut body_flow.available, &available)?;
                } else {
                    body_flow.available = available;
                    body_flow.falls_through = true;
                }
            }
            abrupt = std::mem::take(&mut body_flow.abrupt);
            condition_input = body_flow;
        }
        let reaches_condition = reachable && condition_input.falls_through;
        let mut condition_flow =
            self.validate_statements(condition_setup, condition_input, reaches_condition, child)?;
        self.validate_expression(
            condition,
            &condition_flow.available,
            reaches_condition && condition_flow.falls_through,
        )?;
        let setup_breaks = condition_flow
            .abrupt
            .remove(&ControlOutcome::Break(region.loop_depth))
            .is_some();
        condition_flow
            .abrupt
            .remove(&ControlOutcome::Continue(region.loop_depth));
        if kind == LoopKind::While {
            let body_available = self.copy_bits(&condition_flow.available)?;
            let mut body_flow = self.validate_statements(
                body,
                Flow::falling_through(body_available),
                reachable && condition_flow.falls_through,
                child,
            )?;
            body_breaks = body_flow
                .abrupt
                .remove(&ControlOutcome::Break(region.loop_depth))
                .is_some();
            body_flow
                .abrupt
                .remove(&ControlOutcome::Continue(region.loop_depth));
            self.merge_abrupt_outcomes(&mut abrupt, body_flow.abrupt)?;
        }
        self.merge_abrupt_outcomes(&mut abrupt, condition_flow.abrupt)?;
        Ok(Flow {
            available: incoming,
            falls_through: reachable
                && (condition_flow.falls_through || setup_breaks || body_breaks),
            abrupt,
        })
    }
}
