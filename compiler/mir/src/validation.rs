use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirVariantOperation {
    Test,
    PayloadProject,
}

impl MirVariantOperation {
    const fn name(self) -> &'static str {
        match self {
            Self::Test => "VariantTest",
            Self::PayloadProject => "VariantPayloadProject",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirValidationErrorKind {
    InvalidVariantReference {
        operation: MirVariantOperation,
        error: MirVariantRefError,
    },
    InvalidVariantFieldReference {
        error: MirVariantFieldRefError,
    },
    VariantOperandIsNotEnum {
        operation: MirVariantOperation,
        actual: Type,
    },
    VariantOperandEnumMismatch {
        operation: MirVariantOperation,
        expected: EnumId,
        actual: EnumId,
    },
    VariantTestResultType {
        actual: Type,
    },
    VariantPayloadResultType {
        field: MirVariantFieldRef,
        expected: Type,
        actual: Type,
    },
    VariantPayloadNotDominated {
        field: MirVariantFieldRef,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirValidationError {
    pub function: FunctionId,
    pub block: BlockId,
    pub kind: MirValidationErrorKind,
}

impl std::fmt::Display for MirValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let function = self.function.into_raw().into_u32();
        let block = self.block.into_raw().into_u32();
        write!(
            formatter,
            "invalid MIR in function {function}, block {block}: "
        )?;
        match &self.kind {
            MirValidationErrorKind::InvalidVariantReference { operation, error } => {
                write!(formatter, "{} carries {error}", operation.name())
            }
            MirValidationErrorKind::InvalidVariantFieldReference { error } => {
                write!(formatter, "VariantPayloadProject carries {error}")
            }
            MirValidationErrorKind::VariantOperandIsNotEnum { operation, actual } => write!(
                formatter,
                "{} requires an enum operand, got {actual:?}",
                operation.name()
            ),
            MirValidationErrorKind::VariantOperandEnumMismatch {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "{} expects MIR enum {}, got MIR enum {}",
                operation.name(),
                expected.into_raw().into_u32(),
                actual.into_raw().into_u32()
            ),
            MirValidationErrorKind::VariantTestResultType { actual } => write!(
                formatter,
                "VariantTest result must be Boolean, got {actual:?}"
            ),
            MirValidationErrorKind::VariantPayloadResultType {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "VariantPayloadProject for MIR enum {} variant {} field {} must produce {expected:?}, got {actual:?}",
                field.variant().enum_id().into_raw().into_u32(),
                field.variant().variant_index(),
                field.field_index()
            ),
            MirValidationErrorKind::VariantPayloadNotDominated { field } => write!(
                formatter,
                "VariantPayloadProject for MIR enum {} variant {} field {} is not dominated by the true edge of a matching VariantTest on the same stable value",
                field.variant().enum_id().into_raw().into_u32(),
                field.variant().variant_index(),
                field.field_index()
            ),
        }
    }
}

impl std::error::Error for MirValidationError {}

impl Module {
    pub fn validate(&self) -> Result<(), MirValidationError> {
        validate_module(self)
    }
}

/// Validate the representation-independent MIR enum primitives in every body.
///
/// A matching dominance proof is deliberately tied to an immutable local id,
/// not to printed or structural expression equality. Producers must materialize
/// a tested enum value into such a local before testing and projecting it.
pub fn validate_module(module: &Module) -> Result<(), MirValidationError> {
    for (function_id, function) in module.functions.iter() {
        validate_body(module, function_id, &function.body)?;
    }
    Ok(())
}

fn validate_body(
    module: &Module,
    function: FunctionId,
    body: &Body,
) -> Result<(), MirValidationError> {
    for (block, definition) in body.blocks.iter() {
        try_visit_block_exprs(definition, &mut |expr| {
            validate_variant_shape(module, expr).map_err(|kind| MirValidationError {
                function,
                block,
                kind,
            })
        })?;
    }

    let stable_locals = stable_locals(body);
    let proofs = matching_test_edges(module, body, &stable_locals);
    for (block, definition) in body.blocks.iter() {
        try_visit_block_exprs(definition, &mut |expr| {
            let ExprKind::VariantPayloadProject { operand, field } = &expr.kind else {
                return Ok(());
            };
            let Some(value) = stable_variant_value(body, &stable_locals, operand) else {
                return Err(MirValidationError {
                    function,
                    block,
                    kind: MirValidationErrorKind::VariantPayloadNotDominated { field: *field },
                });
            };
            let dominated = proofs.iter().any(|proof| {
                proof.value == value
                    && proof.variant == field.variant()
                    && edge_dominates_block(body, proof.edge, block)
            });
            if dominated {
                Ok(())
            } else {
                Err(MirValidationError {
                    function,
                    block,
                    kind: MirValidationErrorKind::VariantPayloadNotDominated { field: *field },
                })
            }
        })?;
    }
    Ok(())
}

fn validate_variant_shape(module: &Module, expr: &Expr) -> Result<(), MirValidationErrorKind> {
    match &expr.kind {
        ExprKind::VariantTest { operand, variant } => {
            variant.definition(&module.enums).map_err(|error| {
                MirValidationErrorKind::InvalidVariantReference {
                    operation: MirVariantOperation::Test,
                    error,
                }
            })?;
            validate_operand_type(operand, *variant, MirVariantOperation::Test)?;
            if expr.ty != Type::Boolean {
                return Err(MirValidationErrorKind::VariantTestResultType {
                    actual: expr.ty.clone(),
                });
            }
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            let definition = field
                .definition(&module.enums)
                .map_err(|error| MirValidationErrorKind::InvalidVariantFieldReference { error })?;
            validate_operand_type(
                operand,
                field.variant(),
                MirVariantOperation::PayloadProject,
            )?;
            if expr.ty != definition.ty {
                return Err(MirValidationErrorKind::VariantPayloadResultType {
                    field: *field,
                    expected: definition.ty.clone(),
                    actual: expr.ty.clone(),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_operand_type(
    operand: &Expr,
    variant: MirVariantRef,
    operation: MirVariantOperation,
) -> Result<(), MirValidationErrorKind> {
    let Type::Enum(actual, _) = &operand.ty else {
        return Err(MirValidationErrorKind::VariantOperandIsNotEnum {
            operation,
            actual: operand.ty.clone(),
        });
    };
    if *actual != variant.enum_id() {
        return Err(MirValidationErrorKind::VariantOperandEnumMismatch {
            operation,
            expected: variant.enum_id(),
            actual: *actual,
        });
    }
    Ok(())
}

fn stable_locals(body: &Body) -> Vec<bool> {
    let mut stable = body
        .locals
        .iter()
        .map(|(_, local)| !local.mutable)
        .collect::<Vec<_>>();
    for (_, block) in body.blocks.iter() {
        for statement in &block.statements {
            if let StatementKind::Assign { local, .. } = statement.kind {
                let index = local.into_raw().into_u32() as usize;
                if let Some(stable) = stable.get_mut(index) {
                    *stable = false;
                }
            }
        }
    }
    stable
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableVariantValue(LocalId);

fn stable_variant_value(
    body: &Body,
    stable_locals: &[bool],
    operand: &Expr,
) -> Option<StableVariantValue> {
    let ExprKind::Local(local) = operand.kind else {
        return None;
    };
    let index = local.into_raw().into_u32() as usize;
    if !stable_locals.get(index).copied().unwrap_or(false) {
        return None;
    }
    (body.locals[local].ty == operand.ty).then_some(StableVariantValue(local))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CfgEdgeKind {
    Goto,
    BranchTrue,
    BranchFalse,
    BlockUnwind,
    ThrowUnwind,
    RethrowUnwind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CfgEdge {
    from: BlockId,
    to: BlockId,
    kind: CfgEdgeKind,
}

#[derive(Debug, Clone, Copy)]
struct VariantProof {
    value: StableVariantValue,
    variant: MirVariantRef,
    edge: CfgEdge,
}

fn matching_test_edges(module: &Module, body: &Body, stable_locals: &[bool]) -> Vec<VariantProof> {
    let mut proofs = Vec::new();
    for (block, definition) in body.blocks.iter() {
        let Terminator::Branch {
            cond, then_block, ..
        } = &definition.terminator
        else {
            continue;
        };
        let ExprKind::VariantTest { operand, variant } = &cond.kind else {
            continue;
        };
        if cond.ty != Type::Boolean
            || variant.definition(&module.enums).is_err()
            || validate_operand_type(operand, *variant, MirVariantOperation::Test).is_err()
        {
            continue;
        }
        let Some(value) = stable_variant_value(body, stable_locals, operand) else {
            continue;
        };
        proofs.push(VariantProof {
            value,
            variant: *variant,
            edge: CfgEdge {
                from: block,
                to: *then_block,
                kind: CfgEdgeKind::BranchTrue,
            },
        });
    }
    proofs
}

fn edge_dominates_block(body: &Body, edge: CfgEdge, target: BlockId) -> bool {
    is_reachable(body, target, None) && !is_reachable(body, target, Some(edge))
}

fn is_reachable(body: &Body, target: BlockId, skipped: Option<CfgEdge>) -> bool {
    if !valid_block(body, body.entry) || !valid_block(body, target) {
        return false;
    }
    let mut visited = vec![false; body.blocks.len()];
    let mut work = vec![body.entry];
    while let Some(block) = work.pop() {
        let index = block.into_raw().into_u32() as usize;
        if visited[index] {
            continue;
        }
        visited[index] = true;
        if block == target {
            return true;
        }
        for edge in block_edges(body, block) {
            if Some(edge) != skipped && valid_block(body, edge.to) {
                work.push(edge.to);
            }
        }
    }
    false
}

fn valid_block(body: &Body, block: BlockId) -> bool {
    (block.into_raw().into_u32() as usize) < body.blocks.len()
}

fn block_edges(body: &Body, from: BlockId) -> Vec<CfgEdge> {
    let block = &body.blocks[from];
    let mut edges = Vec::new();
    if let Some(to) = block.unwind {
        edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::BlockUnwind,
        });
    }
    match block.terminator {
        Terminator::Goto(to) => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::Goto,
        }),
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            edges.push(CfgEdge {
                from,
                to: then_block,
                kind: CfgEdgeKind::BranchTrue,
            });
            edges.push(CfgEdge {
                from,
                to: else_block,
                kind: CfgEdgeKind::BranchFalse,
            });
        }
        Terminator::Throw {
            unwind: Some(to), ..
        } => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::ThrowUnwind,
        }),
        Terminator::Rethrow { unwind: Some(to) } => edges.push(CfgEdge {
            from,
            to,
            kind: CfgEdgeKind::RethrowUnwind,
        }),
        Terminator::Return { .. }
        | Terminator::Throw { unwind: None, .. }
        | Terminator::Rethrow { unwind: None }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
    edges
}

fn try_visit_block_exprs(
    block: &BasicBlock,
    visitor: &mut impl FnMut(&Expr) -> Result<(), MirValidationError>,
) -> Result<(), MirValidationError> {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Expr(expr) => try_visit_expr(expr, visitor)?,
            StatementKind::Call(effect) => {
                let call = match effect {
                    CallEffect::Unit(call) | CallEffect::Value { call, .. } => call,
                };
                for argument in &call.args {
                    try_visit_expr(argument, visitor)?;
                }
            }
            StatementKind::ValDecl { init, .. } => try_visit_expr(init, visitor)?,
            StatementKind::Assign { value, .. } | StatementKind::GlobalAssign { value, .. } => {
                try_visit_expr(value, visitor)?
            }
            StatementKind::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                try_visit_expr(array, visitor)?;
                try_visit_expr(index, visitor)?;
                try_visit_expr(value, visitor)?;
            }
            StatementKind::FieldSet { object, value, .. }
            | StatementKind::AtomicFieldStore { object, value, .. } => {
                try_visit_expr(object, visitor)?;
                try_visit_expr(value, visitor)?;
            }
            StatementKind::Eh(_) => {}
        }
    }
    match &block.terminator {
        Terminator::Branch { cond, .. } => try_visit_expr(cond, visitor)?,
        Terminator::Return { value: Some(value) } => try_visit_expr(value, visitor)?,
        Terminator::Throw { exception, .. } => try_visit_expr(exception, visitor)?,
        Terminator::Goto(_)
        | Terminator::Return { value: None }
        | Terminator::Rethrow { .. }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
    Ok(())
}

fn try_visit_expr(
    expr: &Expr,
    visitor: &mut impl FnMut(&Expr) -> Result<(), MirValidationError>,
) -> Result<(), MirValidationError> {
    let mut error = None;
    visit_expr(expr, &mut |expr| {
        if error.is_none()
            && let Err(found) = visitor(expr)
        {
            error = Some(found);
        }
    });
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
