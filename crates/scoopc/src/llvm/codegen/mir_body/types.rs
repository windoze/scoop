//! Local storage / assignment / call result CgTy classification helpers.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn mir_local_storage_cg_ty(
        &mut self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local_id: crate::mir::LocalId,
        local: &crate::mir::LocalDecl,
    ) -> Result<CgTy, LlvmEmitError> {
        let local_cg = self.cg_ty_of_mir_type(mir_types, local.ty);
        let mut member_field_cg = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let crate::mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local_id {
                    continue;
                }
                let crate::mir::Rvalue::MemberAccess {
                    receiver, member, ..
                } = value
                else {
                    continue;
                };
                if !matches!(
                    member.resolved,
                    Some(crate::mir::MemberTarget::Value { .. })
                ) {
                    continue;
                }
                let Ok(field_cg) =
                    self.mir_member_field_cg_ty(stmt.span, body, mir_types, receiver, member)
                else {
                    continue;
                };
                if let Some(previous) = member_field_cg {
                    if !self.cg_ty_layout_equivalent(previous, field_cg) {
                        panic!(
                            "mir_local_storage_cg_ty: MIR verifier accepted member field type drift"
                        );
                    }
                } else {
                    member_field_cg = Some(field_cg);
                }
            }
        }
        if let Some(field_cg) = member_field_cg
            && (matches!(local.source, crate::mir::LocalSourceKind::CompilerTemporary)
                || local_cg.is_some_and(|local_cg| {
                    self.mir_type_contains_param(mir_types, local.ty)
                        || self.cg_ty_layout_equivalent(local_cg, field_cg)
                }))
        {
            return Ok(field_cg);
        }
        if let Some(assigned_cg) = self.mir_local_assignment_cg_ty(body, mir_types, local_id)
            && matches!(local.source, crate::mir::LocalSourceKind::CompilerTemporary)
            && (local_cg.is_none()
                || matches!(local_cg, Some(CgTy::Ref))
                || matches!(assigned_cg, CgTy::Enum(_))
                || self.mir_type_contains_param(mir_types, local.ty)
                || local_cg
                    .is_some_and(|local_cg| self.cg_ty_layout_equivalent(local_cg, assigned_cg)))
        {
            return Ok(assigned_cg);
        }
        Ok(local_cg.unwrap_or_else(|| {
            panic!("mir_local_storage_cg_ty: MIR verifier accepted unsupported local type")
        }))
    }

    pub(in crate::llvm::codegen) fn mir_local_assignment_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local_id: crate::mir::LocalId,
    ) -> Option<CgTy> {
        let mut inferred = None;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let crate::mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                if *target != local_id {
                    continue;
                }
                let candidate = match value {
                    crate::mir::Rvalue::Use(operand) => {
                        self.mir_operand_cg_ty(body, mir_types, operand)?
                    }
                    crate::mir::Rvalue::Transport { value, transport } => {
                        self.mir_transport_result_cg_ty(body, mir_types, value, transport)?
                    }
                    crate::mir::Rvalue::TypeCheck { .. } => CgTy::Bool,
                    crate::mir::Rvalue::Cast { target_ty, .. } => {
                        self.cg_ty_of_mir_type(mir_types, *target_ty)?
                    }
                    crate::mir::Rvalue::Call { kind, .. } => {
                        self.mir_call_result_cg_ty(body, mir_types, kind)?
                    }
                    crate::mir::Rvalue::MemberAccess { member, .. } => {
                        self.mir_member_resolved_static_value_cg_ty(member)?
                    }
                    crate::mir::Rvalue::TupleGet { tuple, index } => {
                        self.mir_tuple_get_result_cg_ty(body, mir_types, tuple, *index)?
                    }
                    _ => continue,
                };
                match inferred {
                    Some(existing) if !self.cg_ty_layout_equivalent(existing, candidate) => {
                        return None;
                    }
                    Some(_) => {}
                    None => inferred = Some(candidate),
                }
            }
        }
        inferred
    }

    pub(in crate::llvm::codegen) fn mir_call_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        kind: &crate::mir::CallKind,
    ) -> Option<CgTy> {
        match kind {
            crate::mir::CallKind::Direct { callee_fqn } => {
                if self.registered_class_instance_key(callee_fqn).is_some() {
                    return Some(CgTy::Ref);
                }
                if matches!(
                    mir_direct_call_base_fqn(callee_fqn),
                    "scoop.core.size" | "scoop.core.Array.size" | "scoop.core.MutableArray.size"
                ) {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: true,
                    }));
                }
                let signature = self
                    .published_codegen_callable_signature(callee_fqn)
                    .or_else(|| {
                        let base = mir_direct_call_base_fqn(callee_fqn);
                        (base != callee_fqn)
                            .then(|| self.published_codegen_callable_signature(base))
                            .flatten()
                    })?;
                self.try_cg_ty_of_type_id(signature.return_ty)
            }
            crate::mir::CallKind::Closure { callee, .. }
            | crate::mir::CallKind::FunValue { callee }
            | crate::mir::CallKind::FunPtr { callee } => {
                let fun_ty = self
                    .mir_operand_funptr_function_type(body, mir_types, callee)
                    .or_else(|| self.mir_operand_function_type(body, mir_types, callee))?;
                self.cg_ty_of_mir_type(mir_types, fun_ty.return_ty)
            }
            crate::mir::CallKind::Resume { resume, .. } => {
                self.cg_ty_of_mir_type(mir_types, resume.answer_ty)
            }
            crate::mir::CallKind::Virtual { .. } | crate::mir::CallKind::Interface { .. } => None,
        }
    }

    pub(in crate::llvm::codegen) fn cg_ty_layout_equivalent(&self, lhs: CgTy, rhs: CgTy) -> bool {
        if lhs == rhs {
            return true;
        }
        match (lhs, rhs) {
            (CgTy::Tuple(lhs), CgTy::Tuple(rhs))
            | (CgTy::Struct(lhs), CgTy::Struct(rhs))
            | (CgTy::Enum(lhs), CgTy::Enum(rhs)) => {
                let lhs = self.types.display(lhs.inner()).to_string();
                let rhs = self.types.display(rhs.inner()).to_string();
                lhs == rhs || lhs.replace(", eff Pure", "") == rhs.replace(", eff Pure", "")
            }
            _ => false,
        }
    }

    pub(in crate::llvm::codegen) fn describe_cg_ty(&self, cg_ty: CgTy) -> String {
        match cg_ty {
            CgTy::Tuple(ty) | CgTy::Struct(ty) | CgTy::Enum(ty) => {
                format!("{cg_ty:?} {}", self.types.display(ty.inner()))
            }
            _ => format!("{cg_ty:?}"),
        }
    }

    pub(in crate::llvm::codegen) fn mir_type_contains_param(
        &self,
        types: &TypeStore,
        ty: TypeId,
    ) -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match types.kind(id) {
                TypeKind::Param(_) => return true,
                TypeKind::StarProjection(star) => stack.push(star.read_ty),
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    stack.extend(nominal.args.iter().copied());
                    if let Some(eff) = &nominal.eff {
                        stack.extend(eff.terms.iter().copied());
                    }
                }
                TypeKind::Ref(RefTypeKind::Function(fun)) => {
                    if let Some(receiver) = fun.receiver {
                        stack.push(receiver);
                    }
                    stack.extend(fun.params.iter().copied());
                    stack.push(fun.return_ty);
                    stack.extend(fun.effects.terms.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Union(union)) => {
                    stack.extend(union.variants.iter().copied());
                }
                TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                    stack.extend(elements.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                | TypeKind::Value(
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
                ) => {}
            }
        }
        false
    }

    pub(in crate::llvm::codegen) fn cg_ty_of_mir_type(
        &self,
        mir_types: &TypeStore,
        ty: TypeId,
    ) -> Option<CgTy> {
        let same_type_store = std::ptr::eq(mir_types, self.types);
        let cg_ty_from_codegen_store = || {
            same_type_store
                .then(|| self.try_cg_ty_of_type_id(ty))
                .flatten()
        };
        match mir_types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::StarProjection(star) => self.cg_ty_of_mir_type(mir_types, star.read_ty),
            TypeKind::Value(ValueTypeKind::Nothing) => Some(CgTy::Never),
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Char) => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Float64) => Some(CgTy::Float64),
            TypeKind::Value(ValueTypeKind::Float32) => Some(CgTy::Float32),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Option(_)) => self
                .equivalent_codegen_mono_type_id(mir_types, ty)
                .map(|codegen_ty| self.cg_ty_of(codegen_ty))
                .or_else(cg_ty_from_codegen_store),
            TypeKind::Value(ValueTypeKind::Tuple(_)) => self
                .equivalent_codegen_mono_type_id(mir_types, ty)
                .map(|codegen_ty| self.cg_ty_of(codegen_ty))
                .or_else(cg_ty_from_codegen_store),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => self
                .builtin_nominal_cg_ty(&nominal.fqn)
                .or_else(|| {
                    self.equivalent_codegen_mono_type_id(mir_types, ty)
                        .map(|codegen_ty| self.cg_ty_of(codegen_ty))
                })
                .or_else(cg_ty_from_codegen_store),
            TypeKind::Param(_) => None,
        }
    }

    pub(in crate::llvm::codegen) fn equivalent_codegen_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_kind = source_types.kind(source_ty);
        if let Some(candidate) = self
            .types
            .iter_ids()
            .find(|&candidate| self.types.kind(candidate) == source_kind)
        {
            return Some(candidate);
        }

        let source_display = source_types.display(source_ty).to_string();
        match source_kind {
            TypeKind::Ref(RefTypeKind::Nominal(source_nominal)) => self
                .builtin_nominal_codegen_type_id(source_nominal)
                .or_else(|| {
                    self.types.iter_ids().find(|candidate| {
                        matches!(
                            self.types.kind(*candidate),
                            TypeKind::Ref(RefTypeKind::Nominal(candidate_nominal))
                                if candidate_nominal.fqn == source_nominal.fqn
                                    && self.nominal_args_equivalent(
                                        source_types,
                                        &source_nominal.args,
                                        &candidate_nominal.args,
                                    )
                        )
                    })
                })
                .or_else(|| self.display_compatible_codegen_type_id(source_kind, &source_display)),
            TypeKind::Value(ValueTypeKind::Nominal(source_nominal)) => self
                .builtin_nominal_codegen_type_id(source_nominal)
                .or_else(|| {
                    self.types.iter_ids().find(|candidate| {
                        matches!(
                            self.types.kind(*candidate),
                            TypeKind::Value(ValueTypeKind::Nominal(candidate_nominal))
                                if candidate_nominal.fqn == source_nominal.fqn
                                    && self.nominal_args_equivalent(
                                        source_types,
                                        &source_nominal.args,
                                        &candidate_nominal.args,
                                    )
                        )
                    })
                })
                .or_else(|| self.display_compatible_codegen_type_id(source_kind, &source_display)),
            TypeKind::Value(ValueTypeKind::Tuple(source_elems)) => self
                .types
                .iter_ids()
                .find(|candidate| {
                    matches!(
                        self.types.kind(*candidate),
                        TypeKind::Value(ValueTypeKind::Tuple(candidate_elems))
                            if self.type_args_equivalent(source_types, source_elems, candidate_elems)
                    )
                })
                .or_else(|| self.display_compatible_codegen_type_id(source_kind, &source_display)),
            TypeKind::Value(ValueTypeKind::Option(source_inner)) => self
                .types
                .iter_ids()
                .find(|candidate| {
                    matches!(
                        self.types.kind(*candidate),
                        TypeKind::Value(ValueTypeKind::Option(candidate_inner))
                            if self.type_args_equivalent(
                                source_types,
                                std::slice::from_ref(source_inner),
                                std::slice::from_ref(candidate_inner),
                            )
                    )
                })
                .or_else(|| self.display_compatible_codegen_type_id(source_kind, &source_display)),
            _ => self.display_compatible_codegen_type_id(source_kind, &source_display),
        }
    }

    pub(in crate::llvm::codegen) fn equivalent_codegen_mono_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<MonoTypeId> {
        let codegen_ty = self.equivalent_codegen_type_id(source_types, source_ty)?;
        Some(self.mono_type_id(codegen_ty, "cross-TypeStore codegen type bridge"))
    }

    fn nominal_args_equivalent(
        &self,
        source_types: &TypeStore,
        source_args: &[TypeId],
        candidate_args: &[TypeId],
    ) -> bool {
        source_args.len() == candidate_args.len()
            && self.type_args_equivalent(source_types, source_args, candidate_args)
    }

    fn type_args_equivalent(
        &self,
        source_types: &TypeStore,
        source_args: &[TypeId],
        candidate_args: &[TypeId],
    ) -> bool {
        source_args
            .iter()
            .zip(candidate_args.iter())
            .all(|(source_arg, candidate_arg)| {
                self.equivalent_codegen_type_id(source_types, *source_arg) == Some(*candidate_arg)
            })
    }

    fn display_compatible_codegen_type_id(
        &self,
        source_kind: &TypeKind,
        source_display: &str,
    ) -> Option<TypeId> {
        self.types.iter_ids().find(|&candidate| {
            self.types.display(candidate).to_string() == source_display
                && type_display_mapping_is_kind_compatible(source_kind, self.types.kind(candidate))
        })
    }

    fn builtin_nominal_codegen_type_id(&self, nominal: &NominalType) -> Option<TypeId> {
        if !nominal.args.is_empty() || nominal.eff.is_some() {
            return None;
        }
        match nominal.fqn.as_str() {
            "scoop.core.Any" => Some(self.builtins.any),
            "scoop.core.String" => Some(self.builtins.string),
            "scoop.core.Unit" => Some(self.builtins.unit),
            "scoop.core.Nothing" => Some(self.builtins.nothing),
            "scoop.core.Bool" => Some(self.builtins.bool_),
            "scoop.core.Char" => Some(self.builtins.char_),
            "scoop.core.Float64" | "scoop.core.Double" => Some(self.builtins.float64),
            "scoop.core.Float32" => Some(self.builtins.float32),
            "scoop.core.Int" | "scoop.unsafe.__AtomicInt" => Some(self.builtins.int),
            "scoop.core.UInt" | "scoop.core.UIntPtr" | "scoop.unsafe.FunPtr" => {
                Some(self.builtins.uint)
            }
            "scoop.core.Byte" | "scoop.core.UInt8" => {
                self.find_codegen_value_type(ValueTypeKind::UIntN(8))
            }
            "scoop.core.Short" | "scoop.core.Int16" => {
                self.find_codegen_value_type(ValueTypeKind::IntN(16))
            }
            "scoop.core.UShort" | "scoop.core.UInt16" => {
                self.find_codegen_value_type(ValueTypeKind::UIntN(16))
            }
            "scoop.core.Int8" => self.find_codegen_value_type(ValueTypeKind::IntN(8)),
            "scoop.core.Int32" => self.find_codegen_value_type(ValueTypeKind::IntN(32)),
            "scoop.core.Int64" | "scoop.core.Long" => {
                self.find_codegen_value_type(ValueTypeKind::IntN(64))
            }
            "scoop.core.UInt32" => self.find_codegen_value_type(ValueTypeKind::UIntN(32)),
            "scoop.core.UInt64" | "scoop.core.ULong" => {
                self.find_codegen_value_type(ValueTypeKind::UIntN(64))
            }
            _ => None,
        }
    }

    fn find_codegen_value_type(&self, needle: ValueTypeKind) -> Option<TypeId> {
        self.types
            .iter_ids()
            .find(|id| self.types.kind(*id) == &TypeKind::Value(needle.clone()))
    }

    pub(in crate::llvm::codegen) fn runtime_type_descriptor_is_codegen_supported(
        &self,
        mir_types: &TypeStore,
        metadata: &crate::mir::RuntimeTypeTestMetadata,
    ) -> bool {
        if !matches!(
            metadata.descriptor.kind,
            crate::mir::RuntimeTypeDescriptorKind::Any
                | crate::mir::RuntimeTypeDescriptorKind::Function
                | crate::mir::RuntimeTypeDescriptorKind::String
                | crate::mir::RuntimeTypeDescriptorKind::Nominal { .. }
        ) {
            return false;
        }
        self.equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .and_then(|target_ty| self.try_cg_ty_of_type_id(target_ty))
            .is_some_and(|target_cg| matches!(target_cg, CgTy::Ref | CgTy::String))
    }

    pub(in crate::llvm::codegen) fn runtime_pattern_type_descriptor_is_codegen_supported(
        &self,
        mir_types: &TypeStore,
        metadata: &crate::mir::RuntimePatternTypeTestMetadata,
    ) -> bool {
        if !matches!(
            metadata.descriptor.kind,
            crate::mir::RuntimeTypeDescriptorKind::Any
                | crate::mir::RuntimeTypeDescriptorKind::Function
                | crate::mir::RuntimeTypeDescriptorKind::String
                | crate::mir::RuntimeTypeDescriptorKind::Nominal { .. }
        ) {
            return false;
        }
        self.equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .and_then(|target_ty| self.try_cg_ty_of_type_id(target_ty))
            .is_some_and(|target_cg| matches!(target_cg, CgTy::Ref | CgTy::String))
    }

    pub(in crate::llvm::codegen) fn mir_tuple_get_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
    ) -> Option<CgTy> {
        let tuple_ty = self.mir_operand_type_id(body, tuple)?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            return None;
        };
        let element_ty = *elements.get(index)?;
        self.cg_ty_of_mir_type(mir_types, element_ty)
    }

    pub(in crate::llvm::codegen) fn mir_member_resolved_top_level_value_fqn<'m>(
        &self,
        member: &'m crate::mir::MemberAccessMetadata,
    ) -> Option<&'m str> {
        let Some(crate::mir::MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
            return None;
        };
        (self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton)
            || self.lookup_object_property_by_fqn(fqn).is_some()
            || self.lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelImmutableVal)
            || self.lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar)
            || self.has_extern_global_contract(fqn)
            || self.mir_member_resolved_enum_unit_variant_fqn(fqn))
        .then_some(fqn.as_str())
    }

    pub(in crate::llvm::codegen) fn mir_member_resolved_static_value_cg_ty(
        &self,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Option<CgTy> {
        let crate::mir::MemberTarget::Value { fqn } = member.resolved.as_ref()? else {
            return None;
        };
        if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton) {
            return Some(CgTy::Ref);
        }
        if let Some((_object, prop)) = self.lookup_object_property_by_fqn(fqn) {
            return self.try_cg_ty_of_type_id(prop.ty);
        }
        if let Some(root) = self.lir_global_root(fqn)
            && matches!(
                root.kind,
                LirGlobalRootKind::TopLevelImmutableVal
                    | LirGlobalRootKind::TopLevelMutableVar
                    | LirGlobalRootKind::ExternGlobal
            )
        {
            return self.try_cg_ty_of_type_id(root.ty?);
        }
        let (owner_fqn, variant_name) = fqn.rsplit_once('.')?;
        let layout = self.enum_layouts.get(owner_fqn)?;
        layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name && variant.fields.is_empty())?;
        self.types
            .iter_ids()
            .find(|id| {
                matches!(
                    self.types.kind(*id),
                    TypeKind::Value(ValueTypeKind::Nominal(nominal))
                        if nominal.fqn == owner_fqn && nominal.args.is_empty() && nominal.eff.is_none()
                )
            })
            .and_then(|ty| self.try_mono_type_id(ty))
            .map(CgTy::Enum)
    }

    pub(in crate::llvm::codegen) fn mir_member_resolved_enum_unit_variant_fqn(
        &self,
        fqn: &str,
    ) -> bool {
        let Some((owner_fqn, variant_name)) = fqn.rsplit_once('.') else {
            return false;
        };
        self.enum_layouts
            .get(owner_fqn)
            .and_then(|layout| {
                layout
                    .variants
                    .iter()
                    .find(|variant| variant.name == variant_name)
            })
            .is_some_and(|variant| variant.fields.is_empty())
    }

    pub(in crate::llvm::codegen) fn mir_member_field_cg_ty(
        &mut self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Result<CgTy, LlvmEmitError> {
        let field_fqn = mir_member_value_fqn_for_codegen(span, member)?;
        let receiver_type_id =
            self.mir_member_receiver_codegen_type_id(span, body, mir_types, receiver, member)?;
        if let Some((_class, _field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, span, Some(receiver_type_id))?
        {
            return Ok(field_cg);
        }

        let receiver_cg = self.cg_ty_of_type_id(receiver_type_id, "MIR member field receiver type");
        let CgTy::Struct(struct_ty) = receiver_cg else {
            return Err(frontend_error(format!(
                "pass MIR member field target `{field_fqn}` receiver_ty=t{} receiver_cg={}",
                receiver_type_id.as_u32(),
                self.describe_cg_ty(receiver_cg),
            )));
        };
        let (_field_idx, field_cg) = self.lookup_struct_field(struct_ty, field_fqn, span)?;
        Ok(field_cg)
    }

    pub(in crate::llvm::codegen) fn mir_transport_result_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        value: &crate::mir::Operand,
        transport: &crate::mir::ValueTransportMetadata,
    ) -> Option<CgTy> {
        self.mir_operand_cg_ty(body, mir_types, value)?;
        let boxing = transport.boxing.as_ref()?;
        if !matches!(
            boxing.reason,
            crate::mir::MirBoxingReason::AnyErasure | crate::mir::MirBoxingReason::RefErasure
        ) || boxing.source_ty != transport.source_ty
        {
            return None;
        }
        if matches!(
            mir_types.kind(transport.source_ty),
            TypeKind::Value(ValueTypeKind::Nothing)
        ) {
            return Some(CgTy::Ref);
        }
        let source_ty = self.equivalent_codegen_type_id(mir_types, transport.source_ty)?;
        let source_cg = self.try_cg_ty_of_type_id(source_ty)?;
        match source_cg {
            CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Unit
            | CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Enum(_) => Some(CgTy::Ref),
            CgTy::Float64 | CgTy::Float32 | CgTy::Never => None,
        }
    }

    pub(in crate::llvm::codegen) fn mir_enum_payload_schema_matches(
        &self,
        mir_types: &TypeStore,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
        args: &[crate::mir::CallArg],
        payload: &crate::mir::AggregateTransportMetadata,
    ) -> bool {
        if payload.kind != crate::mir::AggregateTransportKind::EnumPayload {
            return false;
        }
        let Some(payload_enum_ty) =
            self.equivalent_codegen_type_id(mir_types, payload.aggregate_ty)
        else {
            return false;
        };
        if payload_enum_ty != enum_ty
            || payload.fields.len() != args.len()
            || variant.fields.len() != args.len()
        {
            return false;
        }

        for (idx, ((field, arg), field_cg)) in payload
            .fields
            .iter()
            .zip(args)
            .zip(variant.fields.iter())
            .enumerate()
        {
            if field.index != idx || field.name.as_deref() != arg.name.as_deref() {
                return false;
            }
            if field.transport.source_ty != field.ty
                || field
                    .transport
                    .boxing
                    .as_ref()
                    .is_some_and(|boxing| boxing.source_ty != field.ty)
            {
                return false;
            }
            let Some(field_ty) = self.equivalent_codegen_type_id(mir_types, field.ty) else {
                return false;
            };
            let Some(expected_cg) = self.try_cg_ty_of_type_id(field_ty) else {
                return false;
            };
            if expected_cg != *field_cg {
                return false;
            }
        }

        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn mir_member_receiver_codegen_type_id(
        &self,
        _span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        receiver: &crate::mir::Operand,
        member: &crate::mir::MemberAccessMetadata,
    ) -> Result<TypeId, LlvmEmitError> {
        let receiver_source_ty = match receiver {
            crate::mir::Operand::Local(local) => body
                .locals
                .get(local.as_u32() as usize)
                .map(|local| local.ty)
                .unwrap_or(member.receiver_ty),
            crate::mir::Operand::Const(_) => member.receiver_ty,
        };
        Ok(self
            .equivalent_codegen_type_id(mir_types, receiver_source_ty)
            .or_else(|| self.equivalent_codegen_type_id(mir_types, member.receiver_ty))
            .unwrap_or_else(|| {
                panic!("mir_member_receiver_codegen_type_id: verifier accepted member receiver TypeStore drift")
            }))
    }

    pub(in crate::llvm::codegen) fn equivalent_runtime_ref_codegen_type_id(
        &self,
        source_types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = source_types.display(source_ty).to_string();
        self.types.iter_ids().find(|&candidate| {
            self.types.display(candidate).to_string() == source_display
                && matches!(self.types.kind(candidate), TypeKind::Ref(_))
        })
    }

    pub(in crate::llvm::codegen) fn mir_class_ctor_layout_key(
        &self,
        span: crate::span::Span,
        class_fqn: &str,
        mir_types: &TypeStore,
        target_source_ty: Option<TypeId>,
    ) -> Result<hir::ClassInstanceKey, LlvmEmitError> {
        let Some(target_source_ty) = target_source_ty else {
            return Err(frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} target local missing typed nominal result"
            )));
        };
        let Some(codegen_ty) = self.equivalent_codegen_type_id(mir_types, target_source_ty) else {
            return Err(frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} result type t{} has no codegen TypeStore equivalent",
                target_source_ty.as_u32()
            )));
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(codegen_ty) else {
            return Err(frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} target type t{} is not a nominal class reference",
                codegen_ty.as_u32()
            )));
        };
        if nominal.fqn != class_fqn {
            return Err(frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} target type resolves to mismatched nominal `{}`",
                nominal.fqn
            )));
        }
        let mono_ty = self.types.as_mono(codegen_ty).map_err(|leak| {
            frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} target type t{} is not fully monomorphic: {:?}",
                codegen_ty.as_u32(), leak.leak_path
            ))
        })?;
        let class_key = hir::ClassInstanceKey::from_mono_nominal(self.types, mono_ty)
            .expect("nominal result type must produce ClassInstanceKey");
        if !self.class_inits.contains_key(&class_key) {
            return Err(frontend_error(format!(
                "MIR class ctor `{class_fqn}` at {span:?} resolved to missing class layout key `{class_key}`"
            )));
        }
        Ok(class_key)
    }

    pub(in crate::llvm::codegen) fn equivalent_codegen_effect_row(
        &self,
        source_types: &TypeStore,
        source_row: &crate::ty::EffectRow,
    ) -> Option<crate::ty::EffectRow> {
        let mut terms = Vec::with_capacity(source_row.terms.len());
        for term in &source_row.terms {
            terms.push(self.equivalent_codegen_type_id(source_types, *term)?);
        }
        Some(crate::ty::EffectRow::new(terms))
    }

    pub(in crate::llvm::codegen) fn equivalent_codegen_function_type(
        &self,
        source_types: &TypeStore,
        fun_ty: &crate::ty::FunctionType,
    ) -> Option<crate::ty::FunctionType> {
        let receiver = match fun_ty.receiver {
            Some(ty) => Some(self.equivalent_codegen_type_id(source_types, ty)?),
            None => None,
        };
        let mut params = Vec::with_capacity(fun_ty.params.len());
        for param in &fun_ty.params {
            params.push(self.equivalent_codegen_type_id(source_types, *param)?);
        }
        Some(crate::ty::FunctionType {
            receiver,
            params,
            return_ty: self.equivalent_codegen_type_id(source_types, fun_ty.return_ty)?,
            effects: self.equivalent_codegen_effect_row(source_types, &fun_ty.effects)?,
            effects_closed: fun_ty.effects_closed,
        })
    }

    pub(in crate::llvm::codegen) fn mir_local_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        local: crate::mir::LocalId,
    ) -> Option<CgTy> {
        let local = body.locals.get(local.as_u32() as usize)?;
        self.cg_ty_of_mir_type(mir_types, local.ty)
    }

    pub(in crate::llvm::codegen) fn mir_operand_cg_ty(
        &self,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        operand: &crate::mir::Operand,
    ) -> Option<CgTy> {
        match operand {
            crate::mir::Operand::Local(local) => self.mir_local_cg_ty(body, mir_types, *local),
            crate::mir::Operand::Const(value) => self.mir_const_cg_ty(value),
        }
    }

    pub(in crate::llvm::codegen) fn mir_const_cg_ty(
        &self,
        value: &crate::mir::ConstValue,
    ) -> Option<CgTy> {
        match value {
            crate::mir::ConstValue::Bool(_) => Some(CgTy::Bool),
            crate::mir::ConstValue::Char => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            crate::mir::ConstValue::Unit => Some(CgTy::Unit),
            crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => {
                self.try_cg_ty_of_type_id(self.builtins.int)
            }
            crate::mir::ConstValue::Float64 => Some(CgTy::Float64),
            crate::mir::ConstValue::Float32 => Some(CgTy::Float32),
            crate::mir::ConstValue::String | crate::mir::ConstValue::SynthString(_) => {
                Some(CgTy::String)
            }
        }
    }
}

fn type_display_mapping_is_kind_compatible(source: &TypeKind, candidate: &TypeKind) -> bool {
    matches!(
        (source, candidate),
        (TypeKind::Param(_), TypeKind::Param(_))
            | (TypeKind::Ref(_), TypeKind::Ref(_))
            | (TypeKind::Value(_), TypeKind::Value(_))
            | (TypeKind::StarProjection(_), TypeKind::StarProjection(_))
    )
}
