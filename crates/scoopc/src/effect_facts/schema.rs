use crate::mir::InstanceKey;
use crate::ty::TypeId;

/// `StepSchema(F)` 的稳定 identity。
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

/// `ContinuationSchema` 的稳定 identity。
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

/// 某个 `StepSchema(F)` 内部的稳定 case tag。
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

/// generic-specialized concrete effect op 的语义 identity。
///
/// 底层仍直接复用现有 `InstanceKey` 形状，但通过语义 newtype 避免后续阶段把它当成“普通
/// callable instance key”裸露出去。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConcreteOpKey {
    instance: InstanceKey,
}

impl ConcreteOpKey {
    pub fn new(instance: InstanceKey) -> Self {
        Self { instance }
    }

    pub fn instance_key(&self) -> &InstanceKey {
        &self.instance
    }
}

/// 某个 schema 内的一组稳定 case tag 子集。
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

/// 当前 callable 在 late lowering 前选中的实现档位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplPlan {
    NoOutward,
    SingleCase(CaseTag),
    CanonicalFull,
}

/// `StepSchema(F)` 中的单个 canonical case。
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

/// continuation surface/resume contract 的 canonical schema。
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

/// 单个 callable instance `F` 的 canonical `StepSchema(F)`。
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
