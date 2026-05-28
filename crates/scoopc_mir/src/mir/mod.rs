//! MIR（Mid-level IR / 当前阶段的 generic early MIR template）。
//!
//! 当前这层 MIR 的职责边界：
//! - 负责把 typed/lowered HIR 收口为 backend-agnostic 的显式 CFG、locals、ANF 风格 operand/materialization，
//!   以及语言级的 call / perform / resume / pattern / member-access 事实；
//! - 负责保留 generic template 语义：函数 `fqn`、`TypeKind::Param`、语言级 dispatch metadata
//!   都在这层继续保持抽象，不提前 materialize 成单态实例；
//! - generic MIR 不负责承载 LLVM statepoint/address space/stackmap、backend-private mangled symbol、
//!   vtable slot / itable id、GC ABI 或 runtime thunk 等 backend 落地细节；materialized MIR 只发布
//!   后续 LIR/codegen 共享的 stable exported symbol surface。
//!
//! 后续阶段会在此基础上继续做：
//! - monomorphization / instance materialization
//! - per-instance summary / devirtualization / inlining
//! - backend lowering（例如 LLVM codegen）
//!
//! 当前入口仍主要服务 `dump-mir` 与 MIR fixtures；未覆盖节点继续以 `Todo(...)` 占位，
//! 避免在边界收口阶段退回到 panic/隐式后端推断。

mod callables;
mod closure_simplify;
mod dispatch_devirtualize;
mod dump;
mod escape;
mod inline;
mod lower;
mod materialize;
mod pass_pipeline;
mod pass_view;
#[cfg(test)]
mod placeholder_inventory;
mod summary;
mod transport;

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use crate::ast;
use crate::span::Span;
use crate::stable_id::StableConeKey;
use crate::ty::{EffectRow, MonoTypeId, RefTypeKind, TypeId, TypeKind, TypeStore};

pub(crate) use callables::{MaterializedCallableFamilies, MaterializedCallableFamilyInput};
pub use callables::{MaterializedCallableFamilyView, MaterializedCallableView};
pub use dispatch_devirtualize::{
    DispatchDevirtualizationFacts, DispatchDevirtualizationTargetKey, KnownReceiverSubclassIndex,
    collect_known_receiver_subclasses,
};
pub use dump::{
    BodyLabels, build_body_labels_for_dump, stable_dump_file, stable_dump_materialized,
};
pub use escape::{
    CallableEscapeFacts, ClosureEscapeFact, ContinuationEscapeFact, EscapeStatus,
    MaterializedEscapeFacts,
};
pub use lower::{LoweredMir, MirLowerError, lower_for_dump};
pub use lower::{MirLoweringFacts, lower_hir_file_for_dump_with_facts};
pub use materialize::{
    MaterializedMir, MirMaterializeError, materialize_for_dump, materialize_for_dump_with_opt_level,
};
pub use pass_view::MaterializedMirPassRunRecord;
pub use pass_view::{
    MaterializedMirPassArtifacts, MaterializedMirPassView, MaterializedPassCallableFamilyView,
    MaterializedPassCallableView,
};
pub use scoopc_ids::{InstanceKey, SiteId, TemplateKey};
pub use summary::summarize_pass_rewritten_fun;
pub(crate) use summary::{
    DeclOnlySummaryInput, InstanceRootSummaryInput, build_materialized_summary_table,
};
pub use summary::{
    InstanceSummary, MaterializedMirSummaries, ParamUseSummary, ResultProvenance,
    ResultProvenanceSource,
};
#[cfg(feature = "llvm")]
pub use transport::mir_transport_trace_requirement_for_type;
pub use transport::{
    AggregateTransportField, AggregateTransportKind, AggregateTransportMetadata,
    ArrayElementTransportMetadata, ArrayTransportOperation, CallAbiHandoffMetadata,
    CallTransportMetadata, ClosureCaptureTransportMetadata, ClosureEnvTransportMetadata,
    GcIntrinsicOperation, GcIntrinsicPairing, GcIntrinsicTransportMetadata, GcRootLifetime,
    MirBoxingIntent, MirBoxingReason, MirCallableAbiKind, MirCallableImplPlan, MirTransportKind,
    MirTransportRequirements, ValueTransportMetadata,
};

/// MIR materialization 的 request-root 策略。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum MaterializeRequestRootMode<'a> {
    /// 将 request source 中的全部 callable 作为 request roots；dump / 调试路径沿用该模式。
    RequestSources,
    /// 只从选定 entry main 和显式 export entry points 出发做实例可达扫描。
    EntryMain { fqn: Option<&'a str> },
}

pub struct MaterializeCompilationUnitOptions<'a> {
    pub stable_cone_key: StableConeKey,
    pub source_cones: &'a HashMap<PathBuf, crate::cone::SourceConeInfo>,
    pub request_source_paths: &'a [std::path::PathBuf],
    pub request_root_mode: MaterializeRequestRootMode<'a>,
    pub opt_level: crate::opt::OptLevel,
}

/// 为编译单元 frontend/build 路径暴露可复用的 MIR materialization 入口。
#[cfg(test)]
pub fn materialize_compilation_unit_from_typechecked_inputs(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    request_source_paths: &[std::path::PathBuf],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    let stable_cone_key = request_source_paths
        .first()
        .map(|path| StableConeKey::for_virtual_source_path(path))
        .or_else(|| {
            compilation_unit
                .first()
                .map(|(source, _)| StableConeKey::for_virtual_source_path(source.path()))
        })
        .unwrap_or_else(|| StableConeKey::new("virtual-cone", "0.0.0"));
    materialize_compilation_unit_from_typechecked_inputs_with_options(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        MaterializeCompilationUnitOptions {
            stable_cone_key,
            source_cones: &HashMap::new(),
            request_source_paths,
            request_root_mode: MaterializeRequestRootMode::RequestSources,
            opt_level: crate::opt::OptLevel::O0,
        },
    )
}

pub fn materialize_compilation_unit_from_typechecked_inputs_with_opt_level(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    request_source_paths: &[std::path::PathBuf],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    opt_level: crate::opt::OptLevel,
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    let stable_cone_key = request_source_paths
        .first()
        .map(|path| StableConeKey::for_virtual_source_path(path))
        .or_else(|| {
            compilation_unit
                .first()
                .map(|(source, _)| StableConeKey::for_virtual_source_path(source.path()))
        })
        .unwrap_or_else(|| StableConeKey::new("virtual-cone", "0.0.0"));
    materialize_compilation_unit_from_typechecked_inputs_with_options(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        MaterializeCompilationUnitOptions {
            stable_cone_key,
            source_cones: &HashMap::new(),
            request_source_paths,
            request_root_mode: MaterializeRequestRootMode::RequestSources,
            opt_level,
        },
    )
}

pub fn materialize_compilation_unit_from_typechecked_inputs_with_options(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    options: MaterializeCompilationUnitOptions<'_>,
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    materialize::materialize_compilation_unit_from_typechecked_inputs(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        options,
    )
}

/// 一个源文件 lowering 后的 MIR（当前阶段主要用于 dump/fixtures）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct File {
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ProductionSiteContext<'a> {
    fqn: &'a str,
    block: BasicBlockId,
    span: Span,
}

fn gc_intrinsic_operation(callee_fqn: &str) -> Option<GcIntrinsicOperation> {
    match callee_fqn {
        "scoop.core.GC.pin" => Some(GcIntrinsicOperation::Pin),
        "scoop.core.GC.unpin" => Some(GcIntrinsicOperation::Unpin),
        "scoop.core.GC.handleNew" => Some(GcIntrinsicOperation::HandleNew),
        "scoop.core.GC.handleGet" => Some(GcIntrinsicOperation::HandleGet),
        "scoop.core.GC.handleDrop" => Some(GcIntrinsicOperation::HandleDrop),
        _ => None,
    }
}

