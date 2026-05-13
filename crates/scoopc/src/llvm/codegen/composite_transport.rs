//! Composite transport layout contract and backend verifier.
//!
//! CG-T04a keeps concrete boxing/enum/array/closure/thread payload lowering in later owner tasks,
//! but it establishes the shared physical layout descriptor that those tasks must consume.

use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, GlobalValue, PointerValue};

use crate::llvm::{BackendGateError, LlvmEmitError};
use crate::mir::{
    AggregateTransportMetadata, ArrayElementTransportMetadata, CallTransportMetadata,
    CaptureBoxTransportMetadata, ClosureEnvTransportMetadata, GcIntrinsicTransportMetadata,
    MirBoxingReason, MirTransportKind, Rvalue, StatementKind, TerminatorKind,
    ValueTransportMetadata,
};
use crate::span::Span;
use crate::stable_id::{CanonicalTextKey, PrivateSymbolMangler, canonical_list, canonical_record};
use crate::ty::{TypeId, TypeKind, TypeStore, ValueTypeKind, is_builtin_scalar_nominal_value_type};

use super::MainCodegen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositeTransportStorageKind {
    Inline,
    Boxed,
    Erased,
}

impl CompositeTransportStorageKind {
    fn as_u32(self) -> u32 {
        match self {
            Self::Inline => 0,
            Self::Boxed => 1,
            Self::Erased => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Boxed => "boxed",
            Self::Erased => "erased",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositeTransportLayoutDescriptor {
    source_ty: TypeId,
    source_name: String,
    stable_name_key: String,
    kind: MirTransportKind,
    storage_kind: CompositeTransportStorageKind,
    size_bytes: u64,
    align_bytes: u64,
    gc_slot_offsets: Vec<u64>,
    trace_hook: bool,
    copy_hook: bool,
    drop_hook: bool,
}

impl CompositeTransportLayoutDescriptor {
    fn validate(&self, metadata: &ValueTransportMetadata) -> Result<(), &'static str> {
        if self.size_bytes == 0 || self.align_bytes == 0 {
            return Err("composite transport layout descriptor has invalid size or alignment");
        }
        if metadata.requirements.trace && !self.trace_hook && self.gc_slot_offsets.is_empty() {
            return Err(
                "traceable composite transport layout descriptor is missing a GC slot map or trace hook",
            );
        }
        if !metadata.requirements.trace && !self.gc_slot_offsets.is_empty() {
            return Err(
                "composite transport layout descriptor has GC slots but MIR trace requirement is false",
            );
        }
        if metadata.requirements.copy && !self.copy_hook {
            return Err(
                "copyable composite transport layout descriptor is missing copy hook identity",
            );
        }
        if metadata.requirements.drop && !self.drop_hook {
            return Err(
                "droppable composite transport layout descriptor is missing drop hook identity",
            );
        }
        Ok(())
    }
}

fn composite_transport_kind_key(kind: MirTransportKind) -> &'static str {
    match kind {
        MirTransportKind::Scalar => "scalar",
        MirTransportKind::Reference => "reference",
        MirTransportKind::Tuple => "tuple",
        MirTransportKind::Struct => "struct",
        MirTransportKind::EnumPayload => "enum_payload",
        MirTransportKind::ClosureEnv => "closure_env",
        MirTransportKind::CaptureBox => "capture_box",
        MirTransportKind::ArrayElement => "array_element",
        MirTransportKind::EffectPayload => "effect_payload",
        MirTransportKind::FunctionValue => "function_value",
        MirTransportKind::Unknown => "unknown",
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn verify_mir_body_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        body_span: Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
    ) -> Result<(), LlvmEmitError> {
        for block in &body.blocks {
            for stmt in &block.stmts {
                if let StatementKind::Assign { value, .. } = &stmt.kind {
                    self.verify_rvalue_composite_transport_contract(
                        body_fqn, stmt.span, value, mir_types,
                    )?;
                }
            }

            if let TerminatorKind::Perform { metadata, .. } = &block.terminator.kind {
                for transport in &metadata.payload_transport {
                    self.verify_value_composite_transport_contract(
                        body_fqn,
                        block.terminator.span,
                        mir_types,
                        transport,
                    )?;
                }
            }
        }

        if body.blocks.is_empty() {
            return Err(composite_transport_gate_error(
                body_fqn,
                body_span,
                "PIPELINE_GAPS §4.1",
                "composite transport verifier reached an empty body before layout contract checks",
            ));
        }

        Ok(())
    }

    fn verify_rvalue_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        value: &Rvalue,
        mir_types: &TypeStore,
    ) -> Result<(), LlvmEmitError> {
        match value {
            Rvalue::EnumVariant { payload, .. }
            | Rvalue::MakeTuple {
                transport: payload, ..
            }
            | Rvalue::StructLit {
                transport: payload, ..
            } => self
                .verify_aggregate_composite_transport_contract(body_fqn, span, mir_types, payload),
            Rvalue::CaptureBoxNew { contract, .. }
            | Rvalue::CaptureBoxGet { contract, .. }
            | Rvalue::CaptureBoxSet { contract, .. } => self
                .verify_capture_box_composite_transport_contract(
                    body_fqn, span, mir_types, contract,
                ),
            Rvalue::MakeClosure { env_contract, .. } => self
                .verify_closure_env_composite_transport_contract(
                    body_fqn,
                    span,
                    mir_types,
                    env_contract,
                ),
            Rvalue::Call { transport, .. } => {
                self.verify_call_composite_transport_contract(body_fqn, span, mir_types, transport)
            }
            Rvalue::Transport { transport, .. } => {
                self.verify_value_composite_transport_contract(body_fqn, span, mir_types, transport)
            }
            Rvalue::Use(_)
            | Rvalue::TopLevelRef(_)
            | Rvalue::UnresolvedName { .. }
            | Rvalue::Unary { .. }
            | Rvalue::Binary { .. }
            | Rvalue::TypeCheck { .. }
            | Rvalue::Cast { .. }
            | Rvalue::MemberAccess { .. }
            | Rvalue::ClassCtor { .. }
            | Rvalue::SizeOf { .. }
            | Rvalue::TypeMetadataLiteral(_)
            | Rvalue::InterpolatedString { .. }
            | Rvalue::TupleGet { .. }
            | Rvalue::PatternMatch { .. }
            | Rvalue::PatternExtract { .. }
            | Rvalue::PerformResult { .. }
            | Rvalue::Todo(_) => Ok(()),
        }
    }

