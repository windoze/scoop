//! Schema identities and effect/control ABI contracts.

use scoopc_ids::StableEffectInstanceKey;
use scoopc_types::TypeId;

/// Stable identity of a `StepSchema(F)` fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepSchemaId(u32);

impl StepSchemaId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Stable identity of a continuation schema fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContinuationSchemaId(u32);

impl ContinuationSchemaId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Stable case tag inside one `StepSchema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaseTag(u32);

impl CaseTag {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Specialized effect family identity used by concrete operation cases.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectFamilyKey {
    effect_fqn: String,
    type_args: Vec<TypeId>,
}

impl EffectFamilyKey {
    pub fn new(effect_fqn: String, type_args: Vec<TypeId>) -> Self {
        Self {
            effect_fqn,
            type_args,
        }
    }

    pub fn effect_fqn(&self) -> &str {
        &self.effect_fqn
    }

    pub fn type_args(&self) -> &[TypeId] {
        &self.type_args
    }
}

/// Concrete effect operation selected by a step case.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConcreteOpKey {
    stable_instance_key: StableEffectInstanceKey,
    effect_family: EffectFamilyKey,
}

impl ConcreteOpKey {
    pub fn new(
        stable_instance_key: StableEffectInstanceKey,
        effect_family: EffectFamilyKey,
    ) -> Self {
        Self {
            stable_instance_key,
            effect_family,
        }
    }

    pub fn stable_instance_key(&self) -> &StableEffectInstanceKey {
        &self.stable_instance_key
    }

    pub fn effect_family(&self) -> &EffectFamilyKey {
        &self.effect_family
    }
}

/// Stable subset of cases inside one step schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSet {
    schema: StepSchemaId,
    tags: Vec<CaseTag>,
}

impl CaseSet {
    pub fn new(schema: StepSchemaId, mut tags: Vec<CaseTag>) -> Self {
        tags.sort();
        tags.dedup();
        Self { schema, tags }
    }

    pub fn schema(&self) -> StepSchemaId {
        self.schema
    }

    pub fn tags(&self) -> &[CaseTag] {
        &self.tags
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}

/// Lowering plan selected after effect facts are solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplPlan {
    NoOutward,
    SingleCase(CaseTag),
    CanonicalFull,
}

/// Single canonical case inside one step schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCaseFact {
    case_tag: CaseTag,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    continuation_schema: ContinuationSchemaId,
}

impl StepCaseFact {
    pub fn new(
        case_tag: CaseTag,
        concrete_op_key: ConcreteOpKey,
        payload_tuple_ty: TypeId,
        continuation_schema: ContinuationSchemaId,
    ) -> Self {
        Self {
            case_tag,
            concrete_op_key,
            payload_tuple_ty,
            continuation_schema,
        }
    }

    pub fn case_tag(&self) -> CaseTag {
        self.case_tag
    }

    pub fn concrete_op_key(&self) -> &ConcreteOpKey {
        &self.concrete_op_key
    }

    pub fn payload_tuple_ty(&self) -> TypeId {
        self.payload_tuple_ty
    }

    pub fn continuation_schema(&self) -> ContinuationSchemaId {
        self.continuation_schema
    }
}

/// Continuation surface/resume contract for one captured continuation shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationSchema {
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    surface_ty: TypeId,
}

impl ContinuationSchema {
    pub fn new(
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        out_step_schema: StepSchemaId,
        surface_ty: TypeId,
    ) -> Self {
        Self {
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
            surface_ty,
        }
    }

    pub fn resume_tuple_ty(&self) -> TypeId {
        self.resume_tuple_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn out_step_schema(&self) -> StepSchemaId {
        self.out_step_schema
    }

    pub fn surface_ty(&self) -> TypeId {
        self.surface_ty
    }
}

/// Canonical `StepSchema(F)` for one callable or local control owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepSchema {
    invoke_args_tuple_ty: TypeId,
    complete_ty: TypeId,
    continuation_obj_ty: TypeId,
    cases: Vec<StepCaseFact>,
}

impl StepSchema {
    pub fn new(
        invoke_args_tuple_ty: TypeId,
        complete_ty: TypeId,
        continuation_obj_ty: TypeId,
        cases: Vec<StepCaseFact>,
    ) -> Self {
        Self {
            invoke_args_tuple_ty,
            complete_ty,
            continuation_obj_ty,
            cases,
        }
    }

    pub fn invoke_args_tuple_ty(&self) -> TypeId {
        self.invoke_args_tuple_ty
    }

    pub fn complete_ty(&self) -> TypeId {
        self.complete_ty
    }

    pub fn continuation_obj_ty(&self) -> TypeId {
        self.continuation_obj_ty
    }

    pub fn cases(&self) -> &[StepCaseFact] {
        &self.cases
    }
}