impl File {
    /// Production verifier for MIR stage output.
    ///
    /// Dump/debug paths may still inspect incomplete MIR, but the production stage must
    /// not successfully hand off executable placeholders or ambiguous return/site contracts.
    pub fn validate_production(
        &self,
        types: &TypeStore,
        unit_ty: TypeId,
        bool_ty: TypeId,
    ) -> Result<(), MirValidationError> {
        for item in &self.items {
            match item {
                Item::Fun(fun) => self.validate_production_fun(fun, types, unit_ty, bool_ty)?,
                Item::InitializerRoot(_) | Item::ExternGlobal(_) | Item::Metadata(_) => {}
                Item::Todo { span, kind } => {
                    return Err(MirValidationError::ProductionTodo {
                        fqn: "<file>".to_string(),
                        block: None,
                        span: *span,
                        category: MirPlaceholderCategory::Item,
                        reason: kind.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_production_fun(
        &self,
        fun: &FunDecl,
        types: &TypeStore,
        unit_ty: TypeId,
        bool_ty: TypeId,
    ) -> Result<(), MirValidationError> {
        let Some(body) = &fun.body else {
            return Ok(());
        };

        body.validate_direct_style().map_err(|error| match error {
            MirValidationError::Todo {
                block,
                span,
                category,
                reason,
            } => MirValidationError::ProductionTodo {
                fqn: fun.fqn.clone(),
                block: Some(block),
                span,
                category,
                reason: reason.clone(),
            },
            other => MirValidationError::ProductionBodyContract {
                fqn: fun.fqn.clone(),
                error: Box::new(other),
            },
        })?;
        self.validate_production_signature(fun, body)?;

        for (index, block) in body.blocks.iter().enumerate() {
            let block_id = BasicBlockId(index as u32);

            for stmt in &block.stmts {
                self.validate_production_statement(&fun.fqn, body, block_id, stmt, types)?;
            }

            self.validate_production_unwind(
                &fun.fqn,
                block_id,
                block.terminator.span,
                &block.terminator.unwind,
            )?;
            self.validate_production_terminator(fun, body, block_id, block, unit_ty, bool_ty)?;
        }

        Ok(())
    }

    fn validate_production_statement(
        &self,
        fqn: &str,
        body: &Body,
        block: BasicBlockId,
        stmt: &Statement,
        types: &TypeStore,
    ) -> Result<(), MirValidationError> {
        match &stmt.kind {
            StatementKind::Assign { target, value } => {
                let result_ty = self.validate_production_local(
                    fqn,
                    Some(block),
                    stmt.span,
                    body,
                    *target,
                    "assignment target",
                )?;
                self.validate_production_rvalue(
                    ProductionSiteContext {
                        fqn,
                        block,
                        span: stmt.span,
                    },
                    body,
                    Some(result_ty),
                    value,
                    types,
                )
            }
            StatementKind::StoreMember {
                continuation_route: StoredContinuationRoutePublication::Ambiguous,
                ..
            } => Err(MirValidationError::ProductionTransportMetadata {
                fqn: fqn.to_string(),
                block,
                span: stmt.span,
                transport: "member store continuation route",
                detail: "ambiguous continuation route must be split or rejected before handoff",
            }),
            StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                ..
            } => {
                if !matches!(receiver, Operand::Local(_)) {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span: stmt.span,
                        surface: "member store receiver",
                        detail: "member store receiver must be a local place",
                    });
                }
                let receiver_ty = self.validate_production_operand(
                    fqn,
                    block,
                    stmt.span,
                    body,
                    "member store receiver",
                    receiver,
                )?;
                if receiver_ty.is_some_and(|ty| ty != member.receiver_ty) {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span: stmt.span,
                        surface: "member store receiver",
                        detail: "receiver operand type and member receiver type disagree",
                    });
                }
                let _value_operand_ty = self.validate_production_operand(
                    fqn,
                    block,
                    stmt.span,
                    body,
                    "member store value",
                    value,
                )?;
                let _ = value_ty;
                Ok(())
            }
            StatementKind::StoreTopLevelVar {
                value, value_ty, ..
            } => {
                let _operand_ty = self.validate_production_operand(
                    fqn,
                    block,
                    stmt.span,
                    body,
                    "top-level store value",
                    value,
                )?;
                let _ = value_ty;
                Ok(())
            }
            StatementKind::Todo(reason) => Err(MirValidationError::ProductionTodo {
                fqn: fqn.to_string(),
                block: Some(block),
                span: stmt.span,
                category: MirPlaceholderCategory::Statement,
                reason: reason.clone(),
            }),
            StatementKind::Nop => Ok(()),
        }
    }

    fn validate_production_signature(
        &self,
        fun: &FunDecl,
        body: &Body,
    ) -> Result<(), MirValidationError> {
        for param in &fun.params {
            let local = body
                .locals
                .get(param.local.as_u32() as usize)
                .ok_or_else(|| MirValidationError::TypeContract {
                    fqn: fun.fqn.clone(),
                    block: None,
                    span: param.span,
                    surface: "parameter local",
                    detail: "parameter local is outside the body local table",
                })?;
            if local.ty != param.ty {
                return Err(MirValidationError::TypeContract {
                    fqn: fun.fqn.clone(),
                    block: None,
                    span: param.span,
                    surface: "parameter type",
                    detail: "parameter type and parameter local type disagree",
                });
            }
        }
        Ok(())
    }

    fn validate_production_local(
        &self,
        fqn: &str,
        block: Option<BasicBlockId>,
        span: Span,
        body: &Body,
        local: LocalId,
        surface: &'static str,
    ) -> Result<TypeId, MirValidationError> {
        body.locals
            .get(local.as_u32() as usize)
            .map(|decl| decl.ty)
            .ok_or_else(|| MirValidationError::TypeContract {
                fqn: fqn.to_string(),
                block,
                span,
                surface,
                detail: "local reference is outside the body local table",
            })
    }

    fn validate_production_operand(
        &self,
        fqn: &str,
        block: BasicBlockId,
        span: Span,
        body: &Body,
        surface: &'static str,
        operand: &Operand,
    ) -> Result<Option<TypeId>, MirValidationError> {
        match operand {
            Operand::Local(local) => self
                .validate_production_local(fqn, Some(block), span, body, *local, surface)
                .map(Some),
            Operand::Const(_) => Ok(None),
        }
    }

    fn validate_production_bool_operand(
        &self,
        site: ProductionSiteContext<'_>,
        body: &Body,
        surface: &'static str,
        operand: &Operand,
        bool_ty: TypeId,
    ) -> Result<(), MirValidationError> {
        let operand_ty = match operand {
            Operand::Local(local) => self.validate_production_local(
                site.fqn,
                Some(site.block),
                site.span,
                body,
                *local,
                surface,
            )?,
            Operand::Const(ConstValue::Bool(_)) => return Ok(()),
            Operand::Const(_) => {
                return Err(MirValidationError::TypeContract {
                    fqn: site.fqn.to_string(),
                    block: Some(site.block),
                    span: site.span,
                    surface,
                    detail: "branch condition operand must have Bool type",
                });
            }
        };
        let _ = (operand_ty, bool_ty);
        Ok(())
    }

    fn validate_production_rvalue(
        &self,
        site: ProductionSiteContext<'_>,
        body: &Body,
        result_ty: Option<TypeId>,
        value: &Rvalue,
        types: &TypeStore,
    ) -> Result<(), MirValidationError> {
        let fqn = site.fqn;
        let block = site.block;
        let span = site.span;
        match value {
            Rvalue::Todo(reason) => Err(MirValidationError::ProductionTodo {
                fqn: fqn.to_string(),
                block: Some(block),
                span,
                category: MirPlaceholderCategory::Rvalue,
                reason: reason.clone(),
            }),
            Rvalue::Use(operand) => self
                .validate_production_operand(fqn, block, span, body, "source value", operand)
                .map(|_| ()),
            Rvalue::Transport { value, transport } => {
                let source_ty = self.validate_production_operand(
                    fqn,
                    block,
                    span,
                    body,
                    "transport value",
                    value,
                )?;
                self.validate_value_transport(site, "value erasure", source_ty, transport)
            }
            .and_then(|()| {
                let Some(boxing) = &transport.boxing else {
                    return Err(MirValidationError::ProductionTransportMetadata {
                        fqn: fqn.to_string(),
                        block,
                        span,
                        transport: "value erasure",
                        detail: "value erasure transport is missing boxing intent",
                    });
                };
                if !matches!(
                    boxing.reason,
                    MirBoxingReason::AnyErasure | MirBoxingReason::RefErasure
                ) {
                    return Err(MirValidationError::ProductionTransportMetadata {
                        fqn: fqn.to_string(),
                        block,
                        span,
                        transport: "value erasure",
                        detail: "value erasure transport must use AnyErasure or RefErasure reason",
                    });
                }
                if boxing.target_ty.is_none() {
                    return Err(MirValidationError::ProductionTransportMetadata {
                        fqn: fqn.to_string(),
                        block,
                        span,
                        transport: "value erasure",
                        detail: "value erasure boxing intent must publish target type",
                    });
                }
                if result_ty.is_some_and(|target_ty| boxing.target_ty != Some(target_ty)) {
                    return Err(MirValidationError::ProductionTransportMetadata {
                        fqn: fqn.to_string(),
                        block,
                        span,
                        transport: "value erasure",
                        detail: "value erasure boxing target type and assignment target disagree",
                    });
                }
                Ok(())
            }),
            Rvalue::Call {
                kind,
                args,
                transport,
                ..
            } => {
                for arg in args {
                    self.validate_production_operand(
                        fqn,
                        block,
                        arg.span,
                        body,
                        "call argument",
                        &arg.value,
                    )?;
                }
                self.validate_production_call_kind(fqn, block, span, kind)?;
                self.validate_value_transport(site, "call result", result_ty, &transport.result)?;
                if let Some(aggregate_return) = &transport.aggregate_return {
                    self.validate_value_transport(
                        site,
                        "call aggregate return",
                        result_ty,
                        aggregate_return,
                    )?;
                }
                if let Some(array) = &transport.array {
                    self.validate_value_transport(
                        site,
                        "array element",
                        Some(array.element_ty),
                        &array.element,
                    )?;
                }
                self.validate_gc_intrinsic_transport(site, kind, args, result_ty, transport)?;
                Ok(())
            }
            Rvalue::EnumVariant {
                enum_ty,
                args,
                payload,
                ..
            } => {
                let mut expected_fields = Vec::with_capacity(args.len());
                for arg in args {
                    expected_fields.push((
                        arg.name.as_deref(),
                        self.validate_production_operand(
                            site.fqn,
                            block,
                            arg.span,
                            body,
                            "enum payload argument",
                            &arg.value,
                        )?,
                    ));
                }
                self.validate_aggregate_transport(
                    site,
                    "enum payload",
                    *enum_ty,
                    AggregateTransportKind::EnumPayload,
                    &expected_fields,
                    payload,
                )
            }
            Rvalue::MakeTuple {
                elements,
                transport,
            } => {
                let mut expected_fields = Vec::with_capacity(elements.len());
                for element in elements {
                    expected_fields.push((
                        None,
                        self.validate_production_operand(
                            fqn,
                            block,
                            span,
                            body,
                            "tuple aggregate element",
                            element,
                        )?,
                    ));
                }
                self.validate_aggregate_transport(
                    site,
                    "tuple aggregate",
                    result_ty.unwrap_or(transport.aggregate_ty),
                    transport.kind,
                    &expected_fields,
                    transport,
                )
            }
            Rvalue::StructLit { fields, transport } => {
                let mut expected_fields = Vec::with_capacity(fields.len());
                for field in fields {
                    expected_fields.push((
                        Some(field.name.as_str()),
                        self.validate_production_operand(
                            fqn,
                            block,
                            field.span,
                            body,
                            "struct aggregate field",
                            &field.value,
                        )?,
                    ));
                }
                self.validate_aggregate_transport(
                    site,
                    "struct aggregate",
                    result_ty.unwrap_or(transport.aggregate_ty),
                    AggregateTransportKind::Struct,
                    &expected_fields,
                    transport,
                )
            }
            Rvalue::MakeClosure {
                env, env_contract, ..
            } => {
                if self
                    .validate_production_operand(fqn, block, span, body, "closure env", env)?
                    .is_some_and(|env_ty| env_ty != env_contract.env_ty)
                {
                    return Err(MirValidationError::ProductionTransportMetadata {
                        fqn: fqn.to_string(),
                        block,
                        span,
                        transport: "closure env",
                        detail: "closure env type and env operand type disagree",
                    });
                }
                Ok(())
            }
            Rvalue::TypeCheck {
                value,
                test_ty,
                metadata,
                ..
            } => self.validate_type_test_metadata(
                site,
                self.validate_production_operand(fqn, block, span, body, "typecheck value", value)?,
                *test_ty,
                metadata,
            ),
            Rvalue::Cast {
                op,
                value,
                target_ty,
                metadata,
                ..
            } => self.validate_cast_metadata(
                site,
                *op,
                self.validate_production_operand(fqn, block, span, body, "cast value", value)?,
                *target_ty,
                result_ty,
                metadata,
            ),
            Rvalue::PatternMatch { subject, pattern } => self.validate_pattern_metadata(
                site,
                self.validate_production_operand(
                    fqn,
                    block,
                    span,
                    body,
                    "pattern subject",
                    subject,
                )?,
                pattern,
            ),
            Rvalue::MemberAccess { receiver, .. } => self
                .validate_production_operand(fqn, block, span, body, "member receiver", receiver)
                .map(|_| ()),
            Rvalue::ClassCtor {
                class_fqn,
                ctor,
                args,
                ..
            } => {
                let Some(result_ty) = result_ty else {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span,
                        surface: "class constructor result",
                        detail: "class constructor rvalue must assign to a typed target local",
                    });
                };
                let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(result_ty) else {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span,
                        surface: "class constructor result",
                        detail: "class constructor result target must have class reference type",
                    });
                };
                if nominal.fqn.as_str() != class_fqn {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span,
                        surface: "class constructor result",
                        detail: "class constructor result target and class metadata disagree",
                    });
                }
                if ctor.ordered_param_count != args.len() {
                    return Err(MirValidationError::TypeContract {
                        fqn: fqn.to_string(),
                        block: Some(block),
                        span,
                        surface: "class constructor arguments",
                        detail: "ordered parameter count and lowered argument count disagree",
                    });
                }
                for arg in args {
                    if arg.name.is_some() {
                        return Err(MirValidationError::TypeContract {
                            fqn: fqn.to_string(),
                            block: Some(block),
                            span: arg.span,
                            surface: "class constructor arguments",
                            detail: "materialized constructor arguments must be positional",
                        });
                    }
                    self.validate_production_operand(
                        fqn,
                        block,
                        arg.span,
                        body,
                        "class constructor argument",
                        &arg.value,
                    )?;
                }
                Ok(())
            }
            Rvalue::InterpolatedString { .. } => Err(MirValidationError::TypeContract {
                fqn: fqn.to_string(),
                block: Some(block),
                span,
                surface: "interpolated string",
                detail: "interpolated strings must be desugared before MIR codegen",
            }),
            Rvalue::TupleGet { tuple, .. } => self
                .validate_production_operand(fqn, block, span, body, "tuple get source", tuple)
                .map(|_| ()),
            Rvalue::PatternExtract { subject, .. } => self
                .validate_production_operand(
                    fqn,
                    block,
                    span,
                    body,
                    "pattern extract subject",
                    subject,
                )
                .map(|_| ()),
            _ => Ok(()),
        }
    }

    fn validate_type_test_metadata(
        &self,
        site: ProductionSiteContext<'_>,
        expected_source_ty: Option<TypeId>,
        expected_target_ty: TypeId,
        metadata: &RuntimeTypeTestMetadata,
    ) -> Result<(), MirValidationError> {
        if expected_source_ty.is_some_and(|source_ty| metadata.source_ty != source_ty) {
            return Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                primitive: "typecheck",
                detail: "source type and operand type disagree",
            });
        }
        if metadata.target_ty != expected_target_ty || metadata.descriptor.ty != expected_target_ty
        {
            return Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                primitive: "typecheck",
                detail: "target type and runtime descriptor disagree",
            });
        }
        Ok(())
    }

    fn validate_value_transport(
        &self,
        site: ProductionSiteContext<'_>,
        transport: &'static str,
        expected_source_ty: Option<TypeId>,
        metadata: &ValueTransportMetadata,
    ) -> Result<(), MirValidationError> {
        if expected_source_ty.is_some_and(|source_ty| metadata.source_ty != source_ty) {
            return Err(MirValidationError::ProductionTransportMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                transport,
                detail: "source type and operand/local type disagree",
            });
        }
        if metadata
            .boxing
            .as_ref()
            .is_some_and(|boxing| boxing.source_ty != metadata.source_ty)
        {
            return Err(MirValidationError::ProductionTransportMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                transport,
                detail: "boxing source type and transport source type disagree",
            });
        }
        Ok(())
    }

    fn validate_gc_intrinsic_transport(
        &self,
        site: ProductionSiteContext<'_>,
        kind: &CallKind,
        args: &[CallArg],
        result_ty: Option<TypeId>,
        transport: &CallTransportMetadata,
    ) -> Result<(), MirValidationError> {
        let direct_operation = match kind {
            CallKind::Direct { callee_fqn } => gc_intrinsic_operation(callee_fqn),
            CallKind::Closure { .. }
            | CallKind::FunValue { .. }
            | CallKind::FunPtr { .. }
            | CallKind::Virtual { .. }
            | CallKind::Interface { .. }
            | CallKind::Resume { .. } => None,
        };

        let Some(gc) = &transport.gc else {
            if direct_operation.is_some() {
                return Err(MirValidationError::ProductionTransportMetadata {
                    fqn: site.fqn.to_string(),
                    block: site.block,
                    span: site.span,
                    transport: "GC intrinsic",
                    detail: "GC intrinsic call is missing pin/handle policy metadata",
                });
            }
            return Ok(());
        };

        if direct_operation.is_some() && (args.len() != 1 || args[0].name.is_some()) {
            return Err(MirValidationError::TypeContract {
                fqn: site.fqn.to_string(),
                block: Some(site.block),
                span: site.span,
                surface: "GC intrinsic call arguments",
                detail: "GC intrinsic calls must have one positional argument",
            });
        }

        let Some(expected_operation) = gc_intrinsic_operation(&gc.callee_fqn) else {
            return Err(MirValidationError::ProductionTransportMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                transport: "GC intrinsic",
                detail: "GC intrinsic metadata is missing callee identity",
            });
        };

        let detail = if direct_operation.is_some_and(|operation| operation != expected_operation) {
            Some("direct GC call metadata does not match callee")
        } else if gc.operation != expected_operation {
            Some("GC intrinsic operation does not match callee")
        } else if !gc.unsafe_required {
            Some("GC intrinsic metadata must preserve unsafe requirement")
        } else if gc.subject.source_ty != gc.subject_ty {
            Some("GC intrinsic subject transport type disagrees with subject type")
        } else if result_ty.is_some_and(|ty| transport.result.source_ty != ty) {
            Some("GC intrinsic result transport type disagrees with assignment target")
        } else {
            match gc.operation {
                GcIntrinsicOperation::Pin
                    if gc.root_lifetime != GcRootLifetime::PinnedUntilUnpin
                        || gc.pairing != GcIntrinsicPairing::PinMustPairUnpin
                        || gc.token_ty.is_none()
                        || result_ty.is_some_and(|ty| gc.token_ty != Some(ty)) =>
                {
                    Some("GC.pin metadata must publish pinned lifetime and unpin pairing")
                }
                GcIntrinsicOperation::Unpin
                    if gc.root_lifetime != GcRootLifetime::EndsPinnedRoot
                        || gc.pairing != GcIntrinsicPairing::UnpinMatchesPin =>
                {
                    Some("GC.unpin metadata must publish pinned-root release pairing")
                }
                GcIntrinsicOperation::HandleNew
                    if gc.root_lifetime != GcRootLifetime::StableHandleUntilDrop
                        || gc.pairing != GcIntrinsicPairing::HandleNewMustPairDrop
                        || gc.token_ty.is_none()
                        || result_ty.is_some_and(|ty| gc.token_ty != Some(ty)) =>
                {
                    Some(
                        "GC.handleNew metadata must publish stable-handle lifetime and drop pairing",
                    )
                }
                GcIntrinsicOperation::HandleGet
                    if gc.root_lifetime != GcRootLifetime::BorrowedFromStableHandle
                        || gc.pairing != GcIntrinsicPairing::HandleGetRequiresLiveHandle =>
                {
                    Some("GC.handleGet metadata must require a live stable handle")
                }
                GcIntrinsicOperation::HandleDrop
                    if gc.root_lifetime != GcRootLifetime::EndsStableHandle
                        || gc.pairing != GcIntrinsicPairing::HandleDropMatchesHandleNew =>
                {
                    Some("GC.handleDrop metadata must publish stable-handle release pairing")
                }
                _ => None,
            }
        };

        if let Some(detail) = detail {
            return Err(MirValidationError::ProductionTransportMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                transport: "GC intrinsic",
                detail,
            });
        }

        self.validate_value_transport(
            site,
            "GC intrinsic subject",
            Some(gc.subject_ty),
            &gc.subject,
        )
    }

    fn validate_aggregate_transport(
        &self,
        site: ProductionSiteContext<'_>,
        transport: &'static str,
        expected_aggregate_ty: TypeId,
        expected_kind: AggregateTransportKind,
        expected_fields: &[(Option<&str>, Option<TypeId>)],
        metadata: &AggregateTransportMetadata,
    ) -> Result<(), MirValidationError> {
        let detail = if metadata.aggregate_ty != expected_aggregate_ty {
            Some("aggregate transport type and result/source type disagree")
        } else if metadata.kind != expected_kind {
            Some("aggregate transport kind is wrong for this MIR node")
        } else if metadata.fields.len() != expected_fields.len() {
            Some("aggregate transport field count does not match lowered values")
        } else if metadata
            .fields
            .iter()
            .enumerate()
            .any(|(index, field)| field.index != index || field.ty != field.transport.source_ty)
        {
            Some("aggregate transport field metadata is inconsistent")
        } else if metadata.fields.iter().zip(expected_fields.iter()).any(
            |(field, (expected_name, _))| {
                expected_name.is_some_and(|name| field.name.as_deref() != Some(name))
            },
        ) {
            Some("aggregate transport field name does not match lowered value")
        } else if metadata
            .fields
            .iter()
            .zip(expected_fields.iter())
            .any(|(field, (_, expected_ty))| expected_ty.is_some_and(|ty| field.ty != ty))
        {
            Some("aggregate transport field type does not match lowered value")
        } else {
            None
        };

        if let Some(detail) = detail {
            return Err(MirValidationError::ProductionTransportMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                transport,
                detail,
            });
        }
        for field in &metadata.fields {
            self.validate_value_transport(site, transport, Some(field.ty), &field.transport)?;
        }
        Ok(())
    }

    fn validate_cast_metadata(
        &self,
        site: ProductionSiteContext<'_>,
        op: ast::CastOp,
        expected_source_ty: Option<TypeId>,
        expected_target_ty: TypeId,
        expected_result_ty: Option<TypeId>,
        metadata: &RuntimeCastMetadata,
    ) -> Result<(), MirValidationError> {
        self.validate_type_test_metadata(
            site,
            expected_source_ty,
            expected_target_ty,
            &metadata.test,
        )?;

        match (op, &metadata.failure, &metadata.result) {
            (
                ast::CastOp::As,
                RuntimeCastFailure::Panic { message },
                RuntimeCastResult::Target { ty },
            ) if message == "class cast failed"
                && *ty == expected_target_ty
                && expected_result_ty.is_none_or(|result_ty| result_ty == expected_target_ty) =>
            {
                Ok(())
            }
            (
                ast::CastOp::AsQ,
                RuntimeCastFailure::ReturnNone,
                RuntimeCastResult::Option { some_ty, .. },
            ) if *some_ty == expected_target_ty => {
                if let (Some(result_ty), RuntimeCastResult::Option { option_ty, .. }) =
                    (expected_result_ty, &metadata.result)
                    && *option_ty != result_ty
                {
                    return Err(MirValidationError::ProductionRuntimeValueMetadata {
                        fqn: site.fqn.to_string(),
                        block: site.block,
                        span: site.span,
                        primitive: "cast",
                        detail: "optional result type and assignment target disagree",
                    });
                }
                Ok(())
            }
            _ => Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                primitive: "cast",
                detail: "failure/result contract does not match cast operator",
            }),
        }
    }

    fn validate_pattern_metadata(
        &self,
        site: ProductionSiteContext<'_>,
        expected_subject_ty: Option<TypeId>,
        pattern: &Pattern,
    ) -> Result<(), MirValidationError> {
        match pattern {
            Pattern::Is { ty, metadata } => {
                if expected_subject_ty.is_some_and(|subject_ty| metadata.subject_ty != subject_ty) {
                    return Err(MirValidationError::ProductionRuntimeValueMetadata {
                        fqn: site.fqn.to_string(),
                        block: site.block,
                        span: site.span,
                        primitive: "pattern type test",
                        detail: "subject type and operand type disagree",
                    });
                }
                if metadata.target_ty != *ty || metadata.descriptor.ty != *ty {
                    return Err(MirValidationError::ProductionRuntimeValueMetadata {
                        fqn: site.fqn.to_string(),
                        block: site.block,
                        span: site.span,
                        primitive: "pattern type test",
                        detail: "target type and runtime descriptor disagree",
                    });
                }
                Ok(())
            }
            Pattern::Or { pats } => {
                for pat in pats {
                    self.validate_pattern_metadata(site, expected_subject_ty, pat)?;
                }
                Ok(())
            }
            Pattern::Tuple { elements } => {
                for element in elements {
                    self.validate_pattern_metadata(site, None, element)?;
                }
                Ok(())
            }
            Pattern::Variant { args, .. } => {
                for arg in args {
                    self.validate_pattern_metadata(site, None, arg)?;
                }
                Ok(())
            }
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::Bind { .. }
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => Ok(()),
        }
    }

    fn validate_production_call_kind(
        &self,
        fqn: &str,
        block: BasicBlockId,
        span: Span,
        kind: &CallKind,
    ) -> Result<(), MirValidationError> {
        match kind {
            CallKind::Direct { callee_fqn } if callee_fqn.is_empty() => {
                Err(MirValidationError::ProductionSiteMetadata {
                    fqn: fqn.to_string(),
                    block,
                    span,
                    site: MirSiteMetadataKind::Call,
                    detail: "direct call is missing callee identity",
                })
            }
            CallKind::Closure { fn_ptr, .. } if fn_ptr.is_empty() => {
                Err(MirValidationError::ProductionSiteMetadata {
                    fqn: fqn.to_string(),
                    block,
                    span,
                    site: MirSiteMetadataKind::Call,
                    detail: "closure call is missing invoke function identity",
                })
            }
            CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. }
                if dispatch.owner_fqn.is_empty()
                    || dispatch.member_name.is_empty()
                    || dispatch.member_fqn.is_empty() =>
            {
                Err(MirValidationError::ProductionSiteMetadata {
                    fqn: fqn.to_string(),
                    block,
                    span,
                    site: MirSiteMetadataKind::Call,
                    detail: "dispatch call is missing selected member identity",
                })
            }
            CallKind::Resume { resume, .. } if resume.runtime_error_effect_ty.is_none() => {
                Err(MirValidationError::ProductionSiteMetadata {
                    fqn: fqn.to_string(),
                    block,
                    span,
                    site: MirSiteMetadataKind::Resume,
                    detail: "resume call is missing runtime-error effect metadata",
                })
            }
            CallKind::Direct { .. }
            | CallKind::Closure { .. }
            | CallKind::FunValue { .. }
            | CallKind::FunPtr { .. }
            | CallKind::Virtual { .. }
            | CallKind::Interface { .. }
            | CallKind::Resume { .. } => Ok(()),
        }
    }

    fn validate_production_unwind(
        &self,
        fqn: &str,
        block: BasicBlockId,
        span: Span,
        unwind: &UnwindAction,
    ) -> Result<(), MirValidationError> {
        match unwind {
            UnwindAction::Todo(reason) => Err(MirValidationError::ProductionTodo {
                fqn: fqn.to_string(),
                block: Some(block),
                span,
                category: MirPlaceholderCategory::UnwindAction,
                reason: reason.clone(),
            }),
            UnwindAction::NoUnwind | UnwindAction::Propagate | UnwindAction::Cleanup { .. } => {
                Ok(())
            }
        }
    }

    fn validate_production_terminator(
        &self,
        fun: &FunDecl,
        body: &Body,
        block_id: BasicBlockId,
        block: &BasicBlock,
        unit_ty: TypeId,
        bool_ty: TypeId,
    ) -> Result<(), MirValidationError> {
        match &block.terminator.kind {
            TerminatorKind::Return { value: None } if fun.return_ty != unit_ty => {
                Err(MirValidationError::ProductionMissingReturnValue {
                    fqn: fun.fqn.clone(),
                    block: block_id,
                    span: block.terminator.span,
                    return_ty: fun.return_ty,
                })
            }
            TerminatorKind::Todo(reason) => Err(MirValidationError::ProductionTodo {
                fqn: fun.fqn.clone(),
                block: Some(block_id),
                span: block.terminator.span,
                category: MirPlaceholderCategory::Terminator,
                reason: reason.clone(),
            }),
            TerminatorKind::Perform {
                op_fqn,
                metadata,
                args,
                ..
            } => {
                let site = ProductionSiteContext {
                    fqn: &fun.fqn,
                    block: block_id,
                    span: block.terminator.span,
                };
                self.validate_production_perform(
                    site,
                    body,
                    op_fqn,
                    metadata,
                    args,
                    &block.terminator.unwind,
                )
            }
            TerminatorKind::Handle {
                metadata,
                arms,
                has_finally,
                ..
            } => {
                let site = ProductionSiteContext {
                    fqn: &fun.fqn,
                    block: block_id,
                    span: block.terminator.span,
                };
                self.validate_production_handle(site, body, metadata, arms, *has_finally)
            }
            TerminatorKind::Return { value: Some(value) } => {
                self.validate_production_operand(
                    &fun.fqn,
                    block_id,
                    block.terminator.span,
                    body,
                    "return value",
                    value,
                )?;
                Ok(())
            }
            TerminatorKind::Return { value: None }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::Unreachable => Ok(()),
            TerminatorKind::CondBr { cond, .. } => self.validate_production_bool_operand(
                ProductionSiteContext {
                    fqn: &fun.fqn,
                    block: block_id,
                    span: block.terminator.span,
                },
                body,
                "branch condition",
                cond,
                bool_ty,
            ),
        }
    }

    fn validate_production_perform(
        &self,
        site: ProductionSiteContext<'_>,
        body: &Body,
        op_fqn: &str,
        metadata: &PerformMetadata,
        args: &[PerformArg],
        unwind: &UnwindAction,
    ) -> Result<(), MirValidationError> {
        for arg in args {
            self.validate_production_operand(
                site.fqn,
                site.block,
                arg.span,
                body,
                "perform payload arg",
                &arg.value,
            )?;
        }
        let detail = if op_fqn.is_empty() {
            Some("perform terminator is missing effect operation identity")
        } else if metadata.arg_mapping.len() != args.len() {
            Some("perform metadata arg mapping does not match lowered payload args")
        } else if metadata.payload_component_tys.len() != args.len() {
            Some("perform metadata payload component types do not match lowered payload args")
        } else if metadata.payload_transport.len() != args.len() {
            Some("perform metadata payload transports do not match lowered payload args")
        } else if metadata
            .arg_mapping
            .iter()
            .copied()
            .ne(args.iter().map(|arg| arg.source_arg_index))
        {
            Some("perform metadata arg mapping disagrees with lowered payload args")
        } else if metadata
            .payload_transport
            .iter()
            .zip(metadata.payload_component_tys.iter().copied())
            .any(|(transport, ty)| transport.source_ty != ty)
        {
            Some("perform payload transport type disagrees with payload component type")
        } else if self.perform_payload_transport_mismatches_operand(site, body, metadata, args)? {
            Some("perform payload transport type does not match lowered payload value")
        } else if matches!(unwind, UnwindAction::NoUnwind) {
            Some("perform terminator is missing an explicit unwind action")
        } else {
            None
        };

        if let Some(detail) = detail {
            return Err(MirValidationError::ProductionSiteMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                site: MirSiteMetadataKind::Perform,
                detail,
            });
        }

        for (transport, component_ty) in metadata
            .payload_transport
            .iter()
            .zip(metadata.payload_component_tys.iter().copied())
        {
            self.validate_value_transport(site, "perform payload", Some(component_ty), transport)?;
        }

        Ok(())
    }

    fn perform_payload_transport_mismatches_operand(
        &self,
        site: ProductionSiteContext<'_>,
        body: &Body,
        metadata: &PerformMetadata,
        args: &[PerformArg],
    ) -> Result<bool, MirValidationError> {
        for (transport, arg) in metadata.payload_transport.iter().zip(args.iter()) {
            let ty = self.validate_production_operand(
                site.fqn,
                site.block,
                arg.span,
                body,
                "perform payload arg",
                &arg.value,
            )?;
            if ty.is_some_and(|ty| transport.source_ty != ty) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn validate_production_handle(
        &self,
        site: ProductionSiteContext<'_>,
        body: &Body,
        metadata: &HandleMetadata,
        arms: &[HandlerArm],
        has_finally: bool,
    ) -> Result<(), MirValidationError> {
        let mut detail = None;
        if has_finally && metadata.finally_result_ty.is_none() {
            detail = Some("handle metadata is missing finally result type");
        } else if !has_finally && metadata.finally_result_ty.is_some() {
            detail = Some("handle metadata has finally result type without a finally boundary");
        } else {
            for arm in arms {
                if arm.op_fqn.is_empty() {
                    detail = Some("handle arm is missing effect operation identity");
                    break;
                }
                if arm.binder_count != arm.binder_locals.len() {
                    detail = Some("handle arm binder metadata does not match binder locals");
                    break;
                }
                if arm.payload_component_tys.len() != arm.binder_count {
                    detail = Some("handle arm payload component metadata does not match binders");
                    break;
                }
                for local in arm
                    .binder_locals
                    .iter()
                    .copied()
                    .chain(arm.continuation_local)
                {
                    if self
                        .validate_production_local(
                            site.fqn,
                            Some(site.block),
                            site.span,
                            body,
                            local,
                            "handle binder local",
                        )
                        .is_err()
                    {
                        detail = Some("handle arm binder local is outside the body local table");
                        break;
                    }
                }
                if detail.is_some() {
                    break;
                }
                if arm.kind == HandlerArmKind::EscapeContinuation
                    && arm.continuation_local.is_none()
                {
                    detail = Some("escaping handle arm is missing continuation binder local");
                    break;
                }
            }
        }

        if let Some(detail) = detail {
            return Err(MirValidationError::ProductionSiteMetadata {
                fqn: site.fqn.to_string(),
                block: site.block,
                span: site.span,
                site: MirSiteMetadataKind::Handle,
                detail,
            });
        }

        Ok(())
    }
}

/// 顶层条目（top-level items）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub enum Item {
    Fun(FunDecl),
    /// Non-function root used by later stages to discover top-level initialization.
    InitializerRoot(InitializerRoot),
    /// External global storage contract published by typed HIR and owned by MIR stage output.
    ExternGlobal(ExternGlobalRoot),
    /// Type/object declaration metadata root owned by MIR stage output.
    Metadata(MetadataRoot),
    /// 未纳入当前阶段 MIR 的条目占位（例如顶层 val/global init、type decl 等）。
    Todo {
        span: Span,
        kind: String,
    },
}

