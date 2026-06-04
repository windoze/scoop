use scoopc_ids::LirCallableId;

use crate::effect_facts::{CaseTag, ConcreteOpKey};
use crate::mir::{
    AggregateTransportMetadata, ArrayElementTransportMetadata, BasicBlockId, ConstValue,
    GcIntrinsicOperation, GcIntrinsicPairing, GcRootLifetime, LocalId, MirCallableAbiKind,
    MirCallableImplPlan, PatternBindingStep, ResumeMetadata, SiteId,
    StoredContinuationRoutePublication, ValueTransportMetadata,
};
use crate::span::Span;
use crate::stable_id::{StableInstanceKey, StableTemplateKey};
use crate::ty::{EffectRow, TypeId};

/// Opaque LIR handle for a resolved member declaration or slot.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirMemberKey(String);

impl LirMemberKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque LIR handle for a resolved virtual/interface dispatch family.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirDispatchKey(String);

impl LirDispatchKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// LIR-level runtime type operator for `is` checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirTypeCheckOp {
    Is,
    NotIs,
}

/// LIR-level runtime cast operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirCastOp {
    As,
    AsQuestion,
}

/// Nominal runtime type family retained without depending on AST definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimeNominalKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

/// Runtime descriptor identity with nominal references represented by LIR handles.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirRuntimeTypeDescriptorKey {
    pub ty: TypeId,
    pub kind: LirRuntimeTypeDescriptorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimeTypeDescriptorKind {
    Any,
    String,
    Nominal {
        nominal: scoopc_lir_facts::LirNominalLayoutKey,
        kind: Option<LirRuntimeNominalKind>,
    },
    Function,
    Option,
    Tuple,
    Value,
    TypeParam,
    StarProjection,
    Union,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimeTypeParameterizedMatch {
    None,
    Nominal {
        type_args: Vec<TypeId>,
        effect_arg: Option<EffectRow>,
    },
    Function {
        receiver: Option<TypeId>,
        params: Vec<TypeId>,
        return_ty: TypeId,
        effects: EffectRow,
        effects_closed: bool,
    },
    Option {
        payload_ty: TypeId,
    },
    Tuple {
        element_tys: Vec<TypeId>,
    },
    Union {
        variants: Vec<TypeId>,
    },
    StarProjection {
        read_ty: TypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirRuntimeTypeTestMetadata {
    pub source_ty: TypeId,
    pub target_ty: TypeId,
    pub descriptor: LirRuntimeTypeDescriptorKey,
    pub static_fold: crate::mir::RuntimeTypeStaticFold,
    pub parameterized: LirRuntimeTypeParameterizedMatch,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimeCastFailure {
    Panic { message: String },
    ReturnNone,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimeCastResult {
    Target { ty: TypeId },
    Option { option_ty: TypeId, some_ty: TypeId },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirRuntimeCastMetadata {
    pub test: LirRuntimeTypeTestMetadata,
    pub failure: LirRuntimeCastFailure,
    pub result: LirRuntimeCastResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirRuntimePatternTypeTestKind {
    StaticValue,
    RuntimeRef,
    RuntimeClass,
    RuntimeInterface,
    RuntimeNominal,
    RuntimeParameterized,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirRuntimePatternTypeTestMetadata {
    pub subject_ty: TypeId,
    pub target_ty: TypeId,
    pub descriptor: LirRuntimeTypeDescriptorKey,
    pub match_kind: LirRuntimePatternTypeTestKind,
    pub static_fold: crate::mir::RuntimeTypeStaticFold,
    pub parameterized: LirRuntimeTypeParameterizedMatch,
}

/// Pattern test payload used by LIR value instructions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirPattern {
    Else,
    Or {
        pats: Vec<LirPattern>,
    },
    Wildcard,
    Rest,
    Is {
        ty: TypeId,
        metadata: LirRuntimePatternTypeTestMetadata,
    },
    Bind {
        name: String,
        ty: TypeId,
    },
    Tuple {
        elements: Vec<LirPattern>,
    },
    Variant {
        name: String,
        args: Vec<LirPattern>,
    },
    IntLit {
        raw: String,
    },
    CharLit {
        value: char,
    },
    StringLit {
        value: String,
    },
    BoolLit {
        value: bool,
    },
}

/// LIR operand model: values are either locals or constants.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirOperand {
    Local(LocalId),
    Const(ConstValue),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirCallArg {
    pub span: Span,
    pub name: Option<String>,
    pub value: LirOperand,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirPerformArg {
    pub span: Span,
    pub source_arg_index: usize,
    pub name: Option<String>,
    pub value: LirOperand,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirTopLevelRefTarget {
    Callable(LirCallableId),
    Global(scoopc_lir_facts::LirGlobalRootKey),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirTopLevelRef {
    pub target: LirTopLevelRefTarget,
    pub site_id: Option<SiteId>,
    pub hidden_effects: EffectRow,
    #[serde(default)]
    pub stable_template_key: Option<Box<StableTemplateKey>>,
    #[serde(default)]
    pub stable_instance_key: Option<Box<StableInstanceKey>>,
    #[serde(default)]
    pub generic_type_args: Vec<TypeId>,
    #[serde(default)]
    pub generic_eff_args: Vec<EffectRow>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirMemberAccessMetadata {
    pub name: String,
    pub receiver_ty: TypeId,
    pub resolved: LirMemberTarget,
    pub hidden_effects: EffectRow,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirMemberTarget {
    Value { member: LirMemberKey },
    Fun { callable: LirCallableId },
    ExtensionValue { member: LirMemberKey },
    ExtensionFun { callable: LirCallableId },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirDispatchMetadata {
    pub dispatch: LirDispatchKey,
    pub owner: scoopc_lir_facts::LirNominalLayoutKey,
    pub member_name: String,
    pub member: LirMemberKey,
    pub member_decl_span: Option<Span>,
    pub receiver_ty: TypeId,
    #[serde(default)]
    pub stable_candidate_keys: Vec<StableInstanceKey>,
    #[serde(default)]
    pub stable_template_key: Option<Box<StableTemplateKey>>,
    #[serde(default)]
    pub generic_type_args: Vec<TypeId>,
    #[serde(default)]
    pub generic_eff_args: Vec<EffectRow>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirClassCtorCallMetadata {
    pub target_init_class: scoopc_lir_facts::LirNominalLayoutKey,
    pub selected_ctor_span: Option<Span>,
    pub ordered_param_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirInterpolatedStringPart {
    pub span: Span,
    pub kind: LirInterpolatedStringPartKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirInterpolatedStringPartKind {
    Text,
    Expr { value: LirOperand, ty: TypeId },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirStructLitField {
    pub span: Span,
    pub name: String,
    pub value: LirOperand,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirTypeMetadataLiteral {
    pub source_ty: TypeId,
    pub source_nominal: Option<scoopc_lir_facts::LirNominalLayoutKey>,
    pub kind: crate::mir::TypeMetadataLiteralKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirGcIntrinsicTransportMetadata {
    pub callee: LirCallableId,
    pub operation: GcIntrinsicOperation,
    pub root_lifetime: GcRootLifetime,
    pub pairing: GcIntrinsicPairing,
    pub unsafe_required: bool,
    pub subject_ty: TypeId,
    pub token_ty: Option<TypeId>,
    pub subject: ValueTransportMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallAbiHandoffMetadata {
    pub callable_abi_kind: MirCallableAbiKind,
    pub resolved_outward_cases: Vec<CaseTag>,
    pub impl_plan: MirCallableImplPlan,
    pub adapter_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirCallTransportMetadata {
    pub result: ValueTransportMetadata,
    pub aggregate_return: Option<ValueTransportMetadata>,
    pub array: Option<ArrayElementTransportMetadata>,
    pub gc: Option<LirGcIntrinsicTransportMetadata>,
    pub abi: LirCallAbiHandoffMetadata,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirCallKind {
    Direct {
        callee: LirCallableId,
        #[serde(default)]
        stable_template_key: Option<Box<StableTemplateKey>>,
        #[serde(default)]
        stable_instance_key: Option<Box<StableInstanceKey>>,
        #[serde(default)]
        intrinsic_entry_name: Option<String>,
        #[serde(default)]
        generic_type_args: Vec<TypeId>,
        #[serde(default)]
        generic_eff_args: Vec<EffectRow>,
    },
    Closure {
        callee: LirOperand,
        fn_ptr: LirCallableId,
    },
    FunValue {
        callee: LirOperand,
    },
    FunPtr {
        callee: LirOperand,
    },
    Virtual {
        receiver: LirOperand,
        dispatch: LirDispatchMetadata,
    },
    Interface {
        receiver: LirOperand,
        dispatch: LirDispatchMetadata,
    },
    Resume {
        continuation: LirOperand,
        resume: ResumeMetadata,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirRvalue {
    Use(LirOperand),
    Transport {
        value: LirOperand,
        transport: ValueTransportMetadata,
    },
    TopLevelRef(LirTopLevelRef),
    TypeCheck {
        value: LirOperand,
        op: LirTypeCheckOp,
        test_ty: TypeId,
        metadata: LirRuntimeTypeTestMetadata,
    },
    Cast {
        value: LirOperand,
        op: LirCastOp,
        target_ty: TypeId,
        metadata: LirRuntimeCastMetadata,
    },
    MemberAccess {
        site_id: Option<SiteId>,
        receiver: LirOperand,
        member: LirMemberAccessMetadata,
    },
    EnumVariant {
        enum_ty: TypeId,
        variant_name: String,
        args: Vec<LirCallArg>,
        payload: AggregateTransportMetadata,
    },
    ClassCtor {
        site_id: SiteId,
        class: scoopc_lir_facts::LirNominalLayoutKey,
        ctor: LirClassCtorCallMetadata,
        args: Vec<LirCallArg>,
        hidden_effects: EffectRow,
    },
    Call {
        site_id: SiteId,
        kind: LirCallKind,
        args: Vec<LirCallArg>,
        transport: LirCallTransportMetadata,
    },
    MakeTuple {
        elements: Vec<LirOperand>,
        transport: AggregateTransportMetadata,
    },
    StructLit {
        fields: Vec<LirStructLitField>,
        transport: AggregateTransportMetadata,
    },
    SizeOf {
        site_id: SiteId,
        value_ty: TypeId,
    },
    KindOf {
        site_id: SiteId,
        value_ty: TypeId,
    },
    AlignOf {
        site_id: SiteId,
        value_ty: TypeId,
    },
    DescOf {
        site_id: SiteId,
        value_ty: TypeId,
    },
    TypeMetadataLiteral(LirTypeMetadataLiteral),
    InterpolatedString {
        raw: bool,
        parts: Vec<LirInterpolatedStringPart>,
    },
    TupleGet {
        tuple: LirOperand,
        index: usize,
    },
    PatternMatch {
        subject: LirOperand,
        pattern: LirPattern,
    },
    PatternExtract {
        subject: LirOperand,
        path: Vec<PatternBindingStep>,
    },
    MakeClosure {
        env: LirOperand,
        fn_ptr: LirCallableId,
        env_contract: crate::mir::ClosureEnvTransportMetadata,
    },
    PerformResult {
        op: ConcreteOpKey,
        effect_ty: TypeId,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirStatementKind {
    Nop,
    Assign {
        target: LocalId,
        value: LirRvalue,
    },
    StoreMember {
        receiver: LirOperand,
        member: LirMemberAccessMetadata,
        value: LirOperand,
        value_ty: TypeId,
        continuation_route: StoredContinuationRoutePublication,
    },
    StoreGlobal {
        root: scoopc_lir_facts::LirGlobalRootKey,
        value: LirOperand,
        value_ty: TypeId,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirStatement {
    pub span: Span,
    pub kind: LirStatementKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirUnwindAction {
    NoUnwind,
    Propagate,
    Cleanup { target: BasicBlockId },
}

/// State-level terminators are already modeled by the late-lowered state graph.
/// Its suspend, handler-dispatch, branch, return, unwind, and unreachable cases
/// are the LIR terminator layer used by state-owned instruction bodies.
pub type LirTerminator = super::ir::LateLoweredStateTerminator;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirStateBody {
    statements: Vec<LirStatement>,
    terminator: LirTerminator,
}

impl LirStateBody {
    pub fn new(statements: Vec<LirStatement>, terminator: LirTerminator) -> Self {
        Self {
            statements,
            terminator,
        }
    }

    pub fn statements(&self) -> &[LirStatement] {
        &self.statements
    }

    pub fn terminator(&self) -> &LirTerminator {
        &self.terminator
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LirInstruction {
    Statement(LirStatement),
    Terminator(LirTerminator),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_closure_calls_use_callable_ids() {
        let direct = LirCallKind::Direct {
            callee: LirCallableId::from_raw(3),
            stable_template_key: None,
            stable_instance_key: None,
            intrinsic_entry_name: None,
            generic_type_args: Vec::new(),
            generic_eff_args: Vec::new(),
        };
        let closure = LirCallKind::Closure {
            callee: LirOperand::Local(LocalId::from_raw(1)),
            fn_ptr: LirCallableId::from_raw(4),
        };

        assert!(matches!(direct, LirCallKind::Direct { .. }));
        assert!(matches!(closure, LirCallKind::Closure { .. }));
    }

    #[test]
    fn global_and_member_refs_are_handles() {
        let global =
            LirTopLevelRefTarget::Global(scoopc_lir_facts::LirGlobalRootKey::new("sample.value"));
        let member = LirMemberTarget::Value {
            member: LirMemberKey::new("sample.Type.member"),
        };

        assert!(matches!(global, LirTopLevelRefTarget::Global(_)));
        assert!(matches!(member, LirMemberTarget::Value { .. }));
    }

    #[test]
    fn member_access_requires_resolved_handle() {
        let mut types = crate::ty::TypeStore::new();
        let builtins = types.intern_builtins();
        let metadata = LirMemberAccessMetadata {
            name: "value".to_string(),
            receiver_ty: builtins.any,
            resolved: LirMemberTarget::Value {
                member: LirMemberKey::new("sample.Box.value"),
            },
            hidden_effects: EffectRow::pure(),
        };

        assert!(matches!(metadata.resolved, LirMemberTarget::Value { .. }));
    }
}