    fn verify_call_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        transport: &CallTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        self.verify_value_composite_transport_contract(
            body_fqn,
            span,
            mir_types,
            &transport.result,
        )?;
        if let Some(aggregate_return) = &transport.aggregate_return {
            self.verify_value_composite_transport_contract(
                body_fqn,
                span,
                mir_types,
                aggregate_return,
            )?;
        }
        if let Some(array) = &transport.array {
            self.verify_array_composite_transport_contract(body_fqn, span, mir_types, array)?;
        }
        if let Some(gc) = &transport.gc {
            self.verify_gc_intrinsic_composite_transport_contract(body_fqn, span, mir_types, gc)?;
        }
        if let Some(thread_resume_payload) = &transport.thread_resume_payload {
            self.verify_value_composite_transport_contract(
                body_fqn,
                span,
                mir_types,
                thread_resume_payload,
            )?;
        }
        Ok(())
    }

    fn verify_aggregate_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &AggregateTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        let aggregate = ValueTransportMetadata {
            source_ty: metadata.aggregate_ty,
            kind: match metadata.kind {
                crate::mir::AggregateTransportKind::Tuple => MirTransportKind::Tuple,
                crate::mir::AggregateTransportKind::Struct => MirTransportKind::Struct,
                crate::mir::AggregateTransportKind::EnumPayload => MirTransportKind::EnumPayload,
                crate::mir::AggregateTransportKind::ClosureEnv => MirTransportKind::ClosureEnv,
            },
            requirements: self
                .composite_transport_requirements_for_type(mir_types, metadata.aggregate_ty),
            boxing: None,
        };
        self.verify_value_composite_transport_contract(body_fqn, span, mir_types, &aggregate)?;
        for field in &metadata.fields {
            self.verify_value_composite_transport_contract(
                body_fqn,
                span,
                mir_types,
                &field.transport,
            )?;
        }
        Ok(())
    }

    fn verify_capture_box_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        contract: &CaptureBoxTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        self.verify_value_composite_transport_contract(body_fqn, span, mir_types, &contract.value)
    }

    fn verify_closure_env_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        contract: &ClosureEnvTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        let env_transport = ValueTransportMetadata {
            source_ty: contract.env_ty,
            kind: MirTransportKind::ClosureEnv,
            requirements: self
                .composite_transport_requirements_for_type(mir_types, contract.env_ty),
            boxing: None,
        };
        self.verify_value_composite_transport_contract(body_fqn, span, mir_types, &env_transport)?;
        for capture in &contract.captures {
            self.verify_value_composite_transport_contract(
                body_fqn,
                span,
                mir_types,
                &capture.transport,
            )?;
        }
        Ok(())
    }

    fn verify_array_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &ArrayElementTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        self.verify_value_composite_transport_contract(body_fqn, span, mir_types, &metadata.element)
    }

    fn verify_gc_intrinsic_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &GcIntrinsicTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        self.verify_value_composite_transport_contract(body_fqn, span, mir_types, &metadata.subject)
    }

    fn verify_value_composite_transport_contract(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &ValueTransportMetadata,
    ) -> Result<(), LlvmEmitError> {
        if !self.value_transport_needs_composite_layout(mir_types, metadata) {
            return Ok(());
        }
        self.get_or_create_value_composite_transport_descriptor_global(
            body_fqn, span, mir_types, metadata,
        )?;
        Ok(())
    }

    pub(super) fn get_or_create_value_composite_transport_descriptor_global(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &ValueTransportMetadata,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let mut metadata = metadata.clone();
        let layout_requirements =
            self.composite_transport_requirements_for_type(mir_types, metadata.source_ty);
        metadata.requirements.trace = layout_requirements.trace;
        metadata.requirements.copy |= layout_requirements.copy;
        metadata.requirements.drop |= layout_requirements.drop;
        let descriptor = self.composite_layout_descriptor_for_value_transport(
            body_fqn, span, mir_types, &metadata,
        )?;
        if let Err(detail) = descriptor.validate(&metadata) {
            return Err(composite_transport_gate_error(
                body_fqn,
                span,
                composite_transport_gap_id(&metadata),
                detail,
            ));
        }
        self.get_or_create_composite_transport_descriptor_global(&descriptor)
    }

    fn composite_layout_descriptor_for_value_transport(
        &mut self,
        body_fqn: &str,
        span: Span,
        mir_types: &TypeStore,
        metadata: &ValueTransportMetadata,
    ) -> Result<CompositeTransportLayoutDescriptor, LlvmEmitError> {
        let (source_ty, cg_ty) = if let Some(source_ty) =
            self.equivalent_codegen_type_id(mir_types, metadata.source_ty)
        {
            let cg_ty = self.cg_ty_of(source_ty).ok_or_else(|| {
                composite_transport_gate_error(
                    body_fqn,
                    span,
                    composite_transport_gap_id(metadata),
                    "composite transport use site is missing a codegen layout descriptor type",
                )
            })?;
            (source_ty, cg_ty)
        } else {
            let cg_ty = self.cg_ty_of_mir_type(mir_types, metadata.source_ty).ok_or_else(|| {
                let detail = if matches!(mir_types.kind(metadata.source_ty), TypeKind::Param(_)) {
                    "composite transport use site still references an unsubstituted type parameter"
                } else {
                    "composite transport use site is missing a materialized layout descriptor type"
                };
                composite_transport_gate_error(
                    body_fqn,
                    span,
                    composite_transport_gap_id(metadata),
                    detail,
                )
            })?;
            (metadata.source_ty, cg_ty)
        };
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty).map_err(|_| {
            composite_transport_gate_error(
                body_fqn,
                span,
                composite_transport_gap_id(metadata),
                "composite transport use site cannot be normalized to an LLVM layout descriptor",
            )
        })?;
        let mut gc_slot_offsets = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(span, llvm_ty, 0, &mut gc_slot_offsets)?;
        gc_slot_offsets.sort_unstable();
        gc_slot_offsets.dedup();

        let trace_hook = metadata.requirements.trace && !gc_slot_offsets.is_empty();

        let mut size_bytes = store_size_bytes(self, llvm_ty);
        let mut align_bytes = u64::from(abi_align_bytes(self, llvm_ty));
        let clayout_aligned = self
            .equivalent_codegen_type_id(mir_types, metadata.source_ty)
            .and_then(|codegen_ty| self.struct_clayout(codegen_ty).and_then(|c| c.aligned));
        if let Some(aligned) = clayout_aligned {
            align_bytes = align_bytes.max(u64::from(aligned));
            size_bytes = super::align_to(size_bytes, align_bytes);
        }

        let gc_slot_offset_parts = gc_slot_offsets
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>();
        let stable_name_key = canonical_record(
            "composite_transport_desc",
            [
                composite_transport_storage_kind(metadata)
                    .as_str()
                    .to_string(),
                composite_transport_kind_key(metadata.kind).to_string(),
                self.canonical_type_key_text_for_codegen(
                    source_ty,
                    "composite transport descriptor source type",
                )?,
                size_bytes.to_string(),
                align_bytes.to_string(),
                canonical_list(&gc_slot_offset_parts),
                trace_hook.to_string(),
                metadata.requirements.copy.to_string(),
                metadata.requirements.drop.to_string(),
            ],
        );

        Ok(CompositeTransportLayoutDescriptor {
            source_ty,
            source_name: mir_types.display(metadata.source_ty).to_string(),
            stable_name_key,
            kind: metadata.kind,
            storage_kind: composite_transport_storage_kind(metadata),
            size_bytes,
            align_bytes,
            gc_slot_offsets,
            trace_hook,
            copy_hook: metadata.requirements.copy,
            drop_hook: metadata.requirements.drop,
        })
    }

    fn value_transport_needs_composite_layout(
        &self,
        mir_types: &TypeStore,
        metadata: &ValueTransportMetadata,
    ) -> bool {
        metadata.boxing.is_some()
            || matches!(
                metadata.kind,
                MirTransportKind::Tuple
                    | MirTransportKind::Struct
                    | MirTransportKind::EnumPayload
                    | MirTransportKind::ClosureEnv
                    | MirTransportKind::CaptureBox
                    | MirTransportKind::ArrayElement
                    | MirTransportKind::EffectPayload
            )
            || self.type_needs_composite_transport_layout(mir_types, metadata.source_ty)
    }

    pub(super) fn array_element_transport_needs_composite_runtime(
        &self,
        mir_types: &TypeStore,
        metadata: &ValueTransportMetadata,
    ) -> bool {
        metadata
            .boxing
            .as_ref()
            .is_some_and(|boxing| boxing.reason == MirBoxingReason::ArrayElement)
            || self.type_needs_composite_transport_layout(mir_types, metadata.source_ty)
    }

    fn type_needs_composite_transport_layout(&self, mir_types: &TypeStore, ty: TypeId) -> bool {
        match mir_types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Nominal(_))
                if is_builtin_scalar_nominal_value_type(mir_types, ty) =>
            {
                false
            }
            TypeKind::Value(ValueTypeKind::Option(_))
            | TypeKind::Value(ValueTypeKind::Tuple(_))
            | TypeKind::Value(ValueTypeKind::Nominal(_)) => true,
            TypeKind::Value(
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_),
            )
            | TypeKind::Ref(_)
            | TypeKind::StarProjection(_)
            | TypeKind::Param(_) => false,
        }
    }

    pub(super) fn composite_transport_requirements_for_type(
        &mut self,
        mir_types: &TypeStore,
        ty: TypeId,
    ) -> crate::mir::MirTransportRequirements {
        let trace = self.type_contains_traceable_ref(mir_types, ty);
        crate::mir::MirTransportRequirements {
            trace,
            copy: true,
            drop: trace || self.type_needs_composite_transport_layout(mir_types, ty),
        }
    }

    fn type_contains_traceable_ref(&mut self, mir_types: &TypeStore, ty: TypeId) -> bool {
        match mir_types.kind(ty) {
            TypeKind::Ref(_) => true,
            TypeKind::StarProjection(star) => {
                self.type_contains_traceable_ref(mir_types, star.read_ty)
            }
            TypeKind::Value(ValueTypeKind::Option(_)) => {
                crate::mir::mir_transport_trace_requirement_for_type(mir_types, ty)
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements
                .iter()
                .any(|element| self.type_contains_traceable_ref(mir_types, *element)),
            TypeKind::Value(ValueTypeKind::Nominal(_)) => self
                .cg_ty_of_mir_type(mir_types, ty)
                .and_then(|cg_ty| self.llvm_basic_type_of(Span::new(0, 0), cg_ty).ok())
                .is_some_and(|llvm_ty| {
                    let mut offsets = Vec::new();
                    self.collect_gc_ptr_offsets_in_basic_type(
                        Span::new(0, 0),
                        llvm_ty,
                        0,
                        &mut offsets,
                    )
                    .is_ok()
                        && !offsets.is_empty()
                }),
            TypeKind::Value(
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_),
            )
            | TypeKind::Param(_) => false,
        }
    }

    fn get_or_create_composite_transport_descriptor_global(
        &mut self,
        descriptor: &CompositeTransportLayoutDescriptor,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = composite_transport_descriptor_global_name(descriptor);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let desc_ty = self.llvm_composite_transport_descriptor_type();
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let slot_ptr = self.get_or_create_composite_gc_slot_offsets_global(
            &global_name,
            &descriptor.gc_slot_offsets,
        );
        let trace_fn = if descriptor.trace_hook {
            self.declare_runtime_composite_trace()
                .as_global_value()
                .as_pointer_value()
                .const_cast(ptr_ty)
        } else {
            ptr_ty.const_null()
        };
        let copy_fn = if descriptor.copy_hook {
            self.declare_runtime_composite_copy()
                .as_global_value()
                .as_pointer_value()
                .const_cast(ptr_ty)
        } else {
            ptr_ty.const_null()
        };
        let drop_fn = if descriptor.drop_hook {
            self.declare_runtime_composite_drop()
                .as_global_value()
                .as_pointer_value()
                .const_cast(ptr_ty)
        } else {
            ptr_ty.const_null()
        };

        let values: [BasicValueEnum<'ctx>; 11] = [
            i32_ty.const_zero().into(),
            i32_ty
                .const_int(descriptor.storage_kind.as_u32() as u64, false)
                .into(),
            i64_ty.const_int(descriptor.size_bytes, false).into(),
            i64_ty.const_int(descriptor.align_bytes, false).into(),
            slot_ptr.into(),
            i32_ty
                .const_int(descriptor.gc_slot_offsets.len() as u64, false)
                .into(),
            i32_ty.const_zero().into(),
            trace_fn.into(),
            copy_fn.into(),
            drop_fn.into(),
            ptr_ty.const_null().into(),
        ];
        let gv = self.module.add_global(desc_ty, None, &global_name);
        gv.set_initializer(&desc_ty.const_named_struct(&values));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(gv)
    }

    fn get_or_create_composite_gc_slot_offsets_global(
        &mut self,
        descriptor_global_name: &str,
        offsets: &[u64],
    ) -> PointerValue<'ctx> {
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        if offsets.is_empty() {
            return ptr_ty.const_null();
        }

        let global_name = format!("{descriptor_global_name}__gc_slots");
        if let Some(existing) = self.module.get_global(&global_name) {
            return existing.as_pointer_value().const_cast(ptr_ty);
        }

        let i64_ty = self.context.i64_type();
        let arr_ty = i64_ty.array_type(offsets.len() as u32);
        let values = offsets
            .iter()
            .map(|offset| i64_ty.const_int(*offset, false))
            .collect::<Vec<_>>();
        let gv = self.module.add_global(arr_ty, None, &global_name);
        gv.set_initializer(&i64_ty.const_array(&values));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        gv.as_pointer_value().const_cast(ptr_ty)
    }
}

