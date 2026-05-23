//! ABI value materialization and LLVM type lowering.
//!
//! Translates a source `TypeId` into the corresponding `AbiValue`
//! (carrier, source layout, LLVM type) used by every other module. Owns
//! the LLVM struct/integer/enum-layout primitives plus the case-tag and
//! private-helper-function declarations that the rest of the materializer
//! emits.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn abi_value(&mut self, ty: TypeId) -> Result<AbiValue<'ctx>, LlvmEmitError> {
        self.abi_value_from_types(self.source_types, ty)
    }

    pub(super) fn source_value_layout(
        &mut self,
        ty: TypeId,
    ) -> Result<SourceAbiLayout<'ctx>, LlvmEmitError> {
        if let Some(layout) = self.source_value_layouts.get(&ty) {
            return Ok(layout.clone());
        }

        let source_kind = self.source_types.kind(ty).clone();
        let layout = match source_kind {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let abi = self
                    .abi_value_from_types(self.source_types, ty)
                    .map_err(|err| self.wrap_source_value_layout_error(ty, err))?;
                let mut next_abi_field_index = 0u32;
                let mut fields = Vec::with_capacity(elements.len());
                for (source_index, element_ty) in elements.into_iter().enumerate() {
                    let element_layout = self.source_value_layout(element_ty)?;
                    let abi_field_index = if element_layout.abi().is_elided() {
                        None
                    } else {
                        let field_index = next_abi_field_index;
                        next_abi_field_index = next_abi_field_index.saturating_add(1);
                        Some(field_index)
                    };
                    fields.push(SourceAbiFieldLayout::new(
                        source_index as u32,
                        element_ty,
                        abi_field_index,
                        *element_layout.abi(),
                    ));
                }
                SourceAbiLayout::new(ty, SourceAbiLayoutKind::Tuple, abi, fields)
            }
            _ => {
                let abi = self
                    .abi_value_from_types(self.source_types, ty)
                    .map_err(|err| self.wrap_source_value_layout_error(ty, err))?;
                SourceAbiLayout::new(ty, SourceAbiLayoutKind::Scalar, abi, Vec::new())
            }
        };
        self.source_value_layouts.insert(ty, layout.clone());
        Ok(layout)
    }

    pub(super) fn materialize_class_instance_layouts(
        &self,
    ) -> Result<BTreeMap<TypeId, ClassInstanceLayout>, LlvmEmitError> {
        let mut layouts = BTreeMap::new();
        for ty in self.source_types.iter_ids() {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(ty) else {
                continue;
            };
            if self.type_contains_param_in_types(self.source_types, ty) {
                continue;
            }

            let mono_ty = self.source_types.as_mono(ty).map_err(|leak| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 concrete class `{}` 仍含未实例化类型参数: {:?}",
                    nominal.fqn, leak.leak_path
                ))
            })?;
            let Some(class_key) =
                crate::effect_lowered::source::ClassInstanceKey::from_mono_nominal(
                    self.source_types,
                    mono_ty,
                )
            else {
                continue;
            };
            let Some(class) = self.physical_class_layout(&class_key) else {
                continue;
            };
            if class
                .fields
                .iter()
                .any(|field| self.type_contains_param_in_types(self.source_types, field.ty))
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 concrete class `{class_key}` 的 field layout 仍含未实例化类型参数"
                )));
            }

            let fields = class
                .fields
                .iter()
                .map(|field| ClassInstanceFieldLayout::new(field.fqn.clone(), field.ty))
                .collect();
            layouts.insert(
                ty,
                ClassInstanceLayout::new(ty, nominal.fqn.clone(), class_key, fields),
            );
        }
        Ok(layouts)
    }

    pub(super) fn type_contains_param_in_types(&self, types: &TypeStore, ty: TypeId) -> bool {
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
                TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                    stack.extend(elements.iter().copied());
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
                TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                | TypeKind::Value(ValueTypeKind::Unit)
                | TypeKind::Value(ValueTypeKind::Nothing)
                | TypeKind::Value(ValueTypeKind::Bool)
                | TypeKind::Value(ValueTypeKind::Char)
                | TypeKind::Value(ValueTypeKind::Float64)
                | TypeKind::Value(ValueTypeKind::Float32)
                | TypeKind::Value(ValueTypeKind::Int)
                | TypeKind::Value(ValueTypeKind::UInt)
                | TypeKind::Value(ValueTypeKind::IntN(_))
                | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
            }
        }
        false
    }

    pub(super) fn resume_surface_abi_value(
        &mut self,
        ty: TypeId,
    ) -> Result<AbiValue<'ctx>, LlvmEmitError> {
        if matches!(self.source_types.kind(ty), TypeKind::Param(_)) {
            // Generic effect operations are represented at the effect-family resume surface before
            // an operation type parameter has a single concrete instantiation. That shared resume
            // slot uses the erased managed carrier; ordinary source values and callable invoke
            // arguments still fail fast on bare type params via `source_value_layout`.
            return Ok(AbiValue::new(
                self.codegen.llvm_gc_i8_ptr_type().into(),
                false,
            ));
        }
        self.source_value_layout(ty).map(|layout| *layout.abi())
    }

    pub(super) fn wrap_source_value_layout_error(
        &self,
        ty: TypeId,
        err: LlvmEmitError,
    ) -> LlvmEmitError {
        match err {
            LlvmEmitError::Frontend { message } => frontend_error(format!(
                "LLVM source-type ABI value lowering 无法为 `{}`（t{}）建立 authoritative contract: {message}",
                self.source_types.display(ty),
                ty.as_u32()
            )),
            other => other,
        }
    }

    pub(super) fn abi_value_from_types(
        &mut self,
        types: &TypeStore,
        ty: TypeId,
    ) -> Result<AbiValue<'ctx>, LlvmEmitError> {
        let llvm_ty = self.llvm_abi_type_of_types(types, ty)?;
        let elided = matches!(types.kind(ty), TypeKind::Value(ValueTypeKind::Nothing))
            || self.codegen.target_data.get_store_size(&llvm_ty) == 0;
        Ok(AbiValue::new(llvm_ty, elided))
    }

    pub(super) fn llvm_abi_type_of_types(
        &mut self,
        types: &TypeStore,
        ty: TypeId,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        match types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => {
                Ok(self.codegen.llvm_scoop_string_ptr_type().into())
            }
            TypeKind::Ref(_) => Ok(self.codegen.llvm_gc_i8_ptr_type().into()),
            TypeKind::StarProjection(star) => self.llvm_abi_type_of_types(types, star.read_ty),
            TypeKind::Value(ValueTypeKind::Nothing) => Ok(self.codegen.context.i8_type().into()),
            TypeKind::Value(ValueTypeKind::Unit) => {
                Ok(self.codegen.context.struct_type(&[], false).into())
            }
            TypeKind::Value(ValueTypeKind::Bool) => Ok(self.codegen.context.bool_type().into()),
            TypeKind::Value(ValueTypeKind::Char) => Ok(self.codegen.context.i32_type().into()),
            TypeKind::Value(ValueTypeKind::Float64) => Ok(self.codegen.context.f64_type().into()),
            TypeKind::Value(ValueTypeKind::Float32) => Ok(self.codegen.context.f32_type().into()),
            TypeKind::Value(ValueTypeKind::Int) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: true,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::UInt) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: false,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: u32::from(*bits),
                    signed: true,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: u32::from(*bits),
                    signed: false,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let mut fields = Vec::with_capacity(elements.len());
                for element in elements {
                    let element_ty = self.llvm_abi_type_of_types(types, *element)?;
                    if self.codegen.target_data.get_store_size(&element_ty) == 0 {
                        continue;
                    }
                    fields.push(element_ty);
                }
                Ok(self.codegen.context.struct_type(&fields, false).into())
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                let layout = self.physical_enum_layout_for_option(types, *inner)?;
                self.llvm_enum_value_type_from_layout(layout)
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if nominal.fqn == "scoop.unsafe.__AtomicInt" {
                    return Ok(self
                        .codegen
                        .int_type(IntTy {
                            bits: self.codegen.host.word_bit_width(),
                            signed: true,
                        })
                        .into());
                }
                if nominal.fqn == "scoop.core.UIntPtr" || nominal.fqn == "scoop.unsafe.FunPtr" {
                    return Ok(self
                        .codegen
                        .int_type(IntTy {
                            bits: self.codegen.host.word_bit_width(),
                            signed: false,
                        })
                        .into());
                }
                if let Some(layout) = self.physical_enum_layout_for_nominal_opt(types, nominal) {
                    return self.llvm_enum_value_type_from_layout(layout);
                }
                if let Some(codegen_ty) = self.equivalent_codegen_type_id_from_types(types, ty) {
                    let cg_ty = self
                        .codegen
                        .try_cg_ty_of_type_id(codegen_ty)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "LLVM ABI materialization 无法为 `{}` 恢复 codegen 类型",
                                types.display(ty)
                            ))
                        })?;
                    return self.codegen.llvm_basic_type_of(dummy_span(), cg_ty);
                }
                self.llvm_nominal_value_type_from_layout(types, nominal)
            }
            TypeKind::Param(_) => Err(frontend_error(format!(
                "LLVM ABI materialization 遇到尚未实例化的类型参数 `{}`（t{}）",
                types.display(ty),
                ty.as_u32()
            ))),
        }
    }

    pub(super) fn llvm_nominal_value_type_from_layout(
        &mut self,
        types: &TypeStore,
        nominal: &crate::ty::NominalType,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        let layout = self.physical_enum_layout_for_nominal(types, nominal)?;
        self.llvm_enum_value_type_from_layout(layout)
    }

    pub(super) fn llvm_enum_value_type_from_layout(
        &self,
        layout: &scoopc_lir_facts::LirEnumLayoutFacts,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        match &layout.repr {
            scoopc_lir_facts::LirEnumReprFacts::TaggedUnion => {
                if let Some(existing) = self.codegen.context.get_struct_type(&layout.fqn) {
                    return Ok(existing.into());
                }
                let enum_ty = self.codegen.context.opaque_struct_type(&layout.fqn);
                let tag_ty = self.codegen.context.i32_type();
                let payload_word_ty = self.codegen.int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: false,
                });
                let payload_ptr_ty = self.codegen.llvm_gc_i8_ptr_type();
                enum_ty.set_body(
                    &[tag_ty.into(), payload_word_ty.into(), payload_ptr_ty.into()],
                    false,
                );
                Ok(enum_ty.into())
            }
            scoopc_lir_facts::LirEnumReprFacts::ValueOnly { underlying_ty_fqn } => {
                self.llvm_builtin_integer_from_fqn(underlying_ty_fqn.as_deref())
            }
        }
    }

    pub(super) fn llvm_builtin_integer_from_fqn(
        &self,
        underlying_ty_fqn: Option<&str>,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        let fqn = underlying_ty_fqn.ok_or_else(|| {
            frontend_error(
                "LLVM ABI materialization 缺少 value-only enum 的底层整数类型".to_string(),
            )
        })?;
        let int_ty = match fqn {
            "scoop.core.Int" | "scoop.unsafe.__AtomicInt" => IntTy {
                bits: self.codegen.host.word_bit_width(),
                signed: true,
            },
            "scoop.core.UInt" | "scoop.core.UIntPtr" => IntTy {
                bits: self.codegen.host.word_bit_width(),
                signed: false,
            },
            other => {
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                {
                    IntTy { bits, signed: true }
                } else if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                {
                    IntTy {
                        bits,
                        signed: false,
                    }
                } else {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 目前只支持 integer-backed value-only enum，实际底层类型为 `{other}`"
                    )));
                }
            }
        };
        Ok(self.codegen.int_type(int_ty).into())
    }

    pub(super) fn equivalent_codegen_type_id_from_types(
        &self,
        types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = types.display(source_ty).to_string();
        self.codegen
            .types
            .iter_ids()
            .find(|&candidate| self.codegen.types.display(candidate).to_string() == source_display)
    }

    pub(super) fn define_named_struct(
        &self,
        name: &str,
        fields: &[BasicTypeEnum<'ctx>],
    ) -> StructType<'ctx> {
        let struct_ty = self
            .codegen
            .context
            .get_struct_type(name)
            .unwrap_or_else(|| self.codegen.context.opaque_struct_type(name));
        if struct_ty.is_opaque() {
            struct_ty.set_body(fields, false);
        }
        struct_ty
    }

    pub(super) fn define_union_storage_type(
        &self,
        name: &str,
        payload_tys: &[StructType<'ctx>],
    ) -> StructType<'ctx> {
        let storage_ty = self
            .codegen
            .context
            .get_struct_type(name)
            .unwrap_or_else(|| self.codegen.context.opaque_struct_type(name));
        if !storage_ty.is_opaque() {
            return storage_ty;
        }

        let mut max_size = 0u64;
        let mut max_align = 1u64;
        let mut anchor_ty = None;
        for payload_ty in payload_tys {
            let size = self.codegen.target_data.get_store_size(payload_ty);
            let align = u64::from(self.codegen.target_data.get_abi_alignment(payload_ty));
            if anchor_ty.is_none() || align > max_align || (align == max_align && size > max_size) {
                anchor_ty = Some(*payload_ty);
                max_size = size;
                max_align = align;
            } else if size > max_size {
                max_size = size;
            }
        }

        if max_size == 0 {
            storage_ty.set_body(&[], false);
            return storage_ty;
        }

        let _anchor_ty = anchor_ty.expect("payload_tys 至少包含 Complete variant");
        let unit_size = if max_align > 8 {
            16
        } else if max_align > 4 {
            8
        } else if max_align > 2 {
            4
        } else if max_align > 1 {
            2
        } else {
            1
        };
        let unit_count = max_size.div_ceil(unit_size) as u32;
        let storage_field: BasicTypeEnum<'ctx> = match unit_size {
            16 => self
                .codegen
                .context
                .i128_type()
                .array_type(unit_count)
                .into(),
            8 => self
                .codegen
                .context
                .i64_type()
                .array_type(unit_count)
                .into(),
            4 => self
                .codegen
                .context
                .i32_type()
                .array_type(unit_count)
                .into(),
            2 => self
                .codegen
                .context
                .i16_type()
                .array_type(unit_count)
                .into(),
            _ => self.codegen.context.i8_type().array_type(unit_count).into(),
        };
        let fields: Vec<BasicTypeEnum<'ctx>> = vec![storage_field];
        storage_ty.set_body(&fields, false);
        storage_ty
    }

    pub(super) fn ensure_declared_compiler_private_helper_function(
        &self,
        name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) {
        let _ =
            self.codegen
                .declare_compiler_private_helper_function(name, fn_ty, Linkage::Internal);
    }

    pub(super) fn ensure_struct_anchor(&self, name: &str, struct_ty: StructType<'ctx>) {
        if self.codegen.module.get_global(name).is_some() {
            return;
        }
        let global = self.codegen.module.add_global(struct_ty, None, name);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
        global.set_initializer(&struct_ty.const_zero());
    }

    pub(super) fn ensure_case_tag_constant(&self, name: &str, tag_value: u32) {
        if self.codegen.module.get_global(name).is_some() {
            return;
        }
        let i32_ty = self.codegen.context.i32_type();
        let global = self.codegen.module.add_global(i32_ty, None, name);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
        global.set_initializer(&i32_ty.const_int(u64::from(tag_value), false));
    }
}
