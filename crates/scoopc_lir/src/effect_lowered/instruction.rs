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

/// LIR-local origin for a callable local slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirLocalSourceKind {
    SourceLocal,
    CompilerTemporary,
}

impl From<crate::mir::LocalSourceKind> for LirLocalSourceKind {
    fn from(source: crate::mir::LocalSourceKind) -> Self {
        match source {
            crate::mir::LocalSourceKind::SourceLocal => Self::SourceLocal,
            crate::mir::LocalSourceKind::CompilerTemporary => Self::CompilerTemporary,
        }
    }
}

/// Parameter metadata owned by a LIR callable header.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirParam {
    span: Span,
    name: String,
    ty: TypeId,
    local: LocalId,
}

impl LirParam {
    pub fn new(span: Span, name: String, ty: TypeId, local: LocalId) -> Self {
        Self {
            span,
            name,
            ty,
            local,
        }
    }

    pub fn from_source(param: &crate::mir::Param) -> Self {
        Self::new(param.span, param.name.clone(), param.ty, param.local)
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn local(&self) -> LocalId {
        self.local
    }
}

/// Callable header data required by executable body emission.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirCallableHeader {
    span: Span,
    root_fqn: String,
    name: String,
    function_ty: TypeId,
    return_ty: TypeId,
    params: Vec<LirParam>,
}

impl LirCallableHeader {
    pub fn new(
        span: Span,
        root_fqn: String,
        name: String,
        function_ty: TypeId,
        return_ty: TypeId,
        params: Vec<LirParam>,
    ) -> Self {
        Self {
            span,
            root_fqn,
            name,
            function_ty,
            return_ty,
            params,
        }
    }

    pub fn from_source(callable: &crate::mir::FunDecl) -> Self {
        Self::new(
            callable.span,
            callable.fqn.clone(),
            callable.name.clone(),
            callable.ty,
            callable.return_ty,
            callable.params.iter().map(LirParam::from_source).collect(),
        )
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn function_ty(&self) -> TypeId {
        self.function_ty
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn params(&self) -> &[LirParam] {
        &self.params
    }
}

/// Local slot declaration owned by a LIR executable body.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirLocalDecl {
    local: LocalId,
    span: Span,
    name: Option<String>,
    ty: TypeId,
    source: LirLocalSourceKind,
}

impl LirLocalDecl {
    pub fn new(
        local: LocalId,
        span: Span,
        name: Option<String>,
        ty: TypeId,
        source: LirLocalSourceKind,
    ) -> Self {
        Self {
            local,
            span,
            name,
            ty,
            source,
        }
    }

    pub fn from_source(local: LocalId, decl: &crate::mir::LocalDecl) -> Self {
        Self::new(
            local,
            decl.span,
            decl.name.clone(),
            decl.ty,
            decl.source.into(),
        )
    }

    pub fn local(&self) -> LocalId {
        self.local
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn source(&self) -> LirLocalSourceKind {
        self.source
    }
}

/// Flavor of callable body represented by the unified state-owned executable body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LirExecutableBodyFlavor {
    Plain,
    PlainLocalEffectControl,
    EffectStep,
}

/// Statement index within one LIR-owned state body.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LirStatementIndex(u32);

impl LirStatementIndex {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Stable anchor to a node in a LIR-owned executable body.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum LirBodyAnchor {
    State {
        state: super::ir::StateId,
    },
    Statement {
        state: super::ir::StateId,
        statement: LirStatementIndex,
    },
    Terminator {
        state: super::ir::StateId,
    },
}

impl LirBodyAnchor {
    pub fn state(state: super::ir::StateId) -> Self {
        Self::State { state }
    }

    pub fn statement(state: super::ir::StateId, statement: LirStatementIndex) -> Self {
        Self::Statement { state, statement }
    }

    pub fn terminator(state: super::ir::StateId) -> Self {
        Self::Terminator { state }
    }
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

    pub fn statement_anchor(
        &self,
        state: super::ir::StateId,
        statement: LirStatementIndex,
    ) -> Option<LirBodyAnchor> {
        ((statement.as_u32() as usize) < self.statements.len())
            .then_some(LirBodyAnchor::statement(state, statement))
    }

    pub fn terminator_anchor(&self, state: super::ir::StateId) -> LirBodyAnchor {
        LirBodyAnchor::terminator(state)
    }
}

/// One executable state with its LIR-owned instruction body.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirExecutableState {
    state_id: super::ir::StateId,
    role: super::ir::LateLoweredStateRole,
    body: LirStateBody,
}

impl LirExecutableState {
    pub fn new(
        state_id: super::ir::StateId,
        role: super::ir::LateLoweredStateRole,
        body: LirStateBody,
    ) -> Self {
        Self {
            state_id,
            role,
            body,
        }
    }