/// A top-level value/object initializer root visible from MIR stage output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitializerRoot {
    pub span: Span,
    pub fqn: String,
    pub source_path: PathBuf,
    pub kind: InitializerRootKind,
    pub ty: Option<TypeId>,
    pub initializer_transport: Option<ValueTransportMetadata>,
    pub has_initializer: bool,
    pub dependencies: Vec<InitializerDependency>,
    pub hidden_effects: EffectRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InitializerRootKind {
    RuntimeImmutableVal,
    RuntimeMutableVar {
        storage: crate::hir::TopLevelVarStorage,
    },
    ObjectSingleton,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InitializerDependency {
    pub fqn: String,
    pub kind: InitializerDependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InitializerDependencyKind {
    TopLevelValue,
    ObjectSingleton,
}

/// MIR-owned class initialization payload consumed by LIR class ctor init lowering.
pub type ClassInitIndex = HashMap<ClassInstanceKey, MonoClassInit>;

/// MIR-facing class layout key; the identity is still authored by the HIR frontend.
pub type ClassInstanceKey = crate::hir::ClassInstanceKey;

/// Source-local symbol id preserved for class ctor source payloads.
pub type SymbolId = crate::hir::SymbolId;

/// Source expression payload preserved behind MIR/LIR names for class ctor lowering.
pub type SourceExpr = crate::hir::Expr;

/// Source block payload preserved behind MIR/LIR names for class ctor lowering.
pub type SourceBlock = crate::hir::Block;

/// Source call argument payload preserved behind MIR/LIR names for class ctor lowering.
pub type SourceCallArg = crate::hir::CallArg;

/// Transitional MIR-owned source payload namespace used by source-body codegen helpers.
///
/// MIR is allowed to depend on the frontend stages; LIR should consume these
/// payloads through explicit LIR contracts instead of re-exporting this module.
pub mod source_payload {
    pub use crate::ast::{BinaryOp, CastOp, TopLevelFunCallBinding, TypeCheckOp, TypeKind};
    pub use crate::hir::*;
    pub mod intrinsics {
        pub use crate::intrinsics::{
            NamedIntrinsicAuditEntry, NamedIntrinsicLoweringMode, NamedIntrinsicRuntimeSignature,
            NamedIntrinsicRuntimeTy, fallback_named_intrinsic_entry_name_for_fqn,
            named_intrinsic_audit_entries, named_intrinsic_audit_entry,
        };
    }
    pub use crate::itable::{
        ClassItableEntry, ClassItableIndex, ITABLE_RECEIVER_REF_TYPE_ID, InterfaceIndex,
    };
    pub use crate::syntax::{char_literal, float_literal, int_literal, string_literal};
    pub use crate::vtable::ClassVtableIndex;
}

/// Monomorphic class init source contract published to LIR without exposing HIR/AST owner names.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonoClassInit {
    pub fqn: String,
    pub source_path: PathBuf,
    pub super_class_fqn: Option<String>,
    pub super_ctor_args_span: Option<Span>,
    pub super_ctor_call: Option<CtorCallInfo>,
    pub super_ctor_args: Vec<SourceCallArg>,
    pub this_id: SymbolId,
    pub fields: Vec<ClassField<MonoTypeId>>,
    pub field_indices: HashMap<String, u32>,
    pub steps: Vec<ClassInitStep>,
    pub ctors: Vec<ClassCtor<MonoTypeId>>,
}

impl MonoClassInit {
    /// Build the MIR-owned payload from the frontend-owned class init side table.
    pub fn from_hir(class: &crate::hir::MonoClassInit) -> Self {
        Self {
            fqn: class.fqn.clone(),
            source_path: class.source_path.clone(),
            super_class_fqn: class.super_class_fqn.clone(),
            super_ctor_args_span: class.super_ctor_args_span,
            super_ctor_call: class.super_ctor_call.as_ref().map(CtorCallInfo::from_hir),
            super_ctor_args: class.super_ctor_args.clone(),
            this_id: class.this_id,
            fields: class.fields.iter().map(ClassField::from_hir).collect(),
            field_indices: class.field_indices.clone(),
            steps: class.steps.iter().map(ClassInitStep::from_hir).collect(),
            ctors: class.ctors.iter().map(ClassCtor::from_hir).collect(),
        }
    }
}

/// Class field source contract for backend-neutral class layout consumers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassField<T> {
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: T,
}

