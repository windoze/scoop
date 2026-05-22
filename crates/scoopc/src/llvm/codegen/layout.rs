//! LLVM codegen：type/layout lowering（niche/boxing/field GEP 等）。
//!
//! 该模块专注于“布局相关”的逻辑：
//! - `TypeId -> TypeLayout`（仅用于 niche/boxing 等决策）
//! - enum/Option 的表示选择（tagged union / niche / value-only）与 boxing 启发式
//! - class/struct/tuple 的字段索引与 field GEP helper

use std::collections::{HashMap, HashSet};

use inkwell::values::PointerValue;

use crate::hir;
use crate::ty::layout::{NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{MonoTypeId, TypeId, TypeKind, ValueTypeKind};

use super::ty::CodegenMonoInput;
use super::types::{
    CgEnumLayout, CgEnumRepr, CgEnumVariant, CgTy, ENUM_BOX_DISPARITY_RATIO,
    ENUM_BOX_INLINE_THRESHOLD_WORDS, IntTy,
};
use super::{LlvmEmitError, MainCodegen, align_to, largest_two_sizes};

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn class_init_layout(
        &mut self,
        at: crate::span::Span,
        class_key: &hir::ClassInstanceKey,
    ) -> Result<hir::MonoClassInit, LlvmEmitError> {
        let mut visiting: HashSet<hir::ClassInstanceKey> = HashSet::new();
        self.class_init_layout_inner(at, class_key, &mut visiting)
    }

    fn class_init_layout_inner(
        &mut self,
        _at: crate::span::Span,
        class_key: &hir::ClassInstanceKey,
        visiting: &mut HashSet<hir::ClassInstanceKey>,
    ) -> Result<hir::MonoClassInit, LlvmEmitError> {
        if let Some(cached) = self
            .shared_caches
            .class_init_layout_cache
            .borrow()
            .get(class_key)
            .cloned()
        {
            return Ok(cached);
        }

        if !visiting.insert(class_key.clone()) {
            std::panic::panic_any(
                "typecheck must reject class inheritance cycles before LLVM layout",
            );
        }

        let base = self
            .class_inits
            .get(class_key)
            .cloned()
            .expect("ClassInit must exist for layout key");

        let mut fields: Vec<hir::ClassField<MonoTypeId>> = Vec::new();
        let mut field_indices: HashMap<String, u32> = HashMap::new();

        if let Some(super_fqn) = base.super_class_fqn.as_deref() {
            let super_key = self.registered_class_instance_key(super_fqn).ok_or_else(|| {
                LlvmEmitError::Frontend {
                    message: format!(
                        "class layout `{class_key}` references superclass `{super_fqn}` without ClassInstanceKey metadata"
                    ),
                }
            })?;
            let super_layout = self.class_init_layout_inner(_at, &super_key, visiting)?;
            fields.extend(super_layout.fields);
            field_indices.extend(super_layout.field_indices);
        }

        for field in base.fields {
            let idx = fields.len() as u32;
            field_indices.insert(field.fqn.clone(), idx);
            fields.push(field);
        }

        let layouted = hir::MonoClassInit {
            fqn: base.fqn,
            source_path: base.source_path,
            super_class_fqn: base.super_class_fqn,
            super_ctor_args_span: base.super_ctor_args_span,
            super_ctor_call: base.super_ctor_call,
            super_ctor_args: base.super_ctor_args,
            this_id: base.this_id,
            fields,
            field_indices,
            steps: base.steps,
            ctors: base.ctors,
        };

        let _ = visiting.remove(class_key);
        self.shared_caches
            .class_init_layout_cache
            .borrow_mut()
            .insert(class_key.clone(), layouted.clone());
        Ok(layouted)
    }

    /// 若 `field_fqn` 指向一个 class 的实例字段，则返回该字段的布局/类型信息。
    ///
    /// 返回值：
    /// - `class`：对应 class 的初始化信息（字段列表/初始化步骤）
    /// - `field_idx`：字段在 payload struct 中的稳定索引
    /// - `field_cg`：字段的 codegen 类型（用于 load/store）
    pub(super) fn lookup_class_field_by_fqn(
        &mut self,
        field_fqn: &str,
        at: crate::span::Span,
        // T0125：receiver 的 TypeId，用于泛型 class 的 mangled FQN 查找。
        receiver_ty: Option<TypeId>,
    ) -> Result<Option<(hir::MonoClassInit, u32, CgTy)>, LlvmEmitError> {
        let Some((owner_fqn, _name)) = field_fqn.rsplit_once('.') else {
            return Ok(None);
        };

        // T0125：若 receiver 类型携带 type args，优先用 typed key 查找具体实例化的 ClassInit。
        let lookup_key = receiver_ty.and_then(|recv_ty| self.class_key_from_receiver(recv_ty));

        if let Some(lookup_key) = lookup_key
            && self.class_inits.contains_key(&lookup_key)
        {
            return self.lookup_class_field_by_fqn_inner(field_fqn, at, &lookup_key);
        }

        if let Some(owner_key) = self.registered_class_instance_key(owner_fqn) {
            return self.lookup_class_field_by_fqn_inner(field_fqn, at, &owner_key);
        }

        // 无 receiver 类型时只扫描已注册 key，不再在 codegen 中从裸字符串构造 layout key。
        let mangled_owner_prefix = format!("{owner_fqn}<");
        let class_keys = self.class_inits.keys().cloned().collect::<Vec<_>>();
        for class_key in class_keys {
            if class_key.as_str() != owner_fqn
                && !class_key.as_str().starts_with(&mangled_owner_prefix)
            {
                continue;
            }
            if let Some(field) = self.lookup_class_field_by_fqn_inner(field_fqn, at, &class_key)? {
                return Ok(Some(field));
            }
        }
        Ok(None)
    }

    fn lookup_class_field_by_fqn_inner(
        &mut self,
        field_fqn: &str,
        at: crate::span::Span,
        class_key: &hir::ClassInstanceKey,
    ) -> Result<Option<(hir::MonoClassInit, u32, CgTy)>, LlvmEmitError> {
        let class = self.class_init_layout(at, class_key)?;
        let field_idx = class.field_indices.get(field_fqn).copied().or_else(|| {
            let (_, field_name) = field_fqn.rsplit_once('.')?;
            class
                .fields
                .iter()
                .position(|field| field.name == field_name || field.fqn == field_fqn)
                .map(|idx| idx as u32)
        });
        let Some(field_idx) = field_idx else {
            return Ok(None);
        };
        let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
            panic!("lookup_class_field_by_fqn_inner: verifier accepted class field index drift")
        });
        let field_cg = self.cg_ty_of_type_id(field.ty.inner(), "class field layout lookup");
        Ok(Some((class, field_idx, field_cg)))
    }

    /// 从 receiver TypeId 提取已注册的 class instance key。
    fn class_key_from_receiver(&self, ty: TypeId) -> Option<hir::ClassInstanceKey> {
        let mono_ty = self.types.as_mono(ty).ok()?;
        hir::ClassInstanceKey::from_mono_nominal(self.types, mono_ty)
    }

    pub(in crate::llvm::codegen) fn registered_class_instance_key(
        &self,
        class_fqn: &str,
    ) -> Option<hir::ClassInstanceKey> {
        self.class_inits
            .keys()
            .find(|key| key.as_str() == class_fqn)
            .cloned()
    }

    pub(super) fn lookup_struct_field<T: CodegenMonoInput>(
        &self,
        struct_ty: T,
        field_fqn: &str,
        _at: crate::span::Span,
    ) -> Result<(u32, CgTy), LlvmEmitError> {
        let struct_ty = self.mono_type_id(struct_ty, "struct field lookup");
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            std::panic::panic_any("struct layout lookup must receive a nominal value type");
        };

        // T0124：使用 mangled FQN 查找（支持泛型 struct 的具体实例化）。
        let key = self.nominal_layout_key_from_types(nominal, self.types);
        let layout = self
            .struct_layouts
            .get(&key)
            .expect("struct layout must exist before LLVM field lookup");

        let idx = layout
            .fields
            .iter()
            .position(|f| f.fqn == field_fqn)
            .unwrap_or_else(|| {
                // User-facing unknown field errors are owned by typecheck.
                unreachable!(
                    "typecheck must reject unknown struct fields before LLVM codegen: {field_fqn}"
                )
            });

        let field = &layout.fields[idx];
        let field_ty = self.cg_ty_of_layout_field(field.span, field.ty, field.ty_fqn.as_deref())?;

        // T0119: For `@CLayout(packed = N)` with N > 1, the LLVM struct has padding
        // elements inserted, so the logical field index differs from the LLVM element index.
        let llvm_idx = self
            .shared_caches
            .pack_field_indices
            .borrow()
            .get(&key)
            .map_or(idx as u32, |indices| indices[idx]);

        Ok((llvm_idx, field_ty))
    }

    pub(super) fn struct_clayout<T: CodegenMonoInput>(
        &self,
        struct_ty: T,
    ) -> Option<hir::StructCLayout> {
        let struct_ty = struct_ty.try_into_mono_type_id(self)?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            return None;
        };
        // T0124：使用 mangled FQN 查找。
        let key = self.nominal_layout_key_from_types(nominal, self.types);
        self.struct_layouts
            .get(&key)
            .and_then(|layout| layout.c_layout)
    }

    /// 计算 class 对象中某个字段的地址。
    ///
    /// 约定：
    /// - `obj_ptr` 指向对象头（即 runtime `scoop_alloc` 的返回值，`ScoopGcObjectHeader*` 起始地址）；
    /// - 对象布局在 LLVM 侧表示为 `{ ScoopGcObjectHeader, ClassPayload }`；
    /// - 字段位于 `ClassPayload` 内部，索引由 `hir::MonoClassInit.fields` 的稳定顺序决定。
    pub(super) fn codegen_class_field_ptr(
        &mut self,
        at: crate::span::Span,
        class: &hir::MonoClassInit,
        obj_ptr: PointerValue<'ctx>,
        field_idx: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if field_idx as usize >= class.fields.len() {
            panic!("codegen_class_field_ptr: verifier accepted class field index out of bounds");
        }

        let obj_ty = self.llvm_class_object_type(at, class)?;
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let typed_obj = self
            .builder
            .build_pointer_cast(obj_ptr, obj_ptr_ty, "class_obj_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "class_payload_gep")?;

        let payload_ty = self.llvm_class_payload_type(at, class)?;
        let field_ptr =
            self.builder
                .build_struct_gep(payload_ty, payload_ptr, field_idx, "class_field_gep")?;
        Ok(field_ptr)
    }

    pub(super) fn lookup_tuple_element<T: CodegenMonoInput>(
        &self,
        tuple_ty: T,
        elem_idx: u32,
        _at: crate::span::Span,
    ) -> Result<CgTy, LlvmEmitError> {
        let tuple_ty = self.mono_type_id(tuple_ty, "tuple element lookup");
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty.inner())
        else {
            panic!("lookup_tuple_element: verifier accepted non-tuple TypeId for tuple lookup");
        };

        let elem_ty = elements
            .get(elem_idx as usize)
            .copied()
            .unwrap_or_else(|| panic!("lookup_tuple_element: verifier accepted tuple index drift"));

        Ok(self.cg_ty_of_type_id(elem_ty, "tuple element lookup"))
    }

    pub(super) fn target_layout(&self) -> TargetLayout {
        // 说明：与 typecheck::layout.rs 一致，当前阶段用 host pointer size/align 作为 layout。
        TargetLayout::host()
    }

    pub(super) fn type_layout(&mut self, ty: TypeId) -> TypeLayout {
        if let Some(layout) = self
            .shared_caches
            .type_layout_cache
            .borrow()
            .get(&ty)
            .copied()
        {
            return layout;
        }

        let target = self.target_layout();
        let Some(mono_ty) = self.try_mono_type_id(ty) else {
            return TypeLayout::new(target.pointer_size, target.pointer_align);
        };
        let kind = self.types.kind(mono_ty.inner()).clone();

        let layout = match kind {
            TypeKind::Ref(_) => TypeLayout::new(target.pointer_size, target.pointer_align)
                .with_niche(NicheDomain {
                    storage: NicheStorage::Pointer,
                    next: 0,
                    end: 1,
                }),
            TypeKind::StarProjection(star) => self.type_layout(star.read_ty),
            TypeKind::Param(_) => TypeLayout::new(target.pointer_size, target.pointer_align),
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => TypeLayout::new(1, 1).with_niche(NicheDomain {
                    storage: NicheStorage::U8,
                    next: 2,
                    end: 256,
                }),
                ValueTypeKind::Char => TypeLayout::new(4, 4),
                ValueTypeKind::Float64 => TypeLayout::new(8, 8),
                ValueTypeKind::Float32 => TypeLayout::new(4, 4),
                ValueTypeKind::Int | ValueTypeKind::UInt => {
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = u64::from(bits).div_ceil(8);
                    let align = size.clamp(1, target.pointer_align.max(1));
                    TypeLayout::new(size, align)
                }
                ValueTypeKind::Tuple(elements) => {
                    self.aggregate_fields_layout_for_type_ids(&elements)
                }
                ValueTypeKind::Option(inner) => self.option_type_layout(ty, inner),
                ValueTypeKind::Nominal(_) => {
                    // 当前 codegen 只在 niche/boxing 决策里需要 layout 信息；nominal struct/enum 的精确布局
                    // 将在对应任务里补齐。这里按“opaque word-sized”兜底，避免过度耦合。
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
            },
        };

        self.shared_caches
            .type_layout_cache
            .borrow_mut()
            .insert(ty, layout);
        layout
    }

    fn option_type_layout(&mut self, option_ty: TypeId, inner: TypeId) -> TypeLayout {
        // 注意：该函数只负责“niche 传播”与 `None` 编码缓存（供后续 codegen 使用）。
        if self
            .shared_caches
            .option_niche_cache
            .borrow()
            .contains_key(&option_ty)
        {
            return self
                .shared_caches
                .type_layout_cache
                .borrow()
                .get(&option_ty)
                .copied()
                .unwrap_or(TypeLayout::new(
                    self.target_layout().pointer_size,
                    self.target_layout().pointer_align,
                ));
        }

        let target = self.target_layout();
        let inner_layout = self.type_layout(inner);

        // niche path：inner 提供可用 niche domain。
        if let Some(mut domain) = inner_layout.niche
            && let Some(none_value) = domain.take_one()
        {
            let storage = domain.storage;

            // 关键约束（GC-FIX C2b）：
            // - `addrspace(1)` 的 GC-managed 指针不允许用“非 NULL 小整数”做哨兵值；
            // - 否则 `Option<Option<Ref>>` 会把 `None` 编码成 1/2/...，进入 stackmap roots 后无法区分，
            //   进而在精确 GC 下造成误追踪或崩溃。
            //
            // 因此：Pointer niche 只允许使用一次（`None == NULL`），并禁止把剩余 niche domain 继续向外层传播。
            if storage == NicheStorage::Pointer {
                domain.next = domain.end;
            }

            self.shared_caches
                .option_niche_cache
                .borrow_mut()
                .insert(option_ty, Some((storage, none_value)));

            let layout = TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain);
            self.shared_caches
                .type_layout_cache
                .borrow_mut()
                .insert(option_ty, layout);
            return layout;
        }

        // tagged union fallback：不携带 niche。
        self.shared_caches
            .option_niche_cache
            .borrow_mut()
            .insert(option_ty, None);

        // 说明：当前 codegen 的 enum 表示仍采用 `{ tag: i32, payload: word }`，因此这里返回一个
        // “足够大”的布局即可；精确大小与 tag type 选择后续任务再统一。
        let tag_size = 4u64;
        let tag_align = 4u64;
        let payload_size = target.pointer_size;
        let payload_align = target.pointer_align;
        let payload_offset = align_to(tag_size, payload_align);
        let align = payload_align.max(tag_align);
        let size = align_to(payload_offset + payload_size, align);
        let layout = TypeLayout::new(size, align);
        self.shared_caches
            .type_layout_cache
            .borrow_mut()
            .insert(option_ty, layout);
        layout
    }

    fn aggregate_fields_layout_for_type_ids(&mut self, fields: &[TypeId]) -> TypeLayout {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let l = self.type_layout(field);
            size = align_to(size, l.align);
            size = size.saturating_add(l.size);
            align = align.max(l.align);
        }
        size = align_to(size, align);
        TypeLayout::new(size, align)
    }

    pub(super) fn cg_enum_layout<T: CodegenMonoInput>(
        &mut self,
        at: crate::span::Span,
        enum_ty: T,
    ) -> Result<CgEnumLayout, LlvmEmitError> {
        let enum_ty = self.mono_type_id(enum_ty, "enum layout lookup");
        let cache_key = enum_ty.inner();
        if !self
            .shared_caches
            .enum_cg_layout_cache
            .borrow()
            .contains_key(&cache_key)
        {
            let computed = self.compute_cg_enum_layout(at, enum_ty)?;
            self.shared_caches
                .enum_cg_layout_cache
                .borrow_mut()
                .insert(cache_key, computed);
        }
        Ok(self
            .shared_caches
            .enum_cg_layout_cache
            .borrow()
            .get(&cache_key)
            .cloned()
            .expect("just inserted"))
    }

    fn compute_cg_enum_layout(
        &mut self,
        at: crate::span::Span,
        enum_ty: MonoTypeId,
    ) -> Result<CgEnumLayout, LlvmEmitError> {
        let kind = self.types.kind(enum_ty.inner()).clone();
        match kind {
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                // 确保 option niche 缓存已被填充（用于 nested niche）。
                let _ = self.type_layout(enum_ty.inner());
                let repr = match self
                    .shared_caches
                    .option_niche_cache
                    .borrow()
                    .get(&enum_ty.inner())
                    .copied()
                    .flatten()
                {
                    Some((storage, none_value)) => CgEnumRepr::Niche {
                        storage,
                        none_value,
                    },
                    None => CgEnumRepr::TaggedUnion,
                };

                let inner_cg = self.cg_ty_of_type_id(inner, "Option<T> enum layout");
                let some_boxed = self.enum_variant_requires_boxing(at, &[inner_cg])?;

                Ok(CgEnumLayout {
                    repr,
                    variants: vec![
                        CgEnumVariant {
                            name: "Some".to_string(),
                            tag: 0,
                            boxed: some_boxed,
                            fields: vec![inner_cg],
                        },
                        CgEnumVariant {
                            name: "None".to_string(),
                            tag: 1,
                            boxed: false,
                            fields: Vec::new(),
                        },
                    ],
                })
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                let enum_key = self.nominal_layout_key_from_types(&nominal, self.types);
                let hir_layout = self.enum_layouts.get(&enum_key).unwrap_or_else(|| {
                    panic!("compute_cg_enum_layout: verifier accepted enum without layout metadata")
                });

                let mut repr = CgEnumRepr::TaggedUnion;
                if let hir::EnumRepr::ValueOnly { underlying_ty_fqn } = &hir_layout.repr {
                    let Some(underlying_ty_fqn) = underlying_ty_fqn.as_deref() else {
                        std::panic::panic_any(
                            "typecheck must publish value-only enum underlying types before LLVM layout",
                        );
                    };

                    let underlying_cg = self.cg_ty_of_type_fqn(at, Some(underlying_ty_fqn))?;
                    let CgTy::Int(underlying) = underlying_cg else {
                        std::panic::panic_any(
                            "typecheck must restrict value-only enum underlying types to integers",
                        );
                    };

                    repr = CgEnumRepr::ValueOnly { underlying };
                }

                let mut variants: Vec<CgEnumVariant> =
                    Vec::with_capacity(hir_layout.variants.len());
                let mut payload_layouts: Vec<TypeLayout> =
                    Vec::with_capacity(hir_layout.variants.len());
                for v in &hir_layout.variants {
                    let mut fields = Vec::with_capacity(v.fields.len());
                    for f in &v.fields {
                        let cg = self.cg_ty_of_layout_field(f.span, f.ty, f.ty_fqn.as_deref())?;
                        fields.push(cg);
                    }
                    // 当前阶段 inline tagged union payload 只支持：
                    // - 单字段标量 / 单字段 GC ref；
                    // - 仍保持 niche 表示的 builtin `Option<T>`。
                    //
                    // 多字段 payload，以及单字段但字段本身是 tuple / struct /
                    // 非 niche nested enum 的 aggregate value，都需要走 boxed variant 主线。
                    let boxed = !matches!(repr, CgEnumRepr::ValueOnly { .. })
                        && self.enum_variant_requires_boxing(at, &fields)?;
                    variants.push(CgEnumVariant {
                        name: v.name.clone(),
                        tag: v.tag,
                        boxed,
                        fields,
                    });

                    // value-only enum 的 ABI/layout 由底层整型决定：不做 payload/boxing 决策。
                    if !matches!(repr, CgEnumRepr::ValueOnly { .. }) {
                        payload_layouts.push(self.aggregate_fields_layout_for_cg_tys(
                            &variants.last().expect("just pushed").fields,
                        )?);
                    }
                }

                // boxing：复用 typecheck 的启发式规则（ratio + inline threshold）。
                if !matches!(repr, CgEnumRepr::ValueOnly { .. }) {
                    let target = self.target_layout();
                    let (max_size, second_size) = largest_two_sizes(&payload_layouts);
                    let inline_threshold = target
                        .pointer_size
                        .saturating_mul(ENUM_BOX_INLINE_THRESHOLD_WORDS);
                    let disparity = if second_size == 0 {
                        max_size >= inline_threshold
                    } else {
                        max_size >= inline_threshold
                            && max_size >= second_size.saturating_mul(ENUM_BOX_DISPARITY_RATIO)
                    };

                    if disparity {
                        for (v, payload) in variants.iter_mut().zip(payload_layouts.iter()) {
                            if payload.size == max_size && max_size > target.pointer_size {
                                v.boxed = true;
                            }
                        }
                    }
                }

                Ok(CgEnumLayout { repr, variants })
            }
            _ => std::panic::panic_any("enum layout lookup must receive an enum value type"),
        }
    }

    fn aggregate_fields_layout_for_cg_tys(
        &self,
        fields: &[CgTy],
    ) -> Result<TypeLayout, LlvmEmitError> {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let field_layout = self.cg_ty_layout(field)?;
            size = align_to(size, field_layout.align);
            size = size.saturating_add(field_layout.size);
            align = align.max(field_layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn enum_variant_requires_boxing(
        &mut self,
        at: crate::span::Span,
        fields: &[CgTy],
    ) -> Result<bool, LlvmEmitError> {
        if fields.len() > 1 {
            return Ok(true);
        }
        if let [field_ty] = fields {
            return self.enum_field_requires_boxing(at, *field_ty);
        }
        Ok(false)
    }

    fn enum_field_requires_boxing(
        &mut self,
        at: crate::span::Span,
        field_ty: CgTy,
    ) -> Result<bool, LlvmEmitError> {
        match field_ty {
            CgTy::Int(int_ty) if int_ty.bits > self.host.word_bit_width() => Ok(true),
            CgTy::Tuple(_) | CgTy::Struct(_) => Ok(true),
            // inline nested enum 目前只继续保留 niche path；
            // 其余 nested enum（含 nominal/value-only/tagged-union，以及 tagged-union `Option<T>`）
            // 一律进入 boxed payload 主线，避免落到 `{payload_word, payload_ptr}` 的错误旁路。
            CgTy::Enum(enum_ty) => match self.types.kind(enum_ty.inner()) {
                TypeKind::Value(ValueTypeKind::Option(_)) => Ok(!matches!(
                    self.cg_enum_layout(at, enum_ty)?.repr,
                    CgEnumRepr::Niche { .. }
                )),
                _ => Ok(true),
            },
            _ => Ok(false),
        }
    }

    fn cg_ty_layout(&self, ty: CgTy) -> Result<TypeLayout, LlvmEmitError> {
        let target = self.target_layout();
        Ok(match ty {
            CgTy::Unit | CgTy::Never => TypeLayout::new(0, 1),
            // 当前阶段 Bool 在 LLVM 中用 i1 表示，但 layout/lint/niche 计算按“存储为 u8”建模。
            CgTy::Bool => TypeLayout::new(1, 1),
            CgTy::Float64 => TypeLayout::new(8, 8),
            CgTy::Float32 => TypeLayout::new(4, 4),
            CgTy::Int(int_ty) => {
                let size = u64::from(int_ty.bits).div_ceil(8);
                let align = size.clamp(1, target.pointer_align.max(1));
                TypeLayout::new(size, align)
            }
            CgTy::String => TypeLayout::new(target.pointer_size, target.pointer_align),
            CgTy::Ref => TypeLayout::new(target.pointer_size, target.pointer_align),
            // 兜底：composite 在当前阶段按 word-sized opaque 处理，避免错误放大。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                TypeLayout::new(target.pointer_size, target.pointer_align)
            }
        })
    }

    pub(super) fn enum_payload_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        }
    }
}