fn store_size_bytes<'a, 'ctx>(codegen: &MainCodegen<'a, 'ctx>, ty: BasicTypeEnum<'ctx>) -> u64 {
    match ty {
        BasicTypeEnum::ArrayType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::FloatType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::IntType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::PointerType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::StructType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::VectorType(ty) => codegen.target_data.get_store_size(&ty),
        BasicTypeEnum::ScalableVectorType(ty) => codegen.target_data.get_store_size(&ty),
    }
}

fn abi_align_bytes<'a, 'ctx>(codegen: &MainCodegen<'a, 'ctx>, ty: BasicTypeEnum<'ctx>) -> u32 {
    match ty {
        BasicTypeEnum::ArrayType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::FloatType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::IntType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::PointerType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::StructType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::VectorType(ty) => codegen.target_data.get_abi_alignment(&ty),
        BasicTypeEnum::ScalableVectorType(ty) => codegen.target_data.get_abi_alignment(&ty),
    }
}

fn composite_transport_storage_kind(
    metadata: &ValueTransportMetadata,
) -> CompositeTransportStorageKind {
    match metadata.boxing.as_ref().map(|boxing| boxing.reason) {
        Some(
            MirBoxingReason::AnyErasure
            | MirBoxingReason::RefErasure
            | MirBoxingReason::FunctionValueAdapter,
        ) => CompositeTransportStorageKind::Erased,
        Some(MirBoxingReason::EffectPayload | MirBoxingReason::ClosureCapture) => {
            CompositeTransportStorageKind::Boxed
        }
        Some(MirBoxingReason::ArrayElement) => CompositeTransportStorageKind::Inline,
        None if matches!(
            metadata.kind,
            MirTransportKind::ClosureEnv | MirTransportKind::CaptureBox
        ) =>
        {
            CompositeTransportStorageKind::Boxed
        }
        None => CompositeTransportStorageKind::Inline,
    }
}