impl<T: Copy> ClassField<T> {
    fn from_hir(field: &crate::hir::ClassField<T>) -> Self {
        Self {
            fqn: field.fqn.clone(),
            name: field.name.clone(),
            mutable: field.mutable,
            ty: field.ty,
        }
    }
}

/// One source-ordered class initialization step consumed by LIR ctor lowering.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ClassInitStep {
    PropertyInit { field_fqn: String, init: SourceExpr },
    InitBlock { block: SourceBlock },
}

impl ClassInitStep {
    fn from_hir(step: &crate::hir::ClassInitStep) -> Self {
        match step {
            crate::hir::ClassInitStep::PropertyInit { field_fqn, init } => Self::PropertyInit {
                field_fqn: field_fqn.clone(),
                init: init.clone(),
            },
            crate::hir::ClassInitStep::InitBlock { block } => Self::InitBlock {
                block: block.clone(),
            },
        }
    }
}

/// Source constructor kind normalized into a MIR-owned enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClassCtorKind {
    Primary,
    Secondary,
}

impl ClassCtorKind {
    fn from_hir(kind: crate::hir::ClassCtorKind) -> Self {
        match kind {
            crate::hir::ClassCtorKind::Primary => Self::Primary,
            crate::hir::ClassCtorKind::Secondary => Self::Secondary,
        }
    }
}

/// A monomorphic class constructor payload consumed by LIR class init lowering.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassCtor<T> {
    pub kind: ClassCtorKind,
    pub span: Span,
    pub params: Vec<ClassCtorParam<T>>,
    pub delegation: Option<ClassCtorDelegation>,
    pub body: Option<SourceBlock>,
}

impl<T: Copy> ClassCtor<T> {
    fn from_hir(ctor: &crate::hir::ClassCtor<T>) -> Self {
        Self {
            kind: ClassCtorKind::from_hir(ctor.kind),
            span: ctor.span,
            params: ctor.params.iter().map(ClassCtorParam::from_hir).collect(),
            delegation: ctor.delegation.as_ref().map(ClassCtorDelegation::from_hir),
            body: ctor.body.clone(),
        }
    }
}

/// Constructor delegation kind normalized away from the AST owner enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClassCtorDelegationKind {
    This,
    Super,
}

impl ClassCtorDelegationKind {
    fn from_ast(kind: ast::CtorDelegationKind) -> Self {
        match kind {
            ast::CtorDelegationKind::This => Self::This,
            ast::CtorDelegationKind::Super => Self::Super,
        }
    }
}

/// Constructor delegation payload with AST ownership hidden behind MIR names.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassCtorDelegation {
    pub kind: ClassCtorDelegationKind,
    pub span: Span,
    pub call: Option<CtorCallInfo>,
    pub args: Vec<SourceCallArg>,
}

impl ClassCtorDelegation {
    fn from_hir(delegation: &crate::hir::ClassCtorDelegation) -> Self {
        Self {
            kind: ClassCtorDelegationKind::from_ast(delegation.kind),
            span: delegation.span,
            call: delegation.call.as_ref().map(CtorCallInfo::from_hir),
            args: delegation.args.clone(),
        }
    }
}

/// Constructor parameter payload published to LIR class init lowering.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassCtorParam<T> {
    pub id: SymbolId,
    pub name: String,
    pub decl_span: Span,
    pub ty: T,
    pub has_default: bool,
    pub default_value: Option<SourceExpr>,
    pub is_property: bool,
    pub property_field_fqn: Option<String>,
}

impl<T: Copy> ClassCtorParam<T> {
    fn from_hir(param: &crate::hir::ClassCtorParam<T>) -> Self {
        Self {
            id: param.id,
            name: param.name.clone(),
            decl_span: param.decl_span,
            ty: param.ty,
            has_default: param.has_default,
            default_value: param.default_value.clone(),
            is_property: param.is_property,
            property_field_fqn: param.property_field_fqn.clone(),
        }
    }
}

/// Constructor call binding payload needed for named/default argument replay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtorCallInfo {
    pub class_fqn: String,
    pub ctor_span: Option<Span>,
    pub arg_mapping: Vec<Option<usize>>,
}

impl CtorCallInfo {
    fn from_hir(call: &crate::hir::CtorCallInfo) -> Self {
        Self {
            class_fqn: call.class_fqn.clone(),
            ctor_span: call.ctor_span,
            arg_mapping: call.arg_mapping.clone(),
        }
    }
}

/// MIR-owned contract for an `@Extern` top-level global variable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternGlobalRoot {
    pub span: Span,
    pub fqn: String,
    pub source_path: PathBuf,
    pub ty: TypeId,
    pub mutable: bool,
    pub symbol: String,
    pub linkage: crate::hir::ExternGlobalLinkage,
    pub storage: crate::hir::TopLevelVarStorage,
    pub initializer_absent: bool,
    pub unsafe_required: bool,
}

/// MIR-owned declaration metadata root for type/object declarations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MetadataRoot {
    TypeAlias(TypeAliasMetadata),
    Nominal(NominalMetadata),
    Object(ObjectMetadata),
    ExtensionProperty(ExtensionPropertyMetadata),
}

