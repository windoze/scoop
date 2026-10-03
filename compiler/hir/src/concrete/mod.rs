//! Fully instantiated HIR consumed exclusively by this Cone's MIR stage.
//!
//! This module intentionally defines its own entity-id family.  No type
//! parameter, generic template, or export-side arena id can be represented
//! here.  HIR lowering must construct this graph completely before MIR starts.

use la_arena::{Arena, Idx};
pub use scoop_ast::Span;
pub use scoop_identity::{
    CallableApplicationKey, CallableMaterialization, CallableMaterializationContext,
    CallableOdrMemberId, CallableTemplateOwner, CallbackApplicationKey, CallbackMode,
    CborIdentityRecord, Effect, ExactCallableSignature, GeneratedCallableKey,
    InitializationUnitKey, LexicalCallableParent, LocalValueKey, LocalValueSelector, NonEmptyVec,
    OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberIdentityError, OdrMemberKey,
    OdrMemberRole, PersistentCallableApplicationId, PersistentCallbackApplicationId,
    PersistentExactTypeId, PersistentFieldId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentInitializationUnitId, PersistentLocalValueId, SourceNativeExternalContractRecord,
    SpecializationKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, SyntheticLocalRole,
};

pub use super::{
    ArrayAccessKind, BinOp, CallableModifiers, CallingConvention, ClassModifier,
    ConcreteExpressionOrigin, DefinitionOrigin, EvaluationOrigin, ExternAbi, FunctionAttributes,
    GcEffect, HirCLayoutContract, HirCLayoutValue, HirIntegerConstant, IntegerConversion,
    IntegerDivRem, IntegerKind, IntegerOperation, IntegerOperationArity, IntegerSignedness,
    IntegerTypeCore, IntegerWidth, IntrinsicFunction, IntrinsicFunctionKind, IntrinsicProviderId,
    IntrinsicTypeKind, LocalValueDefinitionSite, MethodModifier, NoGcIntegerOperation,
    OperatorKind, PrimitiveBinaryKind, PrimitiveUnaryKind, Safety, StringConstantOwner,
    StructAttributes, UnOp,
};

mod types;
pub use types::*;

mod type_relations;
pub use type_relations::*;

mod exact_types;
pub use exact_types::*;

mod local_values;
pub use local_values::*;

mod dispatch_slots;
pub use dispatch_slots::*;

mod callable_applications;
pub use callable_applications::*;

mod callback_applications;
pub use callback_applications::*;

mod module;
pub use module::*;

mod callables;
pub use callables::*;

mod imported_dependency;
pub use imported_dependency::*;

mod nominals;
pub use nominals::*;

mod functions;
pub use functions::*;

mod body;
pub use body::*;

mod executable_expressions;
pub use executable_expressions::*;