fn composite_transport_gap_id(metadata: &ValueTransportMetadata) -> &'static str {
    if matches!(
        metadata.kind,
        MirTransportKind::ClosureEnv | MirTransportKind::CaptureBox
    ) || metadata
        .boxing
        .as_ref()
        .is_some_and(|boxing| boxing.reason == MirBoxingReason::ClosureCapture)
    {
        return "PIPELINE_GAPS §3.11";
    }
    if metadata.kind == MirTransportKind::ArrayElement
        || metadata
            .boxing
            .as_ref()
            .is_some_and(|boxing| boxing.reason == MirBoxingReason::ArrayElement)
    {
        return "PIPELINE_GAPS §4.5";
    }
    if metadata.kind == MirTransportKind::EnumPayload {
        return "PIPELINE_GAPS §4.4";
    }
    if metadata.kind == MirTransportKind::EffectPayload {
        return "PIPELINE_GAPS §5.5";
    }
    "PIPELINE_GAPS §4.1"
}

fn composite_transport_descriptor_global_name(
    descriptor: &CompositeTransportLayoutDescriptor,
) -> String {
    PrivateSymbolMangler.mangle(
        "composite_transport_desc",
        &CanonicalTextKey::new(descriptor.stable_name_key.clone()),
    )
}

pub(super) fn composite_transport_gate_error(
    body_fqn: &str,
    span: Span,
    gap_id: &'static str,
    detail: &'static str,
) -> LlvmEmitError {
    let entry = crate::llvm::codegen_gap_inventory::codegen_gap_entry(gap_id)
        .expect("composite transport gate gap id must be in inventory");
    LlvmEmitError::BackendGate(Box::new(BackendGateError {
        body_fqn: body_fqn.to_string(),
        source_span: span,
        gap_id: entry.gap_id,
        owner_task: entry.owner_task,
        suggested_owner: entry.suggested_owner,
        route: entry.route.as_str(),
        detail,
        at: span.into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{MirBoxingIntent, MirTransportRequirements};
    use crate::ty::TypeStore;

    fn transport(source_ty: TypeId, kind: MirTransportKind) -> ValueTransportMetadata {
        ValueTransportMetadata {
            source_ty,
            kind,
            requirements: MirTransportRequirements::plain_value(),
            boxing: None,
        }
    }

    #[test]
    fn refactor_llvm_composite_transport_contract_maps_owner_specific_gates() {
        let mut types = TypeStore::new();
        let source_ty = types.intern_builtins().unit;
        assert_eq!(
            composite_transport_gap_id(&transport(source_ty, MirTransportKind::CaptureBox)),
            "PIPELINE_GAPS §3.11"
        );
        assert_eq!(
            composite_transport_gap_id(&transport(source_ty, MirTransportKind::ArrayElement)),
            "PIPELINE_GAPS §4.5"
        );
        assert_eq!(
            composite_transport_gap_id(&transport(source_ty, MirTransportKind::EnumPayload)),
            "PIPELINE_GAPS §4.4"
        );
        assert_eq!(
            composite_transport_gap_id(&transport(source_ty, MirTransportKind::EffectPayload)),
            "PIPELINE_GAPS §5.5"
        );
    }

    #[test]
    fn refactor_llvm_composite_transport_contract_rejects_fake_trace_hook() {
        let mut types = TypeStore::new();
        let source_ty = types.intern_builtins().unit;
        let metadata = ValueTransportMetadata {
            source_ty,
            kind: MirTransportKind::Struct,
            requirements: MirTransportRequirements {
                trace: true,
                copy: true,
                drop: true,
            },
            boxing: Some(MirBoxingIntent {
                source_ty,
                target_ty: None,
                reason: MirBoxingReason::AnyErasure,
            }),
        };
        let descriptor = CompositeTransportLayoutDescriptor {
            source_ty,
            source_name: "sample.Traceable".to_string(),
            stable_name_key: "test".to_string(),
            kind: MirTransportKind::Struct,
            storage_kind: CompositeTransportStorageKind::Erased,
            size_bytes: 16,
            align_bytes: 8,
            gc_slot_offsets: Vec::new(),
            trace_hook: false,
            copy_hook: true,
            drop_hook: true,
        };

        assert_eq!(
            descriptor.validate(&metadata),
            Err(
                "traceable composite transport layout descriptor is missing a GC slot map or trace hook"
            )
        );
    }
}