impl MetadataRoot {
    pub fn fqn(&self) -> &str {
        match self {
            Self::TypeAlias(alias) => &alias.fqn,
            Self::Nominal(nominal) => &nominal.fqn,
            Self::Object(object) => &object.fqn,
            Self::ExtensionProperty(prop) => &prop.fqn,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeAliasMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub type_params: Vec<DeclTypeParamMetadata>,
    pub ty: TypeId,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NominalMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub kind: ast::TypeKind,
    pub type_params: Vec<DeclTypeParamMetadata>,
    pub supertypes: Vec<SupertypeMetadata>,
    pub interfaces: Vec<String>,
    pub constructors: Vec<CtorMetadata>,
    pub members: Vec<DeclMemberMetadata>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub kind: ast::ObjectKind,
    pub supertypes: Vec<SupertypeMetadata>,
    pub interfaces: Vec<String>,
    pub initializer_root: String,
    pub members: Vec<DeclMemberMetadata>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtensionPropertyMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub type_params: Vec<DeclTypeParamMetadata>,
    pub receiver_ty: TypeId,
    pub ty: TypeId,
    pub getter: Option<AccessorMetadata>,
    pub setter: Option<AccessorMetadata>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeclTypeParamMetadata {
    pub span: Span,
    pub name: String,
    pub variance: Option<ast::TypeParamVariance>,
    pub ty: TypeId,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SupertypeMetadata {
    pub span: Span,
    pub fqn: Option<String>,
    pub ty: TypeId,
    pub ctor_arg_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtorMetadata {
    pub span: Span,
    pub kind: crate::hir::ClassCtorKind,
    pub params: Vec<CtorParamMetadata>,
    pub delegation: Option<ast::CtorDelegationKind>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtorParamMetadata {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub has_default: bool,
    pub property: Option<ast::ValKind>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DeclMemberMetadata {
    Field(FieldMetadata),
    Property(PropertyMetadata),
    Fun(MemberFunMetadata),
    EnumVariant(EnumVariantMetadata),
    InitBlock { span: Span },
    Nested(Box<MetadataRoot>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
    pub origin: crate::hir::FieldOrigin,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
    pub has_backing_field: bool,
    pub getter: Option<AccessorMetadata>,
    pub setter: Option<AccessorMetadata>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccessorMetadata {
    pub span: Span,
    pub fqn: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberFunMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub type_params: Vec<DeclTypeParamMetadata>,
    pub params: Vec<CtorParamMetadata>,
    pub return_ty: TypeId,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnumVariantMetadata {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub fields: Vec<FieldMetadata>,
}

/// 函数声明在 MIR 视图下的承载。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Body>,
}

/// 参数在 MIR 视图下的表示：它同时对应一个 local。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub local: LocalId,
}

/// 基本块 ID（在 `Body::blocks` 内的索引）。
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct BasicBlockId(u32);

impl BasicBlockId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for BasicBlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// 局部变量 ID（在 `Body::locals` 内的索引）。
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LocalId(u32);

impl LocalId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.0)
    }
}

/// 一个函数（或顶层 initializer）在 MIR 中的 body。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Body {
    /// locals 声明表（参数/局部/临时变量；后续会扩展 return local 等约定）。
    pub locals: Vec<LocalDecl>,
    /// 基本块列表（块内顺序执行 statements，最后以 terminator 结束）。
    pub blocks: Vec<BasicBlock>,
    /// CFG 入口块（通常为 `bb0`）。
    pub start: BasicBlockId,
}

impl Body {
    /// 创建一个空 body（调用方需要填充 blocks 并设置 `start`）。
    pub fn new_empty() -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            start: BasicBlockId(0),
        }
    }

    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).expect("too many locals"));
        self.locals.push(decl);
        id
    }

    pub fn push_block(&mut self, bb: BasicBlock) -> BasicBlockId {
        let id = BasicBlockId(u32::try_from(self.blocks.len()).expect("too many basic blocks"));
        self.blocks.push(bb);
        id
    }

    /// 遍历当前 body 内已分配的所有 site id。
    pub fn for_each_site_id(&self, mut f: impl FnMut(SiteId)) {
        for block in &self.blocks {
            for stmt in &block.stmts {
                if let StatementKind::Assign { value, .. } = &stmt.kind
                    && let Some(site_id) = value.site_id()
                {
                    f(site_id);
                }
            }
            if let Some(site_id) = block.terminator.kind.site_id() {
                f(site_id);
            }
        }
    }

    /// 返回当前 body 中尚未使用的最小 `SiteId`。
    ///
    /// 注意：该方法本身不会把 id 写回 body；调用方若要批量生成新节点，应持有返回值并自行递增。
    pub fn next_unused_site_id(&self) -> SiteId {
        let mut next_raw = 0u32;
        self.for_each_site_id(|site_id| {
            next_raw = next_raw.max(site_id.as_u32().saturating_add(1));
        });
        SiteId::from_raw(next_raw)
    }

    /// 检查 CFG 的**结构合法性**：
    /// - `start` 必须在 `blocks` 范围内
    /// - 所有 terminator 的 target 必须在范围内（包含 cleanup/unwind target）
    pub fn validate_cfg(&self) -> Result<(), MirValidationError> {
        if self.blocks.is_empty() {
            return Err(MirValidationError::EmptyBody);
        }
        if self.start.as_usize() >= self.blocks.len() {
            return Err(MirValidationError::InvalidStartBlock {
                start: self.start,
                blocks_len: self.blocks.len(),
            });
        }

        for (idx, block) in self.blocks.iter().enumerate() {
            let from = BasicBlockId(idx as u32);

            // 注意：不分配 Vec，保持验证逻辑轻量；一旦发现无效 target 即返回。
            let mut invalid_target: Option<BasicBlockId> = None;
            block.terminator.for_each_successor(|target| {
                if invalid_target.is_some() {
                    return;
                }
                if target.as_usize() >= self.blocks.len() {
                    invalid_target = Some(target);
                }
            });
            if let Some(target) = invalid_target {
                return Err(MirValidationError::InvalidTarget {
                    from,
                    target,
                    blocks_len: self.blocks.len(),
                });
            }
        }

        Ok(())
    }

    /// 针对 direct-style MIR 的额外形状校验。
    ///
    /// 说明：
    /// - 该验证器建立在 `validate_cfg()` 之上，因此会先检查所有 CFG/cleanup target 是否落在
    ///   `blocks` 范围内；
    /// - 它只约束 P3/P4 会依赖的 direct-style MIR contract，不试图把当前整个 MIR 限制为
    ///   “完全无 Todo”；未纳入本阶段的表达式 lowering 仍可继续用其它 `Todo(...)` 占位。
    pub fn validate_direct_style(&self) -> Result<(), MirValidationError> {
        self.validate_cfg()?;

        let mut seen_site_ids = HashMap::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let block_id = BasicBlockId(index as u32);

            for stmt in &block.stmts {
                self.validate_statement(block_id, stmt)?;
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    continue;
                };
                if let Some(site_id) = value.site_id()
                    && let Some(first_block) = seen_site_ids.insert(site_id, block_id)
                {
                    return Err(MirValidationError::DuplicateSiteId {
                        site_id,
                        first_block,
                        second_block: block_id,
                    });
                }
            }

            self.validate_unwind(block_id, block.terminator.span, &block.terminator.unwind)?;
            self.validate_terminator(block_id, block.terminator.span, &block.terminator.kind)?;
            if let Some(site_id) = block.terminator.kind.site_id()
                && let Some(first_block) = seen_site_ids.insert(site_id, block_id)
            {
                return Err(MirValidationError::DuplicateSiteId {
                    site_id,
                    first_block,
                    second_block: block_id,
                });
            }
        }

        Ok(())
    }

    fn validate_statement(
        &self,
        block: BasicBlockId,
        stmt: &Statement,
    ) -> Result<(), MirValidationError> {
        match &stmt.kind {
            StatementKind::Nop => Ok(()),
            StatementKind::Assign { value, .. } => self.validate_rvalue(block, stmt.span, value),
            StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => Ok(()),
            StatementKind::Todo(reason) => {
                if is_forbidden_effect_todo(reason) {
                    return Err(MirValidationError::Todo {
                        block,
                        span: stmt.span,
                        category: MirPlaceholderCategory::Statement,
                        reason: reason.clone(),
                    });
                }
                Ok(())
            }
        }
    }

    fn validate_rvalue(
        &self,
        block: BasicBlockId,
        span: Span,
        value: &Rvalue,
    ) -> Result<(), MirValidationError> {
        if let Rvalue::Todo(reason) = value
            && is_forbidden_effect_todo(reason)
        {
            return Err(MirValidationError::Todo {
                block,
                span,
                category: MirPlaceholderCategory::Rvalue,
                reason: reason.clone(),
            });
        }
        Ok(())
    }

    fn validate_unwind(
        &self,
        block: BasicBlockId,
        span: Span,
        unwind: &UnwindAction,
    ) -> Result<(), MirValidationError> {
        match unwind {
            UnwindAction::NoUnwind | UnwindAction::Propagate => Ok(()),
            UnwindAction::Cleanup { target } => {
                if !self.blocks[target.as_usize()].is_cleanup {
                    return Err(MirValidationError::CleanupTargetNotMarked {
                        from: block,
                        target: *target,
                    });
                }
                Ok(())
            }
            UnwindAction::Todo(reason) => Err(MirValidationError::Todo {
                block,
                span,
                category: MirPlaceholderCategory::UnwindAction,
                reason: reason.clone(),
            }),
        }
    }

    fn validate_terminator(
        &self,
        block: BasicBlockId,
        span: Span,
        kind: &TerminatorKind,
    ) -> Result<(), MirValidationError> {
        match kind {
            TerminatorKind::Handle {
                arms,
                has_finally,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                if arm_targets.len() != arms.len() {
                    return Err(MirValidationError::InvalidHandleArmTargetCount {
                        from: block,
                        arms_len: arms.len(),
                        targets_len: arm_targets.len(),
                    });
                }
                if finally_target.is_some() != *has_finally {
                    return Err(MirValidationError::InvalidHandleFinallyTarget {
                        from: block,
                        has_finally: *has_finally,
                        finally_target: *finally_target,
                    });
                }
                if let Some(target) = finally_target
                    && !self.blocks[target.as_usize()].is_cleanup
                {
                    return Err(MirValidationError::HandleFinallyTargetNotCleanup {
                        from: block,
                        target: *target,
                    });
                }
                if exit_target.as_usize() >= self.blocks.len() {
                    return Err(MirValidationError::InvalidHandleExitTarget {
                        from: block,
                        target: *exit_target,
                        blocks_len: self.blocks.len(),
                    });
                }
                Ok(())
            }
            TerminatorKind::Todo(reason) if is_forbidden_effect_todo(reason) => {
                Err(MirValidationError::Todo {
                    block,
                    span,
                    category: MirPlaceholderCategory::Terminator,
                    reason: reason.clone(),
                })
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Perform { .. }
            | TerminatorKind::Todo(_) => Ok(()),
        }
    }

    /// 从 `start` 出发，计算可达的基本块集合（按 BFS 顺序返回）。
    pub fn reachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            order.push(bb);

            let block = &self.blocks[bb.as_usize()];
            block.terminator.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        Ok(order)
    }

    /// 检查 CFG 是否“全连通”：所有基本块都从 `start` 可达。
    pub fn is_fully_reachable(&self) -> Result<bool, MirValidationError> {
        let reachable = self.reachable_blocks()?;
        Ok(reachable.len() == self.blocks.len())
    }

    /// 列出不可达的基本块（用于测试与后续 pass 的诊断）。
    pub fn unreachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            let block = &self.blocks[bb.as_usize()];
            block.terminator.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        let mut unreachable = Vec::new();
        for (idx, ok) in visited.iter().copied().enumerate() {
            if !ok {
                unreachable.push(BasicBlockId(idx as u32));
            }
        }
        Ok(unreachable)
    }
}

/// MIR local 的稳定来源分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LocalSourceKind {
    SourceLocal,
    CompilerTemporary,
}

/// 一个 local 的声明信息。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalDecl {
    pub span: Span,
    pub name: Option<String>,
    pub ty: TypeId,
    pub source: LocalSourceKind,
}

/// MIR 基本块：顺序语句 + 终结指令（terminator）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BasicBlock {
    /// 是否为 cleanup block（用于 `finally`/effect unwinding）。
    ///
    /// 该标记本身不影响 CFG 连通性；主要用于 dump/诊断与后续更严格的 MIR 规则。
    pub is_cleanup: bool,
    pub stmts: Vec<Statement>,
    pub terminator: Terminator,
}

/// MIR 语句（顺序执行）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub enum StatementKind {
    Nop,
    /// `target = value`（最小赋值语句，用于 if/when merge 等场景）。
    Assign {
        target: LocalId,
        value: Rvalue,
    },
    /// `receiver.member = value` 的显式 member write contract。
    ///
    /// 说明：
    /// - 该节点保留 member identity、写入值来源，以及 continuation 值穿过 wrapper/aggregate 时的
    ///   published route；
    /// - 它是供后续 effect/late-lowering/LLVM handoff 消费的 compiler-owned contract，而不是 backend-specific
    ///   store lowering；
    /// - `continuation_route=None` 表示该写入值不发布 continuation route；
    /// - `continuation_route=Ambiguous` 表示 lowering 观察到了多个互不兼容的 continuation payload path，
    ///   后续阶段必须显式拒绝而不是自行猜测。
    StoreMember {
        receiver: Operand,
        member: MemberAccessMetadata,
        value: Operand,
        value_ty: TypeId,
        continuation_route: StoredContinuationRoutePublication,
    },
    /// `top.level.var = value` 的显式写入 contract。
    StoreTopLevelVar {
        fqn: String,
        value: Operand,
        value_ty: TypeId,
    },
    /// 未实现节点占位（用于尽早落地数据结构但避免 `todo!()`/panic）。
    Todo(String),
}

/// 一个“可以被使用的值”（最小 operand 模型）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Operand {
    Local(LocalId),
    Const(ConstValue),
}

/// 顶层值/函数引用在 MIR 上保留的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TopLevelRef {
    pub fqn: String,
    pub site_id: Option<SiteId>,
    pub hidden_effects: EffectRow,
}

/// 成员访问在 MIR 上保留的最小语言级 metadata。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemberAccessMetadata {
    pub name: String,
    pub receiver_ty: TypeId,
    pub resolved: Option<MemberTarget>,
    pub hidden_effects: EffectRow,
}

/// 已解析成员在 MIR 上的稳定目标种类。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemberTarget {
    Value { fqn: String },
    Fun { fqn: String },
    ExtensionValue { fqn: String },
    ExtensionFun { fqn: String },
}

/// 调用实参在 MIR 中的最小表示。
///
/// 说明：
/// - `value` 总是已经先被 lowering 为 operand / local，便于后续按 ANF 风格分析求值顺序；
/// - `name` 仅在源级为命名参数时存在；当前阶段保留它，避免后续 pass 被迫回到 HIR 读取调用形状。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallArg {
    pub span: Span,
    pub name: Option<String>,
    pub value: Operand,
}

/// 插值字符串在 MIR 上保留的 ANF 片段。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InterpolatedStringPart {
    Text {
        span: Span,
    },
    Expr {
        span: Span,
        value: Operand,
        ty: TypeId,
    },
}

/// struct literal 在 MIR 上保留的 ANF 字段初始化项。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructLitField {
    pub span: Span,
    pub name: String,
    pub value: Operand,
}

/// MIR-level type metadata literal value.
///
/// Scoop 0.1 keeps runtime `T::class` as a stable type-name string value while retaining the
/// source type identity needed by later stages to upgrade this to richer metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeMetadataLiteral {
    pub source_ty: TypeId,
    pub source_fqn: Option<String>,
    pub kind: TypeMetadataLiteralKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeMetadataLiteralKind {
    TypeNameString,
}

/// `perform` payload 在 MIR 上的一个已排序参数槽位。
///
/// 说明：
/// - `value` 仍按源码求值顺序先被 lower 为 operand/local；
/// - `source_arg_index` 记录该 payload 来自调用点第几个显式实参，便于后续 pass 同时看到
///   “按参数顺序归一化后的 payload 视图”和“原始调用点位置”。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformArg {
    pub span: Span,
    pub source_arg_index: usize,
    pub name: Option<String>,
    pub value: Operand,
}

/// `perform` 调用点在 MIR 上保留的最小 metadata。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformMetadata {
    pub effect_ty: TypeId,
    pub op_type_args: Vec<TypeId>,
    pub result_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub payload_transport: Vec<ValueTransportMetadata>,
    pub arg_mapping: Vec<usize>,
}

impl fmt::Debug for PerformMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("PerformMetadata");
        s.field("effect_ty", &self.effect_ty);
        if !self.op_type_args.is_empty() {
            s.field("op_type_args", &self.op_type_args);
        }
        s.field("result_ty", &self.result_ty);
        s.field("payload_tuple_ty", &self.payload_tuple_ty);
        s.field("payload_component_tys", &self.payload_component_tys);
        s.field("payload_transport", &self.payload_transport);
        s.field("arg_mapping", &self.arg_mapping);
        s.finish()
    }
}

/// virtual / interface dispatch 在 MIR 上保留的最小语言级 metadata。
///
/// 注意：
/// - 这里只保留 receiver 的静态类型与被调成员的声明身份；
/// - 不把 vtable slot / itable id / runtime thunk 等后端细节编码进 MIR。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DispatchMetadata {
    pub owner_fqn: String,
    pub member_name: String,
    pub member_fqn: String,
    pub member_decl_span: Option<Span>,
    pub receiver_ty: TypeId,
}

/// class constructor call 在 MIR 上发布的 selected ctor / ordered-args contract。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClassCtorCallMetadata {
    pub selected_ctor_span: Option<Span>,
    pub ordered_param_count: usize,
}

/// `Continuation.resume(...)` 在 MIR 上保留的最小语义 metadata。
///
/// 注意：
/// - 当前会显式记录 `ResumeTuple` / `Answer` / `Out`，以及 ordinary `Raise<RuntimeError>`
///   required-effect contract；
/// - runtime replay token / payload transport 等细节仍属于更晚的 lowering 阶段。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResumeMetadata {
    pub continuation_ty: TypeId,
    pub resume_ty: TypeId,
    pub answer_ty: TypeId,
    pub return_ty: TypeId,
    pub out_effects: EffectRow,
    pub runtime_error_effect_ty: Option<TypeId>,
    pub suspends_outward: bool,
}

