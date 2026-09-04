//! Fully instantiated HIR consumed exclusively by this Cone's MIR stage.
//!
//! This module intentionally defines its own entity-id family.  No type
//! parameter, generic template, or export-side arena id can be represented
//! here.  HIR lowering must construct this graph completely before MIR starts.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub use super::{
    ArrayAccessKind, BinOp, CLayout, CallableModifiers, CallingConvention, ClassModifier,
    ConcreteExpressionOrigin, DefinitionOrigin, EvaluationOrigin, ExternAbi, FunctionAttributes,
    GcEffect, IntrinsicFunction, IntrinsicFunctionKind, IntrinsicProviderId,
    IntrinsicTypeDeclaration, IntrinsicTypeKind, MethodModifier, OperatorKind, PrimitiveBinaryKind,
    PrimitiveUnaryKind, Safety, StructAttributes, UnOp,
};

mod types;
pub use types::*;

mod module;
pub use module::*;

mod callables;
pub use callables::*;

mod nominals;
pub use nominals::*;

mod functions;
pub use functions::*;

mod body;
pub use body::*;
