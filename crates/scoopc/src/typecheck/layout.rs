//! 类型布局计算（best-effort）与 rich enum 表示选择（T0449）。
//!
//! 当前阶段（仍处于前端 typecheck）我们做两件事：
//! 1. 为后续 codegen 固定“布局选择规则”（niche / boxing / tag type）；
//! 2. 在 `scoop test` 运行时通过 `tracing` 记录 size disparity lint（warning）。
//!
//! 注意：
//! - 这里的布局计算是“最小可用子集”，并不追求覆盖所有类型语法；
//! - 目前没有真正的 target machine（T0803），因此使用 host pointer size/align。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;
use tracing::{debug, warn};

use crate::ast;
use crate::resolve::Index;
use crate::ty::layout::{
    EnumLayout, EnumRepr, EnumTagType, EnumVariantLayout, NicheDomain, NicheStorage, TargetLayout,
    TypeLayout,
};
use crate::ty::{BuiltinTypes, NominalType, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::lower::{TypeLowerError, TypeLowering};
use super::{TypeEnv, TypeSymbolKind};

// boxing / lint 的启发式阈值（spec §2.3.2 未给出精确数值，先在实现侧固定）。
const ENUM_BOX_DISPARITY_RATIO: u64 = 4;
const ENUM_BOX_INLINE_THRESHOLD_WORDS: u64 = 16;

#[derive(Debug, Error, Diagnostic)]
pub enum LayoutError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error("enum 布局计算缺少声明：{fqn}")]
    #[diagnostic(code(scoop::typecheck::missing_enum_decl_for_layout))]
    MissingEnumDecl { fqn: String },

    #[error("enum 布局计算缺少源文件：{path}")]
    #[diagnostic(code(scoop::typecheck::missing_enum_decl_source_for_layout))]
    MissingEnumDeclSource { path: String },

    #[error("enum 布局计算缺少文件上下文（package/import）：{path}")]
    #[diagnostic(code(scoop::typecheck::missing_enum_decl_file_ctx_for_layout))]
    MissingEnumDeclFileContext { path: String },

    #[error("enum 泛型实例化参数数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::typecheck::enum_layout_type_arg_arity_mismatch))]
    EnumLayoutTypeArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
    },
}

/// 对当前 `TypeStore` 中出现过的类型做一次布局/元数据计算。
///
/// 说明：
/// - 该函数不参与 typecheck 语义判定（不会改变 pass/fail），但会：
///   - 固定 `EnumLayout`/`TypeLayout` 的计算规则（供后续 codegen 复用）
///   - emit size disparity lint（warning）
pub fn check_file_type_layouts(
    index: &Index,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), LayoutError> {
    let mut cx = LayoutComputer::new(index, env, types, builtins);

    // 计算所有已出现类型的 layout，以确保：
    // - `Option<RefType>` niche 选择能在 fixtures 中被覆盖
    // - enum boxing/lint 能在 typecheck 阶段被触发（通过 warn! 记录）
    let ids: Vec<TypeId> = cx.types.iter_ids().collect();
    for id in ids {
        let _ = cx.type_layout(id)?;
    }

    Ok(())
}

struct LayoutComputer<'a> {
    index: &'a Index,
    env: &'a TypeEnv,
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    target: TargetLayout,
    cache: HashMap<TypeId, TypeLayout>,
    enum_cache: HashMap<TypeId, EnumLayout>,
    in_progress: HashSet<TypeId>,
}