/// `handle { ... } on { ... }` 站点在 MIR 上保留的 typed contract。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleMetadata {
    pub result_ty: TypeId,
    pub body_result_ty: TypeId,
    pub finally_result_ty: Option<TypeId>,
}

/// `handle` arm 在 MIR 上的显式语义 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HandlerArmKind {
    NonResuming,
    EscapeContinuation,
}

/// MIR 上显式区分的调用种类。
///
/// 注意：
/// - 这里刻意只表达语言级调用形态，不表达 LLVM vtable/itable/statepoint 等后端细节；
/// - direct / closure / fun-value / funptr / virtual / interface / resume 共用同一调用层级，
///   避免后续 pass 再回到 HIR 或 LLVM codegen 现场恢复控制转移语义。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CallKind {
    /// 目标函数在 MIR 上已经静态唯一确定。
    Direct { callee_fqn: String },
    /// 已知调用的是某个 closure value。
    ///
    /// `fn_ptr` 记录该 closure 当前可恢复出的唯一 invoke target，便于后续 closure/provenance 分析。
    Closure { callee: Operand, fn_ptr: String },
    /// 调用一个函数值，但当前还不足以把它恢复成更具体的 direct/closure 形态。
    FunValue { callee: Operand },
    /// 调用一个 native `FunPtr<F>`；MIR 显式保留该种类，避免后续阶段再从 carrier/source type 末端回推 ABI family。
    FunPtr { callee: Operand },
    /// class virtual dispatch（语言级“按 receiver 动态分派到 class override”）。
    Virtual {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// interface dispatch（语言级“按 receiver 的 interface 实现做动态分派”）。
    Interface {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// `Continuation.resume(...)`。
    Resume {
        continuation: Operand,
        resume: ResumeMetadata,
    },
}

/// 常量值（当前阶段不保留字面量原始内容，仅保留种类）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConstValue {
    Bool(bool),
    Char,
    Unit,
    Int,
    /// 编译器合成的整数字面量值（当前主要用于 desugaring / compareTo → 0 比较等场景）。
    ///
    /// 说明：
    /// - 与 `Int` 不同，这里显式保留字面量值，避免后续阶段必须回切源码才能恢复 `0` / `1`；
    /// - 目前仍只用于“编译器自身生成”的 `Int` 常量，不改变源码整数字面量继续按 `span` 回切的主路径。
    SynthInt(i64),
    Float64,
    Float32,
    String,
    /// Compiler-generated String literal with decoded UTF-8 contents preserved in MIR.
    SynthString(String),
}

/// 运行期类型检查使用的 descriptor key。
///
/// 该 key 保留 backend 需要的稳定身份，而不是要求后续阶段从 `TypeId` 重新猜测
/// class/interface/function/value 等运行时分类。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeTypeDescriptorKey {
    pub ty: TypeId,
    pub kind: RuntimeTypeDescriptorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeTypeDescriptorKind {
    Any,
    String,
    Nominal {
        fqn: String,
        kind: Option<ast::TypeKind>,
    },
    Function,
    Option,
    Tuple,
    Value,
    TypeParam,
    StarProjection,
    Union,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeTypeStaticFold {
    AlwaysTrue,
    AlwaysFalse,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeTypeParameterizedMatch {
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
pub struct RuntimeTypeTestMetadata {
    pub source_ty: TypeId,
    pub target_ty: TypeId,
    pub descriptor: RuntimeTypeDescriptorKey,
    pub static_fold: RuntimeTypeStaticFold,
    pub parameterized: RuntimeTypeParameterizedMatch,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeCastFailure {
    Panic { message: String },
    ReturnNone,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeCastResult {
    Target { ty: TypeId },
    Option { option_ty: TypeId, some_ty: TypeId },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeCastMetadata {
    pub test: RuntimeTypeTestMetadata,
    pub failure: RuntimeCastFailure,
    pub result: RuntimeCastResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimePatternTypeTestKind {
    StaticValue,
    RuntimeRef,
    RuntimeClass,
    RuntimeInterface,
    RuntimeNominal,
    RuntimeParameterized,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimePatternTypeTestMetadata {
    pub subject_ty: TypeId,
    pub target_ty: TypeId,
    pub descriptor: RuntimeTypeDescriptorKey,
    pub match_kind: RuntimePatternTypeTestKind,
    pub static_fold: RuntimeTypeStaticFold,
    pub parameterized: RuntimeTypeParameterizedMatch,
}

/// `when` pattern 在 MIR 上的 backend-agnostic 表示。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    Else,
    Or {
        pats: Vec<Pattern>,
    },
    Wildcard,
    Rest,
    Is {
        ty: TypeId,
        metadata: RuntimePatternTypeTestMetadata,
    },
    Bind {
        name: String,
        ty: TypeId,
    },
    Tuple {
        elements: Vec<Pattern>,
    },
    Variant {
        name: String,
        args: Vec<Pattern>,
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

/// 从一个已匹配 subject 中提取 binder 值时使用的投影路径。
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum PatternBindingStep {
    TupleIndex(usize),
    VariantField { variant: String, field_index: usize },
}

/// member write 中“值内 continuation payload 路径”的最小 published contract。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredContinuationValueRoute {
    pub source_local: LocalId,
    pub source_ty: TypeId,
    pub path: Vec<PatternBindingStep>,
}

/// member write 对 continuation payload path 的 published 结论。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StoredContinuationRoutePublication {
    None,
    Unique(StoredContinuationValueRoute),
    Ambiguous,
}

/// 右值（最小 rvalue 模型）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub enum Rvalue {
    Use(Operand),
    /// Explicit value transport/coercion boundary published by MIR lowering.
    ///
    /// The metadata owns source layout, target erasure reason, and copy/drop/trace obligations so
    /// later codegen does not infer boxing from source/target type mismatch.
    Transport {
        value: Operand,
        transport: ValueTransportMetadata,
    },
    TopLevelRef(TopLevelRef),
    UnresolvedName {
        name: String,
    },
    TypeCheck {
        value: Operand,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
        metadata: RuntimeTypeTestMetadata,
    },
    Cast {
        value: Operand,
        op: ast::CastOp,
        target_ty: TypeId,
        metadata: RuntimeCastMetadata,
    },
    MemberAccess {
        site_id: Option<SiteId>,
        receiver: Operand,
        member: MemberAccessMetadata,
    },
    /// Enum/Option variant constructor after typed HIR has supplied the expected enum type.
    EnumVariant {
        enum_ty: TypeId,
        variant_name: String,
        args: Vec<CallArg>,
        payload: AggregateTransportMetadata,
    },
    /// Class constructor call after typed HIR has identified the nominal result class.
    ClassCtor {
        site_id: SiteId,
        class_fqn: String,
        ctor: ClassCtorCallMetadata,
        args: Vec<CallArg>,
        hidden_effects: EffectRow,
    },
    /// 一个显式普通调用节点。
    ///
    /// 当前阶段承载 direct / closure / fun-value / virtual / interface / resume 六类调用；
    /// 更晚若补更多调用/控制转移语义，也应继续复用同一调用层级，而不是再造平行表示。
    Call {
        site_id: SiteId,
        kind: CallKind,
        args: Vec<CallArg>,
        transport: CallTransportMetadata,
    },
    /// 创建一个 tuple 值（最小 aggregate，用于 env struct 等场景）。
    MakeTuple {
        elements: Vec<Operand>,
        transport: AggregateTransportMetadata,
    },
    /// 创建一个 struct 值。字段值已按源码求值顺序先 lower 为 operand。
    StructLit {
        fields: Vec<StructLitField>,
        transport: AggregateTransportMetadata,
    },
    /// 编译期 `sizeOf(value)` intrinsic；`value` 本身不求值，只消费静态类型。
    SizeOf {
        value_ty: TypeId,
    },
    /// 编译期 `kindOf<T>()` intrinsic；泛型实例 materialize 后按具体类型求值。
    KindOf {
        value_ty: TypeId,
    },
    /// 编译期 `alignOf<T>()` intrinsic；`value` 本身不求值，只消费静态类型。
    AlignOf {
        value_ty: TypeId,
    },
    /// 编译期 `descOf<T>()` intrinsic；非 composite 类型在 codegen 阶段 materialize 为 0。
    DescOf {
        value_ty: TypeId,
    },
    /// Runtime class literal / type metadata value primitive.
    TypeMetadataLiteral(TypeMetadataLiteral),
    /// 运行期插值字符串构造。表达式片段已按 ANF 先求值为 operand。
    InterpolatedString {
        raw: bool,
        parts: Vec<InterpolatedStringPart>,
    },
    /// 读取 tuple 的某个字段：`tuple[index]`（按捕获顺序索引）。
    TupleGet {
        tuple: Operand,
        index: usize,
    },
    /// 一个 `when` arm 的 pattern test（结果为 Bool）。
    PatternMatch {
        subject: Operand,
        pattern: Pattern,
    },
    /// 从一个已经匹配成功的 subject 中提取 pattern binder 值。
    PatternExtract {
        subject: Operand,
        path: Vec<PatternBindingStep>,
    },
    /// 创建一个函数值（closure）：`{ env_struct, fn_ptr }`（T0710/T0711）。
    ///
    /// 当前阶段：`env` 为 `Unit`（无捕获）或按 MIR capture schema 排列的 tuple env；
    /// LLVM lowering 必须消费 `env_contract` 与 composite transport metadata，不得从 tuple 形状猜捕获语义。
    MakeClosure {
        env: Operand,
        fn_ptr: String,
        env_contract: ClosureEnvTransportMetadata,
    },
    /// `perform` 被 handler/resume 继续执行后，原表达式位置接收到的结果值 provenance。
    PerformResult {
        op_fqn: String,
        effect_ty: TypeId,
    },
    Todo(String),
}

/// MIR terminator（显式控制流）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
    /// 当该 terminator 发生 unwinding（例如 effect 传播）时应采取的动作。
    pub unwind: UnwindAction,
}

/// terminator 在发生 unwinding 时应采取的动作（最小模型，用于 `finally`/effect unwinding）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub enum UnwindAction {
    /// 该 terminator 不会发生 unwinding。
    NoUnwind,
    /// 若发生 unwinding，则直接继续向外传播；当前 body 内无需额外 cleanup。
    Propagate,
    /// 若发生 unwinding，则先跳转到 cleanup block 执行清理逻辑。
    Cleanup { target: BasicBlockId },
    /// 未实现占位：表示“可能会 unwind，但具体行为尚未建模”。
    Todo(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(deserialize = ""))]
pub enum TerminatorKind {
    /// 从当前 callable 正常返回；`value=None` 表示 `Unit`/隐式返回。
    Return {
        value: Option<Operand>,
    },
    /// cleanup block：执行完清理逻辑后继续向上传播 unwinding。
    ResumeUnwind,
    Goto {
        target: BasicBlockId,
    },
    /// 条件分支：若 `cond` 为真跳转到 `then_target`，否则跳转到 `else_target`。
    CondBr {
        cond: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Unreachable,
    /// effect operation 调用（对应 HIR 的 `ExprKind::Perform`）。
    ///
    /// 当前阶段仅保留“发生了哪一个 effect op”的信息；具体如何进入 handler/如何建模 unwinding
    /// 由后续 effect lowering 任务（TODO T0713/T0707）决定。
    Perform {
        site_id: SiteId,
        op_fqn: String,
        metadata: PerformMetadata,
        args: Vec<PerformArg>,
        /// 被 handler/continuation 恢复后，普通计算继续所在的 direct-style CFG block。
        resume_target: BasicBlockId,
    },
    /// effect handler 区域（对应 HIR 的 `ExprKind::Handle`）。
    ///
    /// 注意：该变体目前仍是“结构占位”，但会携带保守 CFG target，确保 MIR reachability
    /// 能看见 handler body / arms / finally 中保形保留下来的调用点。更晚的 effect lowering
    /// 仍会把 handle 展开为完整的 cleanup/handler 栈管理。
    Handle {
        site_id: SiteId,
        metadata: HandleMetadata,
        arms: Vec<HandlerArm>,
        has_finally: bool,
        body_target: BasicBlockId,
        arm_targets: Vec<BasicBlockId>,
        finally_target: Option<BasicBlockId>,
        /// handle 表达式正常完成（经 body/arm/finally 收束）后，外层求值继续所在的 block。
        exit_target: BasicBlockId,
    },
    /// 未实现控制流占位（例如 if/switch/call/cleanup 等）。
    Todo(String),
}

/// `handle` 在 MIR 视图下的一个 handler arm（结构占位）。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct HandlerArm {
    pub op_fqn: String,
    pub op_type_args: Vec<TypeId>,
    /// arm payload binder 数量（与 `binder_locals.len()` 保持一致）。
    pub binder_count: usize,
    /// arm payload binder 在当前 body 中的隐式输入 local。
    ///
    /// 这些 local 没有单独的赋值语句；它们由 `TerminatorKind::Handle` 进入对应 `arm_target` 时
    /// 作为 block input 被带入，供 arm body 直接引用。
    pub binder_locals: Vec<LocalId>,
    /// 逃逸 continuation arm 的显式 continuation binder local（若存在）。
    pub continuation_local: Option<LocalId>,
    pub handled_effect_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub body_ty: TypeId,
    pub kind: HandlerArmKind,
}

impl fmt::Debug for HandlerArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("HandlerArm");
        s.field("op_fqn", &self.op_fqn);
        if !self.op_type_args.is_empty() {
            s.field("op_type_args", &self.op_type_args);
        }
        s.field("binder_count", &self.binder_count);
        s.field("binder_locals", &self.binder_locals);
        s.field("continuation_local", &self.continuation_local);
        s.field("handled_effect_ty", &self.handled_effect_ty);
        s.field("payload_tuple_ty", &self.payload_tuple_ty);
        s.field("payload_component_tys", &self.payload_component_tys);
        s.field("body_ty", &self.body_ty);
        s.field("kind", &self.kind);
        s.finish()
    }
}

impl TerminatorKind {
    pub fn site_id(&self) -> Option<SiteId> {
        match self {
            TerminatorKind::Perform { site_id, .. } | TerminatorKind::Handle { site_id, .. } => {
                Some(*site_id)
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => None,
        }
    }

    /// 对 terminator 的“正常”后继基本块调用回调（不包含 cleanup/unwind 边）。
    ///
    /// 该接口适合做 CFG 分析（reachable/循环检测等），避免为每次查询分配 `Vec`。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        match self {
            TerminatorKind::Perform { resume_target, .. } => f(*resume_target),
            TerminatorKind::Goto { target } => f(*target),
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                f(*then_target);
                f(*else_target);
            }
            TerminatorKind::Handle {
                body_target,
                arm_targets,
                finally_target,
                ..
            } => {
                f(*body_target);
                for target in arm_targets {
                    f(*target);
                }
                if let Some(target) = finally_target {
                    f(*target);
                }
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
    }
}

impl Rvalue {
    pub fn site_id(&self) -> Option<SiteId> {
        match self {
            Rvalue::Call { site_id, .. } | Rvalue::ClassCtor { site_id, .. } => Some(*site_id),
            Rvalue::TopLevelRef(top_level) => top_level.site_id,
            Rvalue::Use(_)
            | Rvalue::Transport { .. }
            | Rvalue::UnresolvedName { .. }
            | Rvalue::TypeCheck { .. }
            | Rvalue::Cast { .. }
            | Rvalue::MemberAccess { .. }
            | Rvalue::EnumVariant { .. }
            | Rvalue::MakeTuple { .. }
            | Rvalue::StructLit { .. }
            | Rvalue::SizeOf { .. }
            | Rvalue::KindOf { .. }
            | Rvalue::AlignOf { .. }
            | Rvalue::DescOf { .. }
            | Rvalue::TypeMetadataLiteral(_)
            | Rvalue::InterpolatedString { .. }
            | Rvalue::TupleGet { .. }
            | Rvalue::PatternMatch { .. }
            | Rvalue::PatternExtract { .. }
            | Rvalue::MakeClosure { .. }
            | Rvalue::PerformResult { .. }
            | Rvalue::Todo(_) => None,
        }
    }
}

impl Terminator {
    /// 对 terminator 的后继基本块调用回调（包含 cleanup/unwind 边）。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        self.kind.for_each_successor(&mut f);
        if let UnwindAction::Cleanup { target } = &self.unwind {
            f(*target);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MirPlaceholderCategory {
    Item,
    Statement,
    Rvalue,
    Terminator,
    UnwindAction,
}

impl fmt::Display for MirPlaceholderCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirPlaceholderCategory::Item => write!(f, "item"),
            MirPlaceholderCategory::Statement => write!(f, "statement"),
            MirPlaceholderCategory::Rvalue => write!(f, "rvalue"),
            MirPlaceholderCategory::Terminator => write!(f, "terminator"),
            MirPlaceholderCategory::UnwindAction => write!(f, "unwind action"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MirSiteMetadataKind {
    Call,
    Resume,
    Perform,
    Handle,
}

impl fmt::Display for MirSiteMetadataKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirSiteMetadataKind::Call => write!(f, "call"),
            MirSiteMetadataKind::Resume => write!(f, "resume"),
            MirSiteMetadataKind::Perform => write!(f, "perform"),
            MirSiteMetadataKind::Handle => write!(f, "handle"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, serde::Serialize, serde::Deserialize)]
pub enum MirValidationError {
    /// MIR body 为空（没有任何基本块）。
    #[error("MIR body is empty")]
    EmptyBody,
    /// `start` 超出 `blocks` 范围。
    #[error("invalid start block {start:?} for {blocks_len} blocks")]
    InvalidStartBlock {
        start: BasicBlockId,
        blocks_len: usize,
    },
    /// terminator 的 target 超出 `blocks` 范围。
    #[error("invalid target {target:?} from {from:?} for {blocks_len} blocks")]
    InvalidTarget {
        from: BasicBlockId,
        target: BasicBlockId,
        blocks_len: usize,
    },
    #[error("duplicate site id {site_id:?} in {first_block:?} and {second_block:?}")]
    DuplicateSiteId {
        site_id: SiteId,
        first_block: BasicBlockId,
        second_block: BasicBlockId,
    },
    #[error("cleanup target {target:?} from {from:?} is not marked cleanup")]
    CleanupTargetNotMarked {
        from: BasicBlockId,
        target: BasicBlockId,
    },
    #[error("handle at {from:?} has {arms_len} arms but {targets_len} arm targets")]
    InvalidHandleArmTargetCount {
        from: BasicBlockId,
        arms_len: usize,
        targets_len: usize,
    },
    #[error(
        "handle at {from:?} has_finally={has_finally} but finally target is {finally_target:?}"
    )]
    InvalidHandleFinallyTarget {
        from: BasicBlockId,
        has_finally: bool,
        finally_target: Option<BasicBlockId>,
    },
    #[error("handle finally target {target:?} from {from:?} is not marked cleanup")]
    HandleFinallyTargetNotCleanup {
        from: BasicBlockId,
        target: BasicBlockId,
    },
    #[error("handle exit target {target:?} from {from:?} is out of range for {blocks_len} blocks")]
    InvalidHandleExitTarget {
        from: BasicBlockId,
        target: BasicBlockId,
        blocks_len: usize,
    },
    #[error("MIR still contains forbidden {category} todo `{reason}` in {block:?} at {span:?}")]
    Todo {
        block: BasicBlockId,
        span: Span,
        category: MirPlaceholderCategory,
        reason: String,
    },
    #[error("production MIR `{fqn}` contains {category} todo `{reason}` in {block:?} at {span:?}")]
    ProductionTodo {
        fqn: String,
        block: Option<BasicBlockId>,
        span: Span,
        category: MirPlaceholderCategory,
        reason: String,
    },
    #[error("production MIR `{fqn}` failed direct-style contract: {error}")]
    ProductionBodyContract {
        fqn: String,
        #[source]
        error: Box<MirValidationError>,
    },
    #[error(
        "production MIR `{fqn}` has non-Unit return type {return_ty:?} but returns no value in {block:?} at {span:?}"
    )]
    ProductionMissingReturnValue {
        fqn: String,
        block: BasicBlockId,
        span: Span,
        return_ty: TypeId,
    },
    #[error(
        "production MIR `{fqn}` has incomplete {site} site metadata in {block:?} at {span:?}: {detail}"
    )]
    ProductionSiteMetadata {
        fqn: String,
        block: BasicBlockId,
        span: Span,
        site: MirSiteMetadataKind,
        detail: &'static str,
    },
    #[error(
        "production MIR `{fqn}` has incomplete {primitive} runtime value metadata in {block:?} at {span:?}: {detail}"
    )]
    ProductionRuntimeValueMetadata {
        fqn: String,
        block: BasicBlockId,
        span: Span,
        primitive: &'static str,
        detail: &'static str,
    },
    #[error(
        "production MIR `{fqn}` has incomplete {transport} transport metadata in {block:?} at {span:?}: {detail}"
    )]
    ProductionTransportMetadata {
        fqn: String,
        block: BasicBlockId,
        span: Span,
        transport: &'static str,
        detail: &'static str,
    },
    #[error("MIR `{fqn}` has invalid {surface} contract in {block:?} at {span:?}: {detail}")]
    TypeContract {
        fqn: String,
        block: Option<BasicBlockId>,
        span: Span,
        surface: &'static str,
        detail: &'static str,
    },
}