    pub fn state_id(&self) -> super::ir::StateId {
        self.state_id
    }

    pub fn role(&self) -> super::ir::LateLoweredStateRole {
        self.role
    }

    pub fn body(&self) -> &LirStateBody {
        &self.body
    }
}

/// Unified LIR body graph used for plain and effect-step callables.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirStateMachineBody {
    entry_state: super::ir::StateId,
    complete_state: super::ir::StateId,
    cleanup_state: Option<super::ir::StateId>,
    drop_state: Option<super::ir::StateId>,
    states: Vec<LirExecutableState>,
}

impl LirStateMachineBody {
    pub fn new(
        entry_state: super::ir::StateId,
        complete_state: super::ir::StateId,
        cleanup_state: Option<super::ir::StateId>,
        drop_state: Option<super::ir::StateId>,
        states: Vec<LirExecutableState>,
    ) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        for state in &states {
            assert!(
                seen.insert(state.state_id()),
                "duplicate LIR executable state anchor {:?}",
                state.state_id()
            );
        }
        assert!(
            seen.contains(&entry_state),
            "entry state {:?} is not present in LIR executable body",
            entry_state
        );
        assert!(
            seen.contains(&complete_state),
            "complete state {:?} is not present in LIR executable body",
            complete_state
        );
        Self {
            entry_state,
            complete_state,
            cleanup_state,
            drop_state,
            states,
        }
    }

    pub fn entry_state(&self) -> super::ir::StateId {
        self.entry_state
    }

    pub fn complete_state(&self) -> super::ir::StateId {
        self.complete_state
    }

    pub fn cleanup_state(&self) -> Option<super::ir::StateId> {
        self.cleanup_state
    }

    pub fn drop_state(&self) -> Option<super::ir::StateId> {
        self.drop_state
    }

    pub fn states(&self) -> &[LirExecutableState] {
        &self.states
    }

    pub fn state(&self, state_id: super::ir::StateId) -> Option<&LirExecutableState> {
        self.states
            .iter()
            .find(|state| state.state_id() == state_id)
    }

    pub fn state_anchor(&self, state_id: super::ir::StateId) -> Option<LirBodyAnchor> {
        self.state(state_id)
            .map(|state| LirBodyAnchor::state(state.state_id()))
    }

    pub fn statement_anchor(
        &self,
        state_id: super::ir::StateId,
        statement: LirStatementIndex,
    ) -> Option<LirBodyAnchor> {
        self.state(state_id)
            .and_then(|state| state.body().statement_anchor(state_id, statement))
    }

    pub fn terminator_anchor(&self, state_id: super::ir::StateId) -> Option<LirBodyAnchor> {
        self.state(state_id)
            .map(|state| state.body().terminator_anchor(state_id))
    }
}

/// Complete executable body payload independent from MIR `FunDecl::body`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LirExecutableBody {
    flavor: LirExecutableBodyFlavor,
    header: LirCallableHeader,
    locals: Vec<LirLocalDecl>,
    states: LirStateMachineBody,
}

impl LirExecutableBody {
    pub fn new(
        flavor: LirExecutableBodyFlavor,
        header: LirCallableHeader,
        locals: Vec<LirLocalDecl>,
        states: LirStateMachineBody,
    ) -> Self {
        Self {
            flavor,
            header,
            locals,
            states,
        }
    }

    pub fn flavor(&self) -> LirExecutableBodyFlavor {
        self.flavor
    }

    pub fn header(&self) -> &LirCallableHeader {
        &self.header
    }

    pub fn locals(&self) -> &[LirLocalDecl] {
        &self.locals
    }

