use std::num::NonZeroU32;

use super::*;

/// One typed, immutable storage location used while an irrefutable binding
/// plan executes. The referenced `Local` must carry the same `ty` and remain
/// immutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingTemporary {
    pub local: LocalId,
    pub ty: TypeId,
}

/// One user-visible leaf introduced by a binding plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingLeaf {
    pub local: LocalId,
    pub mutability: BindingMutability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMutability {
    Immutable,
    Mutable,
}

impl BindingMutability {
    pub const fn is_mutable(self) -> bool {
        matches!(self, Self::Mutable)
    }
}

/// Complete declaration-order shape of one recursively irrefutable pattern.
/// Runtime ordering is represented independently by
/// `IrrefutableBindingPlan::actions`.
#[derive(Debug, Clone)]
pub enum IrrefutableBindingShape {
    Binding(BindingLeaf),
    Wildcard,
    Tuple(Vec<IrrefutableBindingShape>),
    Struct {
        application: StructApplicationId,
        /// Every declared field in declaration order, including wildcard
        /// entries filled by a source `..`.
        fields: Vec<(AppliedStructFieldRef, IrrefutableBindingShape)>,
    },
    Class {
        application: ClassApplicationId,
        /// Exactly the source-written component positions, in order.
        components: Vec<(NonZeroU32, IrrefutableBindingShape)>,
    },
}

/// A pure built-in projection selected during HIR lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingProjection {
    TupleIndex(u32),
    StructField(AppliedStructFieldRef),
}

/// Source-order, depth-first runtime action schedule for an irrefutable
/// binding. Every target is already typed; component calls already contain
/// their unique resolved callable target and any argument setup.
#[derive(Debug, Clone)]
pub enum IrrefutableBindingAction {
    Project {
        source: BindingTemporary,
        result: BindingTemporary,
        projection: BindingProjection,
        span: Span,
        origin: ExpressionOrigin,
    },
    Component {
        source: BindingTemporary,
        index: NonZeroU32,
        result: BindingTemporary,
        setup: Vec<Statement>,
        call: Expr,
        span: Span,
    },
    Bind {
        source: BindingTemporary,
        target: BindingLeaf,
        span: Span,
        origin: ExpressionOrigin,
    },
}

/// Typed binding data shared by `val` / `var`, lambda parameters, and source
/// `for`. The first two consumers expand it before publishing Export HIR;
/// generic source `for` retains it inside its complete iteration plan until
/// LocalConcrete HIR expansion.
#[derive(Debug, Clone)]
pub struct IrrefutableBindingPlan {
    pub subject: BindingTemporary,
    pub shape: IrrefutableBindingShape,
    pub actions: Vec<IrrefutableBindingAction>,
}