impl MirValidationError {
    pub fn body_fqn(&self) -> Option<&str> {
        match self {
            MirValidationError::ProductionTodo { fqn, .. }
            | MirValidationError::ProductionBodyContract { fqn, .. }
            | MirValidationError::ProductionMissingReturnValue { fqn, .. }
            | MirValidationError::ProductionSiteMetadata { fqn, .. }
            | MirValidationError::ProductionRuntimeValueMetadata { fqn, .. }
            | MirValidationError::ProductionTransportMetadata { fqn, .. }
            | MirValidationError::TypeContract { fqn, .. } => Some(fqn),
            MirValidationError::EmptyBody
            | MirValidationError::InvalidStartBlock { .. }
            | MirValidationError::InvalidTarget { .. }
            | MirValidationError::DuplicateSiteId { .. }
            | MirValidationError::CleanupTargetNotMarked { .. }
            | MirValidationError::InvalidHandleArmTargetCount { .. }
            | MirValidationError::InvalidHandleFinallyTarget { .. }
            | MirValidationError::HandleFinallyTargetNotCleanup { .. }
            | MirValidationError::InvalidHandleExitTarget { .. }
            | MirValidationError::Todo { .. } => None,
        }
    }
}

fn is_forbidden_effect_todo(reason: &str) -> bool {
    matches!(
        reason,
        "unterminated"
            | "handle result pending"
            | "handle body exit pending"
            | "handle arm exit pending"
            | "handle finally exit pending"
            | "perform unwind pending"
            | "break not in loop"
            | "continue not in loop"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::{NominalType, RefTypeKind, TypeKind, TypeStore};

    const TEST_FQN: &str = "sample.main";
    const SYNTHETIC_ITEM_TODO_REASON: &str = "synthetic item todo";
    const SYNTHETIC_STATEMENT_TODO_REASON: &str = "synthetic statement todo";

    fn test_span() -> Span {
        Span::new(10, 20)
    }

    fn test_call_transport(result_ty: TypeId) -> CallTransportMetadata {
        CallTransportMetadata::plain_no_outward(result_ty, MirTransportKind::Unknown)
    }

    fn production_file(return_ty: TypeId, body: Body) -> File {
        File {
            items: vec![Item::Fun(FunDecl {
                span: test_span(),
                fqn: TEST_FQN.to_string(),
                name: "main".to_string(),
                ty: return_ty,
                params: Vec::new(),
                return_ty,
                body: Some(body),
            })],
        }
    }

    fn single_block_body(
        stmts: Vec<Statement>,
        kind: TerminatorKind,
        unwind: UnwindAction,
    ) -> Body {
        let mut body = Body::new_empty();
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts,
            terminator: Terminator {
                span: test_span(),
                kind,
                unwind,
            },
        });
        body.start = bb;
        body
    }

    fn body_with_assign(value: Rvalue, local_ty: TypeId) -> Body {
        let mut body = Body::new_empty();
        let local = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("tmp0".to_string()),
            ty: local_ty,
            source: LocalSourceKind::CompilerTemporary,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: test_span(),
                kind: StatementKind::Assign {
                    target: local,
                    value,
                },
            }],
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        body
    }

    fn class_ctor_rvalue(class_fqn: &str) -> Rvalue {
        Rvalue::ClassCtor {
            site_id: SiteId::from_raw(0),
            class_fqn: class_fqn.to_string(),
            ctor: ClassCtorCallMetadata {
                selected_ctor_span: None,
                ordered_param_count: 0,
            },
            args: Vec::new(),
            hidden_effects: EffectRow::pure(),
        }
    }

    fn body_with_source_assign(
        source_ty: TypeId,
        target_ty: TypeId,
        value: impl FnOnce(LocalId) -> Rvalue,
    ) -> Body {
        let mut body = Body::new_empty();
        let source = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("source".to_string()),
            ty: source_ty,
            source: LocalSourceKind::SourceLocal,
        });
        let target = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("target".to_string()),
            ty: target_ty,
            source: LocalSourceKind::CompilerTemporary,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: test_span(),
                kind: StatementKind::Assign {
                    target,
                    value: value(source),
                },
            }],
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        body
    }

    #[test]
    fn mir_no_todo_rejects_item_todo() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = File {
            items: vec![Item::Todo {
                span: test_span(),
                kind: SYNTHETIC_ITEM_TODO_REASON.to_string(),
            }],
        };

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTodo {
                fqn: "<file>".to_string(),
                block: None,
                span: test_span(),
                category: MirPlaceholderCategory::Item,
                reason: SYNTHETIC_ITEM_TODO_REASON.to_string(),
            })
        );
    }

    #[test]
    fn mir_no_todo_rejects_statement_todo() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let stmt = Statement {
            span: test_span(),
            kind: StatementKind::Todo(SYNTHETIC_STATEMENT_TODO_REASON.to_string()),
        };
        let file = production_file(
            builtins.unit,
            single_block_body(
                vec![stmt],
                TerminatorKind::Return { value: None },
                UnwindAction::NoUnwind,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTodo {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                category: MirPlaceholderCategory::Statement,
                reason: SYNTHETIC_STATEMENT_TODO_REASON.to_string(),
            })
        );
    }

    #[test]
    fn mir_no_todo_rejects_rvalue_todo() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_assign(Rvalue::Todo("missing expr".to_string()), builtins.unit),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTodo {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                category: MirPlaceholderCategory::Rvalue,
                reason: "missing expr".to_string(),
            })
        );
    }

    #[test]
    fn mir_type_contract_rejects_assignment_target_outside_local_table() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let stmt = Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target: LocalId::from_raw(0),
                value: Rvalue::Use(Operand::Const(ConstValue::Unit)),
            },
        };
        let file = production_file(
            builtins.unit,
            single_block_body(
                vec![stmt],
                TerminatorKind::Return { value: None },
                UnwindAction::NoUnwind,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                surface: "assignment target",
                detail: "local reference is outside the body local table",
            })
        );
    }

    #[test]
    fn mir_type_contract_rejects_param_local_type_drift() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = single_block_body(
            Vec::new(),
            TerminatorKind::Return { value: None },
            UnwindAction::NoUnwind,
        );
        let local = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("x".to_string()),
            ty: builtins.int,
            source: LocalSourceKind::SourceLocal,
        });
        let file = File {
            items: vec![Item::Fun(FunDecl {
                span: test_span(),
                fqn: TEST_FQN.to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: vec![Param {
                    span: test_span(),
                    name: "x".to_string(),
                    ty: builtins.bool_,
                    local,
                }],
                return_ty: builtins.unit,
                body: Some(body),
            })],
        };

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: None,
                span: test_span(),
                surface: "parameter type",
                detail: "parameter type and parameter local type disagree",
            })
        );
    }

    #[test]
    fn mir_type_contract_rejects_return_value_outside_local_table() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(LocalId::from_raw(0))),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        let file = production_file(builtins.unit, body);

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                surface: "return value",
                detail: "local reference is outside the body local table",
            })
        );
    }

    #[test]
    fn mir_cfg_contract_allows_direct_bool_type_id_drift() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let cond = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("cond".to_string()),
            ty: builtins.int,
            source: LocalSourceKind::SourceLocal,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::CondBr {
                    cond: Operand::Local(cond),
                    then_target: BasicBlockId(0),
                    else_target: BasicBlockId(0),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        let file = production_file(builtins.unit, body);

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Ok(())
        );
    }

    #[test]
    fn mir_cfg_contract_rejects_residual_interpolated_string_rvalue() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_assign(
                Rvalue::InterpolatedString {
                    raw: false,
                    parts: Vec::new(),
                },
                builtins.string,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                surface: "interpolated string",
                detail: "interpolated strings must be desugared before MIR codegen",
            })
        );
    }

    #[test]
    fn mir_class_ctor_contract_rejects_non_nominal_result_type() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_assign(class_ctor_rvalue("sample.Box"), builtins.int),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                surface: "class constructor result",
                detail: "class constructor result target must have class reference type",
            })
        );
    }

    #[test]
    fn mir_class_ctor_contract_rejects_result_nominal_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let other_class_ty = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "sample.Other".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let file = production_file(
            builtins.unit,
            body_with_assign(class_ctor_rvalue("sample.Box"), other_class_ty),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::TypeContract {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                surface: "class constructor result",
                detail: "class constructor result target and class metadata disagree",
            })
        );
    }

    #[test]
    fn mir_no_todo_rejects_terminator_todo() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            single_block_body(
                Vec::new(),
                TerminatorKind::Todo("unterminated".to_string()),
                UnwindAction::NoUnwind,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTodo {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                category: MirPlaceholderCategory::Terminator,
                reason: "unterminated".to_string(),
            })
        );
    }

    #[test]
    fn mir_no_todo_direct_style_rejects_unterminated_sentinel() {
        let body = single_block_body(
            Vec::new(),
            TerminatorKind::Todo("unterminated".to_string()),
            UnwindAction::NoUnwind,
        );

        assert_eq!(
            body.validate_direct_style(),
            Err(MirValidationError::Todo {
                block: BasicBlockId(0),
                span: test_span(),
                category: MirPlaceholderCategory::Terminator,
                reason: "unterminated".to_string(),
            })
        );
    }

    #[test]
    fn mir_no_todo_rejects_unwind_todo() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            single_block_body(
                Vec::new(),
                TerminatorKind::Return { value: None },
                UnwindAction::Todo("perform unwind pending".to_string()),
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTodo {
                fqn: TEST_FQN.to_string(),
                block: Some(BasicBlockId(0)),
                span: test_span(),
                category: MirPlaceholderCategory::UnwindAction,
                reason: "perform unwind pending".to_string(),
            })
        );
    }

    #[test]
    fn mir_no_todo_rejects_non_unit_empty_return() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.int,
            single_block_body(
                Vec::new(),
                TerminatorKind::Return { value: None },
                UnwindAction::NoUnwind,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionMissingReturnValue {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                return_ty: builtins.int,
            })
        );
    }

    #[test]
    fn mir_no_todo_requires_resume_runtime_error_metadata() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_assign(
                Rvalue::Call {
                    site_id: SiteId::from_raw(0),
                    kind: CallKind::Resume {
                        continuation: Operand::Const(ConstValue::Unit),
                        resume: ResumeMetadata {
                            continuation_ty: builtins.unit,
                            resume_ty: builtins.unit,
                            answer_ty: builtins.unit,
                            return_ty: builtins.unit,
                            out_effects: EffectRow::pure(),
                            runtime_error_effect_ty: None,
                            suspends_outward: false,
                        },
                    },
                    args: vec![CallArg {
                        span: test_span(),
                        name: None,
                        value: Operand::Const(ConstValue::Unit),
                    }],
                    transport: test_call_transport(builtins.unit),
                },
                builtins.unit,
            ),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionSiteMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                site: MirSiteMetadataKind::Resume,
                detail: "resume call is missing runtime-error effect metadata",
            })
        );
    }

    #[test]
    fn mir_value_metadata_rejects_typecheck_source_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_source_assign(builtins.int, builtins.bool_, |source| Rvalue::TypeCheck {
                value: Operand::Local(source),
                op: ast::TypeCheckOp::Is,
                test_ty: builtins.string,
                metadata: RuntimeTypeTestMetadata {
                    source_ty: builtins.unit,
                    target_ty: builtins.string,
                    descriptor: RuntimeTypeDescriptorKey {
                        ty: builtins.string,
                        kind: RuntimeTypeDescriptorKind::String,
                    },
                    static_fold: RuntimeTypeStaticFold::Dynamic,
                    parameterized: RuntimeTypeParameterizedMatch::None,
                },
            }),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                primitive: "typecheck",
                detail: "source type and operand type disagree",
            })
        );
    }

    #[test]
    fn mir_value_metadata_rejects_asq_result_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let option_int = types.ty_option(builtins.int);
        let file = production_file(
            builtins.unit,
            body_with_source_assign(builtins.any, builtins.bool_, |source| Rvalue::Cast {
                value: Operand::Local(source),
                op: ast::CastOp::AsQ,
                target_ty: builtins.int,
                metadata: RuntimeCastMetadata {
                    test: RuntimeTypeTestMetadata {
                        source_ty: builtins.any,
                        target_ty: builtins.int,
                        descriptor: RuntimeTypeDescriptorKey {
                            ty: builtins.int,
                            kind: RuntimeTypeDescriptorKind::Value,
                        },
                        static_fold: RuntimeTypeStaticFold::Dynamic,
                        parameterized: RuntimeTypeParameterizedMatch::None,
                    },
                    failure: RuntimeCastFailure::ReturnNone,
                    result: RuntimeCastResult::Option {
                        option_ty: option_int,
                        some_ty: builtins.int,
                    },
                },
            }),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                primitive: "cast",
                detail: "optional result type and assignment target disagree",
            })
        );
    }

    #[test]
    fn mir_value_metadata_rejects_pattern_subject_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = production_file(
            builtins.unit,
            body_with_source_assign(builtins.any, builtins.bool_, |source| {
                Rvalue::PatternMatch {
                    subject: Operand::Local(source),
                    pattern: Pattern::Is {
                        ty: builtins.string,
                        metadata: RuntimePatternTypeTestMetadata {
                            subject_ty: builtins.int,
                            target_ty: builtins.string,
                            descriptor: RuntimeTypeDescriptorKey {
                                ty: builtins.string,
                                kind: RuntimeTypeDescriptorKind::String,
                            },
                            match_kind: RuntimePatternTypeTestKind::RuntimeRef,
                            static_fold: RuntimeTypeStaticFold::Dynamic,
                            parameterized: RuntimeTypeParameterizedMatch::None,
                        },
                    },
                }
            }),
        );

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionRuntimeValueMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                primitive: "pattern type test",
                detail: "subject type and operand type disagree",
            })
        );
    }

    #[test]
    fn mir_aggregate_transport_rejects_ambiguous_continuation_route() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let receiver = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("receiver".to_string()),
            ty: builtins.any,
            source: LocalSourceKind::SourceLocal,
        });
        let value = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("value".to_string()),
            ty: builtins.any,
            source: LocalSourceKind::SourceLocal,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: test_span(),
                kind: StatementKind::StoreMember {
                    receiver: Operand::Local(receiver),
                    member: MemberAccessMetadata {
                        name: "next".to_string(),
                        receiver_ty: builtins.any,
                        resolved: None,
                        hidden_effects: EffectRow::pure(),
                    },
                    value: Operand::Local(value),
                    value_ty: builtins.any,
                    continuation_route: StoredContinuationRoutePublication::Ambiguous,
                },
            }],
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        let file = production_file(builtins.unit, body);

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTransportMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                transport: "member store continuation route",
                detail: "ambiguous continuation route must be split or rejected before handoff",
            })
        );
    }

    #[test]
    fn mir_aggregate_transport_rejects_field_type_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let tuple_ty = types.ty_tuple(vec![builtins.int]);
        let mut body = Body::new_empty();
        let source = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("source".to_string()),
            ty: builtins.int,
            source: LocalSourceKind::SourceLocal,
        });
        let target = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("target".to_string()),
            ty: tuple_ty,
            source: LocalSourceKind::CompilerTemporary,
        });
        let bb = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: test_span(),
                kind: StatementKind::Assign {
                    target,
                    value: Rvalue::MakeTuple {
                        elements: vec![Operand::Local(source)],
                        transport: AggregateTransportMetadata {
                            aggregate_ty: tuple_ty,
                            kind: AggregateTransportKind::Tuple,
                            fields: vec![AggregateTransportField {
                                index: 0,
                                name: None,
                                ty: builtins.string,
                                transport: ValueTransportMetadata::plain(
                                    builtins.string,
                                    MirTransportKind::Reference,
                                ),
                            }],
                        },
                    },
                },
            }],
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb;
        let file = production_file(builtins.unit, body);

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionTransportMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                transport: "tuple aggregate",
                detail: "aggregate transport field type does not match lowered value",
            })
        );
    }

    #[test]
    fn mir_aggregate_transport_rejects_perform_payload_transport_mismatch() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let source = body.push_local(LocalDecl {
            span: test_span(),
            name: Some("payload".to_string()),
            ty: builtins.int,
            source: LocalSourceKind::SourceLocal,
        });
        let resume = BasicBlockId(1);
        let start = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "sample.E.op".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.any,
                        op_type_args: Vec::new(),
                        result_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: vec![builtins.int],
                        payload_transport: vec![ValueTransportMetadata::plain(
                            builtins.string,
                            MirTransportKind::Reference,
                        )],
                        arg_mapping: vec![0],
                    },
                    args: vec![PerformArg {
                        span: test_span(),
                        source_arg_index: 0,
                        name: None,
                        value: Operand::Local(source),
                    }],
                    resume_target: resume,
                },
                unwind: UnwindAction::Propagate,
            },
        });
        let resume_block = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: test_span(),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        assert_eq!(resume_block, resume);
        body.start = start;
        let file = production_file(builtins.unit, body);

        assert_eq!(
            file.validate_production(&types, builtins.unit, builtins.bool_),
            Err(MirValidationError::ProductionSiteMetadata {
                fqn: TEST_FQN.to_string(),
                block: BasicBlockId(0),
                span: test_span(),
                site: MirSiteMetadataKind::Perform,
                detail: "perform payload transport type disagrees with payload component type",
            })
        );
    }

    #[test]
    fn cfg_reachable_two_blocks_ok() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();

        let mut body = Body::new_empty();
        let _tmp = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Nop,
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(1),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });

        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
    }

    #[test]
    fn cfg_invalid_target_is_error() {
        let mut body = Body::new_empty();
        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(42),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_cfg(),
            Err(MirValidationError::InvalidTarget {
                from: BasicBlockId(0),
                target: BasicBlockId(42),
                blocks_len: 1,
            })
        );
    }

    #[test]
    fn cfg_cleanup_edge_is_reachable() {
        // 模拟一个“可能 unwind 的 terminator”：
        // - 正常路径不存在（Perform 目前作为占位 terminator）
        // - unwind 路径跳到 cleanup block，然后用 ResumeUnwind 继续传播
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        op_type_args: Vec::new(),
                        result_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Cleanup {
                    target: BasicBlockId(2),
                },
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let bb2 = body.push_block(BasicBlock {
            is_cleanup: true,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::ResumeUnwind,
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert_eq!(bb2, BasicBlockId(2));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1, bb2]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
        assert!(body.blocks[bb2.as_usize()].is_cleanup);
    }

    #[test]
    fn mir_cfg_rejects_cleanup_target_without_cleanup_flag() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let result_local = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: result_local,
                    value: Rvalue::PerformResult {
                        op_fqn: "scoop.core.Raise.raise".to_string(),
                        effect_ty: builtins.unit,
                    },
                },
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        op_type_args: Vec::new(),
                        result_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Cleanup {
                    target: BasicBlockId(2),
                },
            },
        });
        let _bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let _bb2 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::ResumeUnwind,
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_direct_style(),
            Err(MirValidationError::CleanupTargetNotMarked {
                from: BasicBlockId(0),
                target: BasicBlockId(2),
            })
        );
    }

    #[test]
    fn mir_site_id_rejects_duplicate_call_and_terminator_site_ids() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let result_local = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: result_local,
                    value: Rvalue::Call {
                        site_id: SiteId::from_raw(0),
                        kind: CallKind::Direct {
                            callee_fqn: "sample.helper".to_string(),
                        },
                        args: Vec::new(),
                        transport: test_call_transport(builtins.unit),
                    },
                },
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        op_type_args: Vec::new(),
                        result_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Propagate,
            },
        });
        let _bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_direct_style(),
            Err(MirValidationError::DuplicateSiteId {
                site_id: SiteId::from_raw(0),
                first_block: BasicBlockId(0),
                second_block: BasicBlockId(0),
            })
        );
    }

    #[test]
    fn dump_mir_keeps_generic_functions_as_templates_before_monomorphization() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/generic_template_boundary.scoop",
            r#"
package fixtures.mir

fun id<T>(x: T): T {
    return x
}

fun use<T>(x: T): T {
    return id(x)
}

fun entry(): Int {
    return use(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns: Vec<&str> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect();

        assert!(fun_fqns.contains(&"fixtures.mir.id"));
        assert!(fun_fqns.contains(&"fixtures.mir.use"));
        assert!(fun_fqns.contains(&"fixtures.mir.entry"));
        assert!(fun_fqns.iter().all(|fqn| !fqn.contains("::<")));

        let use_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mir.use" => Some(fun),
                _ => None,
            })
            .expect("expected generic use function in MIR dump");
        assert!(matches!(
            lowered.types.kind(use_fun.params[0].ty),
            TypeKind::Param(_)
        ));
        assert!(matches!(
            lowered.types.kind(use_fun.return_ty),
            TypeKind::Param(_)
        ));

        let body = use_fun
            .body
            .as_ref()
            .expect("generic use function should have body");
        let call_kind = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .expect("expected direct call in generic use function body");
        match call_kind {
            CallKind::Direct { callee_fqn } => {
                assert_eq!(callee_fqn, "fixtures.mir.id");
            }
            other => panic!("expected direct generic-template call, got {other:?}"),
        }
    }
}
