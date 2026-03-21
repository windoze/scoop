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
use crate::ty::layout::{EnumLayout, EnumRepr, EnumTagType, EnumVariantLayout, NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{BuiltinTypes, NominalType, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::lower::{TypeLowerError, TypeLowering};
use super::TypeEnv;

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
    MissingEnumDecl {
        fqn: String,
    },

    #[error("enum 布局计算缺少源文件：{path}")]
    #[diagnostic(code(scoop::typecheck::missing_enum_decl_source_for_layout))]
    MissingEnumDeclSource {
        path: String,
    },

    #[error("enum 布局计算缺少文件上下文（package/import）：{path}")]
    #[diagnostic(code(scoop::typecheck::missing_enum_decl_file_ctx_for_layout))]
    MissingEnumDeclFileContext {
        path: String,
    },

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
            TypeKind::Param(_) => {
                // 类型参数的实际 kind/布局需要在 monomorphization 后才能确定。
                // 当前阶段用“指针大小的 opaque layout”占位，避免提前把 layout 语义耦合进推断系统。
                self.pointer_layout().without_niche()
            }
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => self.bool_layout(),
                ValueTypeKind::Int | ValueTypeKind::UInt => self.word_layout(),
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = ((bits as u64) + 7) / 8;
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
        // - 引用类型在语言层语义为 non-null（nullable 走 `Option<T>`），因此 null 可作为 niche；
        // - 同时，按指针对齐（至少 2），诸如 0x1 的 misaligned 值也可作为“非法地址 niche”。
        //
        // 这里用 `[0, pointer_align)` 作为“可用 niche 值集合”（连续分配），以便支持 nested niche。
        TypeLayout::new(self.target.pointer_size, self.target.pointer_align).with_niche(NicheDomain {
            storage: NicheStorage::Pointer,
            next: 0,
            end: self.target.pointer_align.max(1),
        })
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
        if let Some(mut domain) = inner_layout.niche {
            if let Some(none_value) = domain.take_one() {
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
        }

        // 2) tagged union fallback。
        let tag = EnumTagType::for_variant_count(2);
        let tag_layout = TypeLayout::new(tag.size(), tag.align());
        let payload = inner_layout.without_niche();

        let payload_offset = align_to(tag_layout.size, payload.align);
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
                variants: vec![
                    EnumVariantLayout {
                        name: "Some".to_string(),
                        boxed: false,
                        payload,
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
        if matches!(self.types.kind(id), TypeKind::Value(ValueTypeKind::Option(_))) {
            let _ = self.type_layout(id)?;
            return Ok(self.enum_cache.get(&id).expect("Option layout must be cached"));
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

        let decl_source = self.env.source(&decl.decl_file).ok_or_else(|| {
            LayoutError::MissingEnumDeclSource {
                path: decl.decl_file.display().to_string(),
            }
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
            variants.push(EnumVariantLayout {
                name: v.name.clone(),
                boxed: false,
                payload,
            });
        }

        // boxing：当某个 variant 明显大于其它 variant 时，把该 variant 的 payload 自动装箱为指针。
        let (max_size, second_size) = largest_two_sizes(&variants);
        let inline_threshold = self.target.pointer_size.saturating_mul(ENUM_BOX_INLINE_THRESHOLD_WORDS);
        let disparity = if second_size == 0 {
            max_size >= inline_threshold
        } else {
            max_size >= inline_threshold && max_size >= second_size.saturating_mul(ENUM_BOX_DISPARITY_RATIO)
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

        let tag = EnumTagType::for_variant_count(decl.variants.len());
        let payload = union_payload_layout(&variants);

        let tag_layout = TypeLayout::new(tag.size(), tag.align());
        let payload_offset = align_to(tag_layout.size, payload.align);
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

    fn enum_type_id(&mut self, nominal: &NominalType) -> TypeId {
        self.types
            .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal.clone())))
    }

    fn option_type_id(&mut self, inner: TypeId) -> TypeId {
        self.types.ty_option(inner)
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

fn union_payload_layout(variants: &[EnumVariantLayout]) -> TypeLayout {
    let mut size = 0u64;
    let mut align = 1u64;
    for v in variants {
        size = size.max(v.payload.size);
        align = align.max(v.payload.align);
    }
    TypeLayout::new(size, align)
}