    pub fn states(&self) -> &LirStateMachineBody {
        &self.states
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
    use crate::effect_lowered::ir::{
        LateLoweredCompletionPayloadSource, LateLoweredStateRole, LateLoweredStateTerminator,
        StateId,
    };

    fn sample_state_machine(complete_ty: TypeId, statement_count: usize) -> LirStateMachineBody {
        let entry = StateId::new(0);
        let complete = StateId::new(1);
        let statements = (0..statement_count)
            .map(|_| LirStatement {
                span: Span::new(0, 1),
                kind: LirStatementKind::Nop,
            })
            .collect();

        LirStateMachineBody::new(
            entry,
            complete,
            None,
            None,
            vec![
                LirExecutableState::new(
                    entry,
                    LateLoweredStateRole::Entry,
                    LirStateBody::new(
                        statements,
                        LateLoweredStateTerminator::Return {
                            payload_source: LateLoweredCompletionPayloadSource::unit(complete_ty),
                            complete_state: complete,
                        },
                    ),
                ),
                LirExecutableState::new(
                    complete,
                    LateLoweredStateRole::Complete,
                    LirStateBody::new(Vec::new(), LateLoweredStateTerminator::Unreachable),
                ),
            ],
        )
    }

    fn enum_body<'a>(source: &'a str, enum_name: &str) -> &'a str {
        let marker = format!("pub enum {enum_name}");
        let enum_start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing enum {enum_name}"));
        let brace_start = enum_start
            + source[enum_start..]
                .find('{')
                .unwrap_or_else(|| panic!("missing enum body for {enum_name}"));
        let mut depth = 0usize;
        for (offset, ch) in source[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[brace_start..=brace_start + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated enum body for {enum_name}");
    }

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

    #[test]
    fn plain_executable_body_carries_header_params_and_locals() {
        let mut types = crate::ty::TypeStore::new();
        let builtins = types.intern_builtins();
        let span = Span::new(10, 20);
        let param_local = LocalId::from_raw(0);
        let tmp_local = LocalId::from_raw(1);
        let header = LirCallableHeader::new(
            span,
            "sample.main".to_string(),
            "main".to_string(),
            builtins.any,
            builtins.int,
            vec![LirParam::new(
                span,
                "value".to_string(),
                builtins.int,
                param_local,
            )],
        );
        let body = LirExecutableBody::new(
            LirExecutableBodyFlavor::Plain,
            header,
            vec![
                LirLocalDecl::new(
                    param_local,
                    span,
                    Some("value".to_string()),
                    builtins.int,
                    LirLocalSourceKind::SourceLocal,
                ),
                LirLocalDecl::new(
                    tmp_local,
                    Span::new(21, 22),
                    None,
                    builtins.string,
                    LirLocalSourceKind::CompilerTemporary,
                ),
            ],
            sample_state_machine(builtins.int, 1),
        );

        assert_eq!(body.flavor(), LirExecutableBodyFlavor::Plain);
        assert_eq!(body.header().root_fqn(), "sample.main");
        assert_eq!(body.header().name(), "main");
        assert_eq!(body.header().return_ty(), builtins.int);
        assert_eq!(body.header().params()[0].local(), param_local);
        assert_eq!(body.locals().len(), 2);
        assert_eq!(body.locals()[0].source(), LirLocalSourceKind::SourceLocal);
        assert_eq!(
            body.locals()[1].source(),
            LirLocalSourceKind::CompilerTemporary
        );
        assert_eq!(body.states().entry_state(), StateId::new(0));
    }

    #[test]
    fn state_statement_and_terminator_anchors_are_unique_and_body_owned() {
        let mut types = crate::ty::TypeStore::new();
        let builtins = types.intern_builtins();
        let states = sample_state_machine(builtins.unit, 2);
        let entry = StateId::new(0);
        let complete = StateId::new(1);
        let missing = StateId::new(99);
        let first_statement = states
            .statement_anchor(entry, LirStatementIndex::new(0))
            .expect("entry statement 0 应存在于 LIR body");
        let second_statement = states
            .statement_anchor(entry, LirStatementIndex::new(1))
            .expect("entry statement 1 应存在于 LIR body");
        let terminator = states
            .terminator_anchor(entry)
            .expect("entry terminator 应存在于 LIR body");
        let entry_state = states
            .state_anchor(entry)
            .expect("entry state 应存在于 LIR body");
        let complete_state = states
            .state_anchor(complete)
            .expect("complete state 应存在于 LIR body");

        assert_eq!(
            std::collections::BTreeSet::from([
                first_statement,
                second_statement,
                terminator,
                entry_state,
                complete_state,
            ])
            .len(),
            5
        );
        assert_eq!(
            states.statement_anchor(entry, LirStatementIndex::new(2)),
            None
        );
        assert_eq!(states.state_anchor(missing), None);
        assert_eq!(states.terminator_anchor(missing), None);
    }

    #[test]
    fn lir_instruction_enums_do_not_define_placeholder_variants() {
        let source = include_str!("instruction.rs");
        for enum_name in ["LirRvalue", "LirStatementKind", "LirInstruction"] {
            let body = enum_body(source, enum_name);
            assert!(!body.contains("Todo"), "{enum_name} must stay total");
            assert!(
                !body.contains("UnresolvedName"),
                "{enum_name} must not carry unresolved names"
            );
        }
    }
}
