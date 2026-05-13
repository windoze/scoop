//! LLVM codegen：`TypeId -> CgTy/LLVM type` lowering。
//!
//! 该模块承载“类型相关”的转换入口：
//! - `cg_ty_of` / `cg_ty_of_type_fqn`
//! - `llvm_basic_type_of` 以及 struct/tuple/enum/class 的 LLVM type 生成

use inkwell::types::BasicTypeEnum;
use inkwell::types::StructType;
use inkwell::values::GlobalValue;

use crate::hir;
use crate::stable_id::{CanonicalTextKey, PrivateSymbolMangler, canonical_record};
use crate::ty::{NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::types::{CgEnumRepr, CgEnumVariant, CgTy, IntTy};
use super::{LlvmEmitError, MainCodegen, TypeDescriptorSpec};

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn enum_boxed_payload_key(
        &self,
        enum_ty: TypeId,
        variant_name: &str,
        context: &str,
    ) -> Result<CanonicalTextKey, LlvmEmitError> {
        Ok(CanonicalTextKey::new(canonical_record(
            "enum_boxed_payload",
            [
                self.canonical_type_key_text_for_codegen(enum_ty, context)?,
                variant_name.to_string(),
            ],
        )))
    }

    pub(super) fn codegen_type_store_for_type_id(&self, ty: TypeId) -> Option<&TypeStore> {
        if (ty.as_u32() as usize) < self.types.len() {
            return Some(self.types);
        }
        self.materialized_pass_view()
            .map(|view| &view.materialized().types)
            .filter(|types| (ty.as_u32() as usize) < types.len())
    }

    pub(super) fn builtin_nominal_cg_ty(&self, fqn: &str) -> Option<CgTy> {
        match fqn {
            "scoop.core.Bool" => Some(CgTy::Bool),
            "scoop.core.Char" => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            "scoop.core.Float64" | "scoop.core.Double" => Some(CgTy::Float64),
            "scoop.core.Float32" => Some(CgTy::Float32),
            "scoop.core.Int" => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.core.UInt" | "scoop.core.UIntPtr" => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            "scoop.core.Int8" => Some(CgTy::Int(IntTy {
                bits: 8,
                signed: true,
            })),
            "scoop.core.Int16" | "scoop.core.Short" => Some(CgTy::Int(IntTy {
                bits: 16,
                signed: true,
            })),
            "scoop.core.Int32" => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: true,
            })),
            "scoop.core.Int64" | "scoop.core.Long" => Some(CgTy::Int(IntTy {
                bits: 64,
                signed: true,
            })),
            "scoop.core.UInt8" | "scoop.core.Byte" => Some(CgTy::Int(IntTy {
                bits: 8,
                signed: false,
            })),
            "scoop.core.UInt16" | "scoop.core.UShort" => Some(CgTy::Int(IntTy {
                bits: 16,
                signed: false,
            })),
            "scoop.core.UInt32" => Some(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            "scoop.core.UInt64" | "scoop.core.ULong" => Some(CgTy::Int(IntTy {
                bits: 64,
                signed: false,
            })),
            _ => None,
        }
    }

    /// 返回名义类型在 struct_layouts/enum_layouts 中的查找 key（T0124）。
    ///
    /// 对于无 type args 的类型返回 base FQN；对于参数化类型返回 mangled FQN。
    pub(super) fn nominal_layout_key_from_types(
        &self,
        nominal: &NominalType,
        types: &TypeStore,
    ) -> String {
        crate::hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types)
    }

    pub(super) fn nominal_layout_key(&self, nominal: &NominalType) -> String {
        self.nominal_layout_key_from_types(nominal, self.types)
    }

    fn enum_layout_key(
        &self,
        at: crate::span::Span,
        enum_ty: TypeId,
        unsupported_kind: &'static str,
    ) -> Result<String, LlvmEmitError> {
        let types = self.codegen_type_store_for_type_id(enum_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: unsupported_kind,
                at: at.into(),
            },
        )?;
        match types.kind(enum_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => Ok(crate::hir::mangle_nominal_fqn(
                "scoop.core.Option",
                &[*inner],
                types,
            )),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                Ok(self.nominal_layout_key_from_types(nominal, types))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: unsupported_kind,
                at: at.into(),
            }),
        }
    }

    pub(super) fn cg_ty_of(&self, ty: TypeId) -> Option<CgTy> {
        let types = self.codegen_type_store_for_type_id(ty)?;
        match types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::StarProjection(star) => self.cg_ty_of(star.read_ty),
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
            TypeKind::Value(ValueTypeKind::Option(_)) => Some(CgTy::Enum(ty)),
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Some(CgTy::Tuple(ty)),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if let Some(cg_ty) = self.builtin_nominal_cg_ty(&nominal.fqn) {
                    return Some(cg_ty);
                }
                // T1027：internal atomics（`__AtomicInt`）——值类型、与底层整数相同布局。
                //
                // 说明：
                // - typecheck 内部会为 typealias 保留一个名义 `TypeId`（便于诊断/审计），
                //   但后端必须把它映射到与 `Int` 完全一致的 ABI（word-sized, signed）。
                if nominal.fqn == "scoop.unsafe.__AtomicInt" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: true,
                    }));
                }
                // `UIntPtr`（typealias）：在 early stage 直接落到 word-sized unsigned int。
                if nominal.fqn == "scoop.core.UIntPtr" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: false,
                    }));
                }
                // T1026：`FunPtr<F>` —— 运行期表示为 word-sized address（unsigned），并作为 opaque handle 传递。
                if nominal.fqn == "scoop.unsafe.FunPtr" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: false,
                    }));
                }
                // T0124：使用 mangled FQN 查找（支持泛型 struct/enum 的具体实例化）。
                let key = self.nominal_layout_key_from_types(nominal, types);
                if self.struct_layouts.contains_key(&key) {
                    return Some(CgTy::Struct(ty));
                }
                if self.enum_layouts.contains_key(&key) {
                    return Some(CgTy::Enum(ty));
                }
                None
            }
            // T0125：monomorphization 后，TypeKind::Param 不应出现在 codegen 阶段。
            // 若仍出现，说明 monomorph 遗漏了替换——返回 None 并由调用方报告诊断。
            TypeKind::Param(p) => {
                tracing::warn!(
                    "cg_ty_of: TypeKind::Param({}) encountered in codegen (monomorph miss)",
                    p.name
                );
                None
            }
        }
    }

    /// Check if a return type is a GC-free aggregate (struct/tuple/enum containing no
    /// GC reference fields). LLVM's `gc.result` cannot lower aggregate types that span
    /// multiple physical registers during statepoint lowering, so functions returning
    /// such types must not have `gc "statepoint-example"`.
    pub(super) fn returns_gc_free_aggregate(&self, return_ty: TypeId) -> bool {
        let Some(cg) = self.cg_ty_of(return_ty) else {
            return false;
        };
        match cg {
            CgTy::Struct(type_id) => self.struct_type_is_gc_free(type_id),
            CgTy::Tuple(type_id) => self.tuple_type_is_gc_free(type_id),
            CgTy::Enum(type_id) => self.enum_type_is_gc_free(type_id),
            _ => false,
        }
    }

    /// Check if a struct type contains no GC references (String/Ref) in any of its fields.
    fn struct_type_is_gc_free(&self, ty: TypeId) -> bool {
        let Some(types) = self.codegen_type_store_for_type_id(ty) else {
            return false;
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty) else {
            return false;
        };
        let key = self.nominal_layout_key_from_types(nominal, types);
        let Some(layout) = self.struct_layouts.get(&key) else {
            return false;
        };
        layout.fields.iter().all(|f| {
            f.ty_fqn
                .as_deref()
                .is_some_and(|fqn| self.type_fqn_is_gc_free(fqn))
        })
    }

    /// Check if a tuple type contains no GC references in any of its elements.
    fn tuple_type_is_gc_free(&self, ty: TypeId) -> bool {
        let Some(types) = self.codegen_type_store_for_type_id(ty) else {
            return false;
        };
        let TypeKind::Value(ValueTypeKind::Tuple(elems)) = types.kind(ty) else {
            return false;
        };
        elems.iter().all(|elem_ty| {
            self.cg_ty_of(*elem_ty)
                .is_some_and(|cg| !matches!(cg, CgTy::String | CgTy::Ref))
        })
    }

    /// Check if an enum type contains no GC references in any variant's fields.
    fn enum_type_is_gc_free(&self, ty: TypeId) -> bool {
        let Some(types) = self.codegen_type_store_for_type_id(ty) else {
            return false;
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty) else {
            return false;
        };
        let key = self.nominal_layout_key_from_types(nominal, types);
        let Some(layout) = self.enum_layouts.get(&key) else {
            return false;
        };
        layout.variants.iter().all(|v| {
            v.fields.iter().all(|f| {
                f.ty_fqn
                    .as_deref()
                    .is_some_and(|fqn| self.type_fqn_is_gc_free(fqn))
            })
        })
    }

    /// Check if a type FQN refers to a GC-free type (value type with no reference fields).
    fn type_fqn_is_gc_free(&self, fqn: &str) -> bool {
        match fqn {
            "scoop.core.Unit"
            | "scoop.core.Bool"
            | "scoop.core.Float64"
            | "scoop.core.Float32"
            | "scoop.core.Int"
            | "scoop.core.UInt"
            | "scoop.core.UIntPtr"
            | "scoop.unsafe.__AtomicInt" => true,
            "scoop.core.String" | "scoop.core.Any" => false,
            other => {
                // Fixed-width integer types: Int8, Int16, Int32, Int64, UInt8, etc.
                if let Some(suffix) = other.strip_prefix("scoop.core.Int")
                    && suffix.parse::<u32>().is_ok()
                {
                    return true;
                }
                if let Some(suffix) = other.strip_prefix("scoop.core.UInt")
                    && suffix.parse::<u32>().is_ok()
                {
                    return true;
                }
                // Nested struct: check its fields recursively.
                if let Some(layout) = self.struct_layouts.get(other) {
                    return layout.fields.iter().all(|f| {
                        f.ty_fqn
                            .as_deref()
                            .is_some_and(|inner| self.type_fqn_is_gc_free(inner))
                    });
                }
                // Nested enum: check its variant fields recursively.
                if let Some(layout) = self.enum_layouts.get(other) {
                    return layout.variants.iter().all(|v| {
                        v.fields.iter().all(|f| {
                            f.ty_fqn
                                .as_deref()
                                .is_some_and(|inner| self.type_fqn_is_gc_free(inner))
                        })
                    });
                }
                // Unknown type: assume not GC-free (conservative).
                false
            }
        }
    }

    pub(super) fn cg_ty_of_type_fqn(
        &self,
        at: crate::span::Span,
        ty_fqn: Option<&str>,
    ) -> Result<CgTy, LlvmEmitError> {
        let Some(ty_fqn) = ty_fqn else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct field type",
                at: at.into(),
            });
        };

        match ty_fqn {
            "scoop.core.Unit" => Ok(CgTy::Unit),
            "scoop.core.Bool" => Ok(CgTy::Bool),
            "scoop.core.Any" => Ok(CgTy::Ref),
            "scoop.core.String" => Ok(CgTy::String),
            "scoop.core.TypeKind" => Ok(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            "scoop.core.Char" => Ok(CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            })),
            "scoop.core.Float64" => Ok(CgTy::Float64),
            "scoop.core.Float32" => Ok(CgTy::Float32),
            "scoop.core.Int" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.unsafe.__AtomicInt" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.core.UInt" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            "scoop.core.UIntPtr" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            other => {
                // 固定位宽整数族（与 HIR lowering 的 special-case 规则对齐）。
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy { bits, signed: true }));
                }
                if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy {
                        bits,
                        signed: false,
                    }));
                }

                if self.class_inits.contains_key(other) || self.object_inits.contains_key(other) {
                    return Ok(CgTy::Ref);
                }
                if let Some((base, _)) = other.split_once('<')
                    && self.class_inits.contains_key(base)
                {
                    return Ok(CgTy::Ref);
                }

                // 用户定义的 nominal 值类型（struct/enum）：通过 TypeStore 反查 TypeId，再复用 `cg_ty_of`。
                //
                // 说明：
                // - `hir::StructFieldLayout.ty_fqn` 当前仅保存 “字段类型的 FQN”；
                // - 对于 `struct Wrap(val e: E)` 这类场景，需要能把 `E` 映射为 `CgTy::Enum`，
                //   以便在 LLVM struct type 中内嵌该字段，并支持后续的 field GEP/load/store。
                // - T0124：支持 mangled FQN（含 type args）的查找。
                if (self.struct_layouts.contains_key(other)
                    || self.enum_layouts.contains_key(other))
                    && let Some(ty) = self.types.iter_ids().find(|id| match self.types.kind(*id) {
                        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                            // T0124：使用 mangled FQN 比较（支持参数化类型如 “Pair<Int, String>”）。
                            let key = self.nominal_layout_key(nominal);
                            key == other
                        }
                        _ => false,
                    })
                    && let Some(cg) = self.cg_ty_of(ty)
                {
                    return Ok(cg);
                }

                Err(LlvmEmitError::Frontend {
                    message: format!("struct field type `{other}` is not lowerable"),
                })
            }
        }
    }

    pub(super) fn cg_ty_of_layout_field(
        &self,
        at: crate::span::Span,
        ty: Option<TypeId>,
        ty_fqn: Option<&str>,
    ) -> Result<CgTy, LlvmEmitError> {
        if let Some(ty) = ty
            && let Some(cg) = self.cg_ty_of(ty)
        {
            return Ok(cg);
        }
        if ty.is_none()
            && let Some(ty_fqn) = ty_fqn
            && let Ok(cg) = self.cg_ty_of_type_fqn(at, Some(ty_fqn))
        {
            return Ok(cg);
        }
        if let Some(ty) = ty {
            tracing::warn!(
                "cg_ty_of_layout_field: fallback to ty_fqn for layout field at {:?}; ty={}, ty_fqn={:?}",
                at,
                self.types.display(ty),
                ty_fqn
            );
        } else {
            tracing::warn!(
                "cg_ty_of_layout_field: missing TypeId for layout field at {:?}; ty_fqn={:?}",
                at,
                ty_fqn
            );
        }
        self.cg_ty_of_type_fqn(at, ty_fqn)
    }

    pub(super) fn llvm_basic_type_of(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        Ok(match ty {
            // 说明：Unit 没有运行期值；当前阶段仅用于”可放入 alloca”与保持 load/store 管线统一。
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Float64 => self.context.f64_type().into(),
            CgTy::Float32 => self.context.f32_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().into(),
            CgTy::Ref => self.llvm_gc_i8_ptr_type().into(),
            CgTy::Tuple(tuple_ty) => self.llvm_tuple_type(at, tuple_ty)?.into(),
            CgTy::Struct(struct_ty) => self.llvm_struct_type(at, struct_ty)?.into(),
            CgTy::Enum(enum_ty) => self.llvm_enum_value_type(at, enum_ty)?,
            // T1612: Nothing/Never 不应有运行期值；此处仅为不可达路径的 IR 连通提供占位类型。
            CgTy::Never => self.context.i8_type().into(),
        })
    }

    pub(super) fn llvm_struct_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let types =
            self.codegen_type_store_for_type_id(ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct type id",
                    at: at.into(),
                })?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        // T0124：使用 mangled FQN 查找（支持泛型 struct 的具体实例化）。
        let key = self.nominal_layout_key_from_types(nominal, types);
        let layout = self
            .struct_layouts
            .get(&key)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "struct layout",
                at: at.into(),
            })?;

        if let Some(existing) = self.context.get_struct_type(&layout.fqn) {
            // T0119: LLVM struct 可能已由更早的 lowering 路径创建，而共享 packed-field cache
            // 仍未为该类型补齐“逻辑字段索引 -> LLVM 元素索引”映射；此时需要按现有 LLVM type
            // 重新推导一次，再写回编译单元级共享 cache。
            let pack_value = layout.c_layout.as_ref().and_then(|c| c.packed);
            if let Some(n) = pack_value
                && n > 1
                && !self
                    .shared_caches
                    .pack_field_indices
                    .borrow()
                    .contains_key(&layout.fqn)
            {
                let mut user_fields: Vec<BasicTypeEnum<'ctx>> =
                    Vec::with_capacity(layout.fields.len());
                for field in &layout.fields {
                    let field_cg =
                        self.cg_ty_of_layout_field(field.span, field.ty, field.ty_fqn.as_deref())?;
                    user_fields.push(self.llvm_basic_type_of(field.span, field_cg)?);
                }

                let mut indices: Vec<u32> = Vec::new();
                let mut elem_idx: u32 = 0;
                let mut off: u64 = 0;
                for field_ty in &user_fields {
                    let natural_align = self.target_data.get_abi_alignment(field_ty) as u64;
                    let effective_align = std::cmp::min(natural_align, n as u64);
                    let aligned_offset = (off + effective_align - 1) & !(effective_align - 1);
                    if aligned_offset > off {
                        elem_idx += 1; // skip padding element
                    }
                    indices.push(elem_idx);
                    elem_idx += 1;
                    off = aligned_offset + self.target_data.get_store_size(field_ty);
                }

                self.shared_caches
                    .pack_field_indices
                    .borrow_mut()
                    .insert(layout.fqn.clone(), indices);
            }
            return Ok(existing);
        }

        let struct_ty = self.context.opaque_struct_type(&layout.fqn);

        let mut user_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(layout.fields.len());
        for field in &layout.fields {
            let field_cg =
                self.cg_ty_of_layout_field(field.span, field.ty, field.ty_fqn.as_deref())?;
            user_fields.push(self.llvm_basic_type_of(field.span, field_cg)?);
        }

        let pack_value = layout.c_layout.as_ref().and_then(|c| c.packed);
        match pack_value {
            Some(1) => {
                // packed = 1: LLVM native packed struct (all fields at align 1, no padding).
                struct_ty.set_body(&user_fields, true);
            }
            Some(n) if n > 1 => {
                // packed = N > 1: `#pragma pack(N)` semantics.
                // Each field's effective alignment = min(field_natural_align, N).
                // We use LLVM packed struct (is_packed=true) with explicit padding bytes
                // so that LLVM doesn't add its own padding rules.
                let mut packed_fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
                let mut field_element_indices: Vec<u32> = Vec::new();
                let mut offset: u64 = 0;

                for field_ty in &user_fields {
                    let natural_align = self.target_data.get_abi_alignment(field_ty) as u64;
                    let effective_align = std::cmp::min(natural_align, n as u64);

                    // Insert padding bytes to reach the next aligned offset.
                    let aligned_offset = (offset + effective_align - 1) & !(effective_align - 1);
                    let padding = aligned_offset - offset;
                    if padding > 0 {
                        let pad_ty = self.context.i8_type().array_type(padding as u32);
                        packed_fields.push(pad_ty.into());
                    }

                    field_element_indices.push(packed_fields.len() as u32);
                    packed_fields.push(*field_ty);

                    let field_size = self.target_data.get_store_size(field_ty);
                    offset = aligned_offset + field_size;
                }

                struct_ty.set_body(&packed_fields, true);
                self.shared_caches
                    .pack_field_indices
                    .borrow_mut()
                    .insert(layout.fqn.clone(), field_element_indices);
            }
            _ => {
                // No packing: normal LLVM struct layout.
                struct_ty.set_body(&user_fields, false);
            }
        }
        Ok(struct_ty)
    }

    /// 生成（或获取）某个 class 的 payload struct 类型：`{ field0, field1, ... }`。
    ///
    /// 说明：
    /// - payload 不包含对象头（header）；header 由 `llvm_class_object_type` 负责；
    /// - 当前阶段 fields 的顺序来自 `hir::ClassInit.fields`（stable order），用于可回归的字段索引；
    /// - 该类型名使用 runtime 命名空间前缀，避免与用户类型冲突。
    pub(super) fn llvm_class_payload_type(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let stable_key = self.stable_nominal_type_key(&class.fqn, "class_layout");
        let name =
            PrivateSymbolMangler.type_name("ClassPayload", "class_payload_type", &stable_key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            if existing.is_opaque() {
                let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> =
                    Vec::with_capacity(class.fields.len());
                for field in &class.fields {
                    let field_cg =
                        self.cg_ty_of(field.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "class field type",
                                at: at.into(),
                            })?;
                    llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
                }
                existing.set_body(&llvm_fields, false);
            }
            return Ok(existing);
        }

        let payload_ty = self.context.opaque_struct_type(&name);
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(class.fields.len());
        for field in &class.fields {
            let field_cg = self
                .cg_ty_of(field.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class field type",
                    at: at.into(),
                })?;
            llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
        }
        payload_ty.set_body(&llvm_fields, false);
        Ok(payload_ty)
    }

    pub(super) fn llvm_class_object_type(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let stable_key = self.stable_nominal_type_key(&class.fqn, "class_layout");
        let name = PrivateSymbolMangler.type_name("ClassObject", "class_object_type", &stable_key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            if existing.is_opaque() {
                let header_ty = self.llvm_gc_object_header_type();
                let payload_ty = self.llvm_class_payload_type(at, class)?;
                existing.set_body(&[header_ty.into(), payload_ty.into()], false);
            }
            return Ok(existing);
        }

        let obj_ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        let payload_ty = self.llvm_class_payload_type(at, class)?;
        obj_ty.set_body(&[header_ty.into(), payload_ty.into()], false);
        Ok(obj_ty)
    }

    pub(super) fn llvm_enum_value_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        // 注意：先从共享 cache 取出 enum layout，再抽取后续 lowering 真正需要的信息。
        let (repr, some_field) = {
            let cg_layout = self.cg_enum_layout(at, ty)?;
            let repr = cg_layout.repr;
            let some_field = cg_layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let types = self.codegen_type_store_for_type_id(ty).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "enum type id",
                        at: at.into(),
                    },
                )?;
                let fqn = match types.kind(ty) {
                    TypeKind::Value(ValueTypeKind::Option(_)) => "scoop.core.Option",
                    TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "enum type id",
                            at: at.into(),
                        });
                    }
                };

                if let Some(existing) = self.context.get_struct_type(fqn)
                    && !existing.is_opaque()
                {
                    return Ok(existing.into());
                }

                // 最小 rich enum 表示：`{ tag: i32, payload_word: iN, payload_ptr: i8 addrspace(1)* }`
                // - tag：按声明顺序分配的 variant id
                // - payload_word：承载整数/bool/boxed payload 指针（native ptr→word，非 GC 指针）
                // - payload_ptr：承载 GC-managed 指针 payload（避免 ptr<->int，供 statepoint/stackmap 识别 roots）
                let tag_ty = self.context.i32_type();
                let payload_word_ty = self.int_type(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                });
                let payload_ptr_ty = self.llvm_gc_i8_ptr_type();
                let enum_ty = self
                    .context
                    .get_struct_type(fqn)
                    .unwrap_or_else(|| self.context.opaque_struct_type(fqn));
                enum_ty.set_body(
                    &[tag_ty.into(), payload_word_ty.into(), payload_ptr_ty.into()],
                    false,
                );
                Ok(enum_ty.into())
            }
            CgEnumRepr::Niche { storage, .. } => match storage {
                crate::ty::layout::NicheStorage::Pointer => {
                    let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Option niche payload type",
                        at: at.into(),
                    })?;
                    Ok(self.llvm_basic_type_of(at, some_field)?)
                }
                crate::ty::layout::NicheStorage::U8 => Ok(self.context.i8_type().into()),
            },
            CgEnumRepr::ValueOnly { underlying } => Ok(self.int_type(underlying).into()),
        }
    }

    pub(super) fn llvm_tuple_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let (elements, use_primary_types) = {
            let tuple_types = self.codegen_type_store_for_type_id(ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple type id",
                    at: at.into(),
                },
            )?;
            let TypeKind::Value(ValueTypeKind::Tuple(elements)) = tuple_types.kind(ty) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple type id",
                    at: at.into(),
                });
            };
            (elements.clone(), std::ptr::eq(tuple_types, self.types))
        };

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(elements.len());
        for elem_ty in elements {
            let elem_cg = if use_primary_types {
                self.cg_ty_of(elem_ty)
            } else {
                let tuple_types = self
                    .materialized_pass_view()
                    .map(|view| &view.materialized().types)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: at.into(),
                    })?;
                self.cg_ty_of_mir_type(tuple_types, elem_ty)
            }
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element type",
                at: at.into(),
            })?;
            llvm_fields.push(self.llvm_basic_type_of(at, elem_cg)?);
        }

        Ok(self.context.struct_type(&llvm_fields, false))
    }

    pub(super) fn llvm_enum_boxed_payload_struct_type(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        // 说明：boxed payload 在运行期是一个独立的聚合对象；当前阶段用一个具名 LLVM struct 承载其字段布局，
        // 以便 ctor/binder 双方对齐类型（避免 bitcast 到不一致的匿名 struct）。
        let key = self.enum_boxed_payload_key(
            enum_ty,
            &variant.name,
            "enum boxed payload LLVM struct type",
        )?;
        let name = PrivateSymbolMangler.type_name(
            "EnumBoxedPayloadFields",
            "enum_boxed_payload_struct_type",
            &key,
        );

        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let payload_ty = self.context.opaque_struct_type(&name);
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(variant.fields.len());
        for &field_cg in &variant.fields {
            llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
        }
        payload_ty.set_body(&llvm_fields, false);
        Ok(payload_ty)
    }

    pub(super) fn llvm_enum_boxed_payload_object_type(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let key = self.enum_boxed_payload_key(
            enum_ty,
            &variant.name,
            "enum boxed payload LLVM object type",
        )?;
        let name = PrivateSymbolMangler.type_name(
            "EnumBoxedPayloadObject",
            "enum_boxed_payload_object_type",
            &key,
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let payload_struct_ty = self.llvm_enum_boxed_payload_struct_type(at, enum_ty, variant)?;
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), payload_struct_ty.into()], false);
        Ok(ty)
    }

    pub(super) fn get_or_create_enum_boxed_payload_type_desc_global(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
        payload_obj_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let enum_fqn = self.enum_layout_key(at, enum_ty, "enum boxed payload type desc")?;

        let key = self.enum_boxed_payload_key(
            enum_ty,
            &variant.name,
            "enum boxed payload type descriptor",
        )?;
        let global_name = PrivateSymbolMangler.mangle("enum_boxed_payload_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let canonical_name = format!(
            "scoop.runtime.EnumBoxedPayload__{}__{}",
            enum_fqn, variant.name
        );
        let trace_start_offset_bytes = self
            .target_data
            .offset_of_element(&payload_obj_ty, 1)
            .unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &canonical_name,
            obj_ty: payload_obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }
}