impl<'a> LayoutComputer<'a> {
    fn new(
        index: &'a Index,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        Self {
            index,
            env,
            types,
            builtins,
            target: TargetLayout::host(),
            cache: HashMap::new(),
            enum_cache: HashMap::new(),
            in_progress: HashSet::new(),
        }
    }

    fn type_layout(&mut self, id: TypeId) -> Result<TypeLayout, LayoutError> {
        if let Some(layout) = self.cache.get(&id).copied() {
            return Ok(layout);
        }

        // 防御性：避免递归 value type 布局计算导致的无限递归。
        if !self.in_progress.insert(id) {
            debug!(
                ty = %self.types.display(id),
                "detected recursive type layout request; fallback to pointer layout"
            );
            let layout = self.pointer_layout().without_niche();
            self.cache.insert(id, layout);
            return Ok(layout);
        }

        let kind = self.types.kind(id).clone();
        let layout = match kind {
            TypeKind::Ref(_) => self.pointer_layout(),
            TypeKind::StarProjection(star) => self.type_layout(star.read_ty)?,
            TypeKind::Param(_) => {
                // 类型参数的实际 kind/布局需要在 monomorphization 后才能确定。
                // 当前阶段用“指针大小的 opaque layout”占位，避免提前把 layout 语义耦合进推断系统。
                self.pointer_layout().without_niche()
            }
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => self.bool_layout(),
                ValueTypeKind::Char => TypeLayout::new(4, 4),
                ValueTypeKind::Float64 => TypeLayout::new(8, 8),
                ValueTypeKind::Float32 => TypeLayout::new(4, 4),
                ValueTypeKind::Int | ValueTypeKind::UInt => self.word_layout(),
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = (bits as u64).div_ceil(8);
                    let align = size.clamp(1, self.target.pointer_align.max(1));
                    TypeLayout::new(size, align)
                }
                ValueTypeKind::Tuple(elements) => self.tuple_layout(&elements)?,
                ValueTypeKind::Option(inner) => self.option_layout(inner)?,
                ValueTypeKind::Nominal(nominal) => self.nominal_layout(&nominal)?,
            },
        };

        self.in_progress.remove(&id);
        self.cache.insert(id, layout);
        Ok(layout)
    }

    fn pointer_layout(&self) -> TypeLayout {
        // 说明：
        // - 引用类型在语言层语义为 non-null（nullable 走 `Option<T>`），因此 `0`（NULL）可作为 niche；
        // - GC trace safety（spec §2.3.2；T1518）：
        //   对于 GC-managed ref，**禁止**用 `0x1` 等非 NULL 的 pointer-shaped 值编码 `None`。
        //   这类值一旦落入“静态可枚举”的 GC pointer slot（stackmap/bitmap），精确 GC 会把它当作对象指针追踪并崩溃/UB。
        //
        // 因此：Pointer niche 只提供一个值（NULL），并避免产生可向外层传播的“剩余 domain”
        // （从而 `Option<Option<Ref>>` 外层会回退到 tagged union 表示）。
        TypeLayout::new(self.target.pointer_size, self.target.pointer_align).with_niche(
            NicheDomain {
                storage: NicheStorage::Pointer,
                next: 0,
                end: 1,
            },
        )
    }

    fn word_layout(&self) -> TypeLayout {
        TypeLayout::new(self.target.pointer_size, self.target.pointer_align)
    }

    fn bool_layout(&self) -> TypeLayout {
        // spec §2.3.2：`Option<Bool>` uses value `2` for `None`（因此 bool 必须是小整数存储）。
        TypeLayout::new(1, 1).with_niche(NicheDomain {
            storage: NicheStorage::U8,
            next: 2,
            end: 256,
        })
    }

    fn tuple_layout(&mut self, elements: &[TypeId]) -> Result<TypeLayout, LayoutError> {
        self.aggregate_fields_layout(elements)
    }

    fn option_layout(&mut self, inner: TypeId) -> Result<TypeLayout, LayoutError> {
        // 注意：这里不直接复用 sysroot 的 `Option<T>` enum 声明，
        // 因为内部类型表示把 `Option<T>` 作为 builtin value type special-case。
        let inner_layout = self.type_layout(inner)?;

        // 1) niche path：inner 提供可用 niche 值。
        if let Some(mut domain) = inner_layout.niche
            && let Some(none_value) = domain.take_one()
        {
            debug!(
                inner = %self.types.display(inner),
                storage = ?domain.storage,
                none_value,
                "Option<T> uses niche optimization"
            );

            let layout = TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain);
            let option_id = self.option_type_id(inner);
            self.enum_cache.insert(
                option_id,
                EnumLayout {
                    repr: EnumRepr::Niche {
                        storage: domain.storage,
                        none_value,
                    },
                    layout,
                    tag_offset: 0,
                    payload_offset: 0,
                    payload: layout,
                    gc_ref_word_mask: Vec::new(),
                    ref_payload_offset: 0,
                    ref_payload: TypeLayout::new(0, 1),
                    variants: vec![
                        EnumVariantLayout {
                            name: "Some".to_string(),
                            boxed: false,
                            payload: inner_layout.without_niche(),
                        },
                        EnumVariantLayout {
                            name: "None".to_string(),
                            boxed: false,
                            payload: TypeLayout::new(0, 1),
                        },
                    ],
                },
            );
            return Ok(layout);
        }

        // 2) tagged union fallback。
        let tag = EnumTagType::for_variant_count(2);
        let tag_layout = TypeLayout::new(tag.size(), tag.align());
        // GC safety（T1515/T1516）：
        // - tagged union 的运行期表示固定包含一个“GC pointer slot”，用于承载 `Some(ref)` 或 boxed payload；
        // - 该 slot 必须在任意时刻只包含 `NULL` 或有效 GC object 指针；
        // - 因此这里把 payload 建模为 `{ word_payload, ref_payload }` 的组合布局（而不是 inner 的 union overlay）。
        let payload_word = self.word_layout().without_niche();
        let ref_payload = self.pointer_layout().without_niche();
        let ref_payload_rel_off = align_to(payload_word.size, ref_payload.align);
        let payload_align = payload_word.align.max(ref_payload.align);
        let payload_size = align_to(ref_payload_rel_off + ref_payload.size, payload_align);
        let payload = TypeLayout::new(payload_size, payload_align);

        let payload_offset = align_to(tag_layout.size, payload.align);
        let ref_payload_offset = payload_offset + ref_payload_rel_off;
        let align = payload.align.max(tag_layout.align);
        let size = align_to(payload_offset + payload.size, align);
        let layout = TypeLayout::new(size, align);

        let option_id = self.option_type_id(inner);
        self.enum_cache.insert(
            option_id,
            EnumLayout {
                repr: EnumRepr::TaggedUnion { tag },
                layout,
                tag_offset: 0,
                payload_offset,
                payload,
                gc_ref_word_mask: self.gc_ref_word_mask_for_ref_slot(ref_payload_offset),
                ref_payload_offset,
                ref_payload,
                variants: vec![
                    EnumVariantLayout {
                        name: "Some".to_string(),
                        boxed: false,
                        payload: inner_layout.without_niche(),
                    },
                    EnumVariantLayout {
                        name: "None".to_string(),
                        boxed: false,
                        payload: TypeLayout::new(0, 1),
                    },
                ],
            },
        );

        Ok(layout)
    }

    fn nominal_layout(&mut self, nominal: &NominalType) -> Result<TypeLayout, LayoutError> {
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            // 理论上不会发生：TypeLowering 已确保 nominal type 一定来自 TypeEnv。
            debug!(fqn = %nominal.fqn, "missing nominal type symbol in TypeEnv; fallback to pointer");
            return Ok(self.pointer_layout().without_niche());
        };

        match sym.kind {
            super::TypeSymbolKind::TypeAlias => Ok(self.pointer_layout().without_niche()),
            super::TypeSymbolKind::Nominal(kind) => match kind {
                ast::TypeKind::Enum => {
                    let id = self.enum_type_id(nominal);
                    let layout = self.enum_layout(id)?.layout;
                    Ok(layout)
                }
                ast::TypeKind::Struct => {
                    // struct 的精确布局/对齐规则会在对应任务中完善（PLAN §4.2 / codegen §8.2）。
                    // 当前阶段用“opaque word-sized”占位，保证 enum 布局计算可前进。
                    debug!(
                        ty = %nominal.fqn,
                        "struct layout is not fixed yet; fallback to word-sized opaque layout"
                    );
                    Ok(self.word_layout())
                }
                ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => {
                    Ok(self.pointer_layout())
                }
            },
        }
    }

    fn enum_layout(&mut self, id: TypeId) -> Result<&EnumLayout, LayoutError> {
        if self.enum_cache.contains_key(&id) {
            return Ok(self.enum_cache.get(&id).expect("just checked"));
        }

        // `Option<T>` 的 enum metadata 已在 `option_layout` 内写入。
        if matches!(
            self.types.kind(id),
            TypeKind::Value(ValueTypeKind::Option(_))
        ) {
            let _ = self.type_layout(id)?;
            return Ok(self
                .enum_cache
                .get(&id)
                .expect("Option layout must be cached"));
        }

        let nominal = match self.types.kind(id) {
            TypeKind::Value(ValueTypeKind::Nominal(n)) => n.clone(),
            other => {
                debug!(ty = %self.types.display(id), kind = ?other, "non-enum type queried for enum_layout");
                // 兜底：给一个最小 tagged union metadata，避免 panic。
                let layout = self.type_layout(id)?;
                self.enum_cache.insert(
                    id,
                    EnumLayout {
                        repr: EnumRepr::TaggedUnion {
                            tag: EnumTagType::U8,
                        },
                        layout,
                        tag_offset: 0,
                        payload_offset: 0,
                        payload: layout,
                        gc_ref_word_mask: Vec::new(),
                        ref_payload_offset: 0,
                        ref_payload: TypeLayout::new(0, 1),
                        variants: Vec::new(),
                    },
                );
                return Ok(self.enum_cache.get(&id).expect("inserted"));
            }
        };

        let Some(decl) = self.env.enum_decl(&nominal.fqn) else {
            return Err(LayoutError::MissingEnumDecl {
                fqn: nominal.fqn.clone(),
            });
        };

        if decl.type_params.len() != nominal.args.len() {
            return Err(LayoutError::EnumLayoutTypeArgArityMismatch {
                fqn: nominal.fqn.clone(),
                expected: decl.type_params.len(),
                found: nominal.args.len(),
            });
        }

        let decl_source =
            self.env
                .source(&decl.decl_file)
                .ok_or_else(|| LayoutError::MissingEnumDeclSource {
                    path: decl.decl_file.display().to_string(),
                })?;
        let decl_ctx = self.env.file_type_context(&decl.decl_file).ok_or_else(|| {
            LayoutError::MissingEnumDeclFileContext {
                path: decl.decl_file.display().to_string(),
            }
        })?;

        // 在 “enum 声明处文件”的上下文中把 variant 字段类型做 lowering。
        //
        // 注意：`TypeLowering` 会可变借用 `TypeStore`；因此我们需要先收集完所有 lowered 字段类型，
        // 再 drop `lower`，随后才能递归调用 `self.type_layout(...)` 计算 payload layout。
        let mut lowered_variant_fields: Vec<Vec<TypeId>> = Vec::with_capacity(decl.variants.len());
        {
            let mut lower = TypeLowering::new_with_ctx(
                decl_source,
                self.index,
                self.env,
                self.types,
                self.builtins,
                decl_ctx.pkg_prefix.clone(),
                decl_ctx.imports.clone(),
            );

            lower.push_type_param_bindings(
                decl.type_params
                    .iter()
                    .cloned()
                    .zip(nominal.args.iter().copied()),
            );

            for v in &decl.variants {
                let mut fields = Vec::with_capacity(v.fields.len());
                for f in &v.fields {
                    fields.push(lower.lower_type_ref(&f.ty)?);
                }
                lowered_variant_fields.push(fields);
            }

            lower.pop_type_param_bindings();
        }

        let mut variants: Vec<EnumVariantLayout> = Vec::with_capacity(decl.variants.len());
        for (v, fields) in decl.variants.iter().zip(lowered_variant_fields.iter()) {
            let payload = self.aggregate_fields_layout(fields)?;
            // 当前阶段 inline tagged union payload 只承载“单字段标量 / 单字段 GC ref”。
            // 因此以下 payload 需要提前进入 boxed 主线：
            // - 多字段 variant；
            // - 单字段但字段本身是 tuple / struct 的 aggregate value。
            //
            // 注意：这里仍保留 “raw payload size/align” 以便 size disparity lint 能触发；
            // 之后会在统一的 boxing pass 中把 boxed variants 的 payload layout 收敛为 word-sized。
            let boxed = self.enum_variant_requires_boxing(fields);
            variants.push(EnumVariantLayout {
                name: v.name.clone(),
                boxed,
                payload,
            });
        }

        // boxing：当某个 variant 明显大于其它 variant 时，把该 variant 的 payload 自动装箱为指针。
        let (max_size, second_size) = largest_two_sizes(&variants);
        let inline_threshold = self
            .target
            .pointer_size
            .saturating_mul(ENUM_BOX_INLINE_THRESHOLD_WORDS);
        let disparity = if second_size == 0 {
            max_size >= inline_threshold
        } else {
            max_size >= inline_threshold
                && max_size >= second_size.saturating_mul(ENUM_BOX_DISPARITY_RATIO)
        };

        if disparity {
            let boxed_payload = self.word_layout();
            let mut boxed_names = Vec::new();
            for v in variants.iter_mut() {
                if v.payload.size == max_size && max_size > boxed_payload.size {
                    v.boxed = true;
                    v.payload = boxed_payload;
                    boxed_names.push(v.name.clone());
                }
            }

            if !boxed_names.is_empty() {
                warn!(
                    enum_fqn = %nominal.fqn,
                    max_size,
                    second_size,
                    boxed = %boxed_names.join(", "),
                    "enum variant size disparity is significant; boxing oversized variant(s)"
                );
            }
        }

        // boxed variants 的运行期表示为“指针大小 payload”（由 codegen 侧把 payload 指针写入 ref slot）。
        //
        // 说明：上面的 disparity pass 仅对“最大 payload”做 boxed 标记与警告；但多字段 variant
        // 也会被提前标记为 boxed，因此需要在这里统一收敛其 payload layout，避免把 raw payload size
        // 误认为 enum 的 inline payload。
        let boxed_payload = self.word_layout();
        for v in variants.iter_mut() {
            if v.boxed {
                v.payload = boxed_payload;
            }
        }

        let tag = EnumTagType::for_variant_count(decl.variants.len());
        // GC safety（T1515/T1516）：
        // - rich enum 的 tagged union payload 固定包含一个 GC pointer slot（ref_payload）；
        // - non-ref 数据不得覆盖该 slot，未使用时必须为 0；
        // - variant 的多字段 payload 需要通过 boxing 进入 heap object，以保持 slot 可静态枚举。
        let payload_word = self.word_layout().without_niche();
        let ref_payload = self.pointer_layout().without_niche();
        let ref_payload_rel_off = align_to(payload_word.size, ref_payload.align);
        let payload_align = payload_word.align.max(ref_payload.align);
        let payload_size = align_to(ref_payload_rel_off + ref_payload.size, payload_align);
        let payload = TypeLayout::new(payload_size, payload_align);

        let tag_layout = TypeLayout::new(tag.size(), tag.align());
        let payload_offset = align_to(tag_layout.size, payload.align);
        let ref_payload_offset = payload_offset + ref_payload_rel_off;
        let align = payload.align.max(tag_layout.align);
        let size = align_to(payload_offset + payload.size, align);
        let layout = TypeLayout::new(size, align);

        self.enum_cache.insert(
            id,
            EnumLayout {
                repr: EnumRepr::TaggedUnion { tag },
                layout,
                tag_offset: 0,
                payload_offset,
                payload,
                gc_ref_word_mask: self.gc_ref_word_mask_for_ref_slot(ref_payload_offset),
                ref_payload_offset,
                ref_payload,
                variants,
            },
        );

        Ok(self.enum_cache.get(&id).expect("inserted"))
    }

    fn aggregate_fields_layout(&mut self, fields: &[TypeId]) -> Result<TypeLayout, LayoutError> {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let field_layout = self.type_layout(field)?;
            size = align_to(size, field_layout.align);
            size = size.saturating_add(field_layout.size);
            align = align.max(field_layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn enum_variant_requires_boxing(&self, fields: &[TypeId]) -> bool {
        fields.len() > 1
            || matches!(
                fields,
                [field_ty] if self.enum_field_requires_boxing(*field_ty)
            )
    }

    fn enum_field_requires_boxing(&self, ty: TypeId) -> bool {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Tuple(_)) => true,
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => matches!(
                self.env.type_symbol(&nominal.fqn).map(|sym| sym.kind),
                Some(TypeSymbolKind::Nominal(ast::TypeKind::Struct))
            ),
            _ => false,
        }
    }

    fn enum_type_id(&mut self, nominal: &NominalType) -> TypeId {
        self.types
            .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal.clone())))
    }

    fn option_type_id(&mut self, inner: TypeId) -> TypeId {
        self.types.ty_option(inner)
    }

    fn gc_ref_word_mask_for_ref_slot(&self, ref_payload_offset: u64) -> Vec<u64> {
        let word = self.target.pointer_size.max(1);
        if !ref_payload_offset.is_multiple_of(word) {
            return Vec::new();
        }
        let idx = ref_payload_offset / word;
        let len = (idx / 64) + 1;
        let mut words = vec![0u64; len as usize];
        let wi = (idx / 64) as usize;
        let bit = (idx % 64) as u32;
        words[wi] |= 1u64 << bit;
        words
    }
}

trait WithoutNiche {
    fn without_niche(self) -> Self;
}

impl WithoutNiche for TypeLayout {
    fn without_niche(mut self) -> Self {
        self.niche = None;
        self
    }
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn largest_two_sizes(variants: &[EnumVariantLayout]) -> (u64, u64) {
    let mut max = 0u64;
    let mut second = 0u64;
    for v in variants {
        let s = v.payload.size;
        if s >= max {
            second = max;
            max = s;
            continue;
        }
        if s > second {
            second = s;
        }
    }
    (max, second)
}
