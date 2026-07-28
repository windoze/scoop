//! typed HIR：typecheck 产出的、可独立消费的拥有型数据。
//!
//! [`TypedHir`] 持有 typecheck 阶段计算的全部「类型化」结果，供 `dump-hir` 与
//! 后续 lowering 消费。它**自包含**（不借用 resolve `Index` / `Interner`），
//! 因此可在 `run_typecheck` 返回后长期存活。
//!
//! 设计要点：
//!
//! - **拥有 `Interner` 副本**：`dump-hir` 渲染需要把 `Symbol` 解析为文本。我们克隆
//!   interner（深拷贝，但仅在 dump 路径发生一次），使 `TypedHir` 与原始 session
//!   解耦，避免悬垂借用。
//! - **拥有 `TypeStore`**：move 出 `TypeEnv`，所有 `TypeId` 句柄在新 store 中保持
//!   有效（store 本就是 `TypeId` 的唯一来源）。
//! - **声明级类型表快照**：顶层函数签名 / 成员函数签名 / 成员类型 / 构造器签名 /
//!   顶层 val 类型 / enum variant 列表，均为 `Symbol` 键的 `HashMap`，可廉价克隆。
//! - **per-file 表达式类型表**：每个用户文件一份 `expr_types: NodeIdTable<TypeId>`，
//!   在 typecheck body 时由 `ExprChecker` 写回。

pub mod facts;
pub mod render;

use std::collections::{HashMap, HashSet};

use scoop2_base::{FileId, Interner, NodeId, Symbol};

use crate::resolve::output::{NodeIdTable, ResolvedValue};
use crate::ty::{EffectRow, TypeId, TypeStore};

pub use facts::{
    EffectSite, PatternBinding, PatternBindingKey, PatternBindingSource, ResolvedCall,
    ResolvedMember, ResolvedPlace, SemanticFacts,
};

/// 一个顶层函数 / 成员函数 / 构造器的类型化签名快照（render 用）。
#[derive(Clone)]
pub struct TypedSignature {
    /// 渲染后的参数类型文本列表（按声明顺序）。
    pub param_types: Vec<TypeId>,
    /// 返回类型。
    pub return_ty: TypeId,
    /// 类型形参数量（>0 表示泛型）。
    pub type_param_count: usize,
    /// 参数名（与 param_types 平行）。
    pub param_names: Vec<Symbol>,
    /// 是否带默认值（与 param_types 平行）。
    pub has_defaults: Vec<bool>,
    /// effect 行（`/ Row`）；`Pure`（空行）若未声明。
    pub effect_row: EffectRow,
    /// 是否带 `vararg`。
    pub has_vararg: bool,
    /// 声明 span（定位源声明）。
    pub decl_span: scoop2_base::Span,
    /// 声明文件。
    pub decl_file: FileId,
}

/// 类型参数约束快照（导出 type_constraints 供 MIR 单态化用）。
#[derive(Clone, Debug)]
pub struct TypeConstraintsSnapshot {
    /// 类型参数名序列（按声明顺序）。
    pub type_params: Vec<Symbol>,
    /// where 约束（参数名, bound 文本）。
    pub constraints: Vec<(Symbol, crate::syntax::ast::GenericBound)>,
}

/// 一个文件的 typed 产物：源 AST 句柄 + per-NodeId 表达式类型表。
pub struct TypedFile {
    pub file_id: FileId,
    /// 该文件的包前缀（点分，可能为空）。
    pub package_prefix: String,
    /// 表达式 `NodeId → TypeId`（仅 User 文件；Sysroot 不做 body 类型检查）。
    pub expr_types: NodeIdTable<TypeId>,
    /// 语义事实侧表（调用决议 / 成员 / place / effect / value_refs）。
    pub facts: SemanticFacts,
}

/// typecheck 的完整产出：自包含的 typed HIR。
///
/// 由 [`crate::typecheck::run_typecheck`] 构造并返回。所有 `Symbol` 句柄通过内置
/// [`interner`](Self::interner) 解析；所有 `TypeId` 句柄通过内置
/// [`store`](Self::store) 查询。
pub struct TypedHir {
    /// 类型存储（从 `TypeEnv` move 出；所有 TypeId 的唯一来源）。
    pub store: TypeStore,
    /// interner 副本（解析 Symbol → 文本）。
    pub interner: Interner,
    /// 顶层函数 FQN → 签名重载集。
    pub top_level_funs: HashMap<Symbol, Vec<TypedSignature>>,
    /// 类型 FQN → (方法名 → 签名重载集)。成员函数 / 扩展。
    pub member_funs: HashMap<Symbol, HashMap<Symbol, Vec<TypedSignature>>>,
    /// 类型 FQN → (成员名 → 成员类型)。属性 / 字段。
    pub members: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    /// 类型 FQN → 次构造器签名重载集。
    pub ctor_signatures: HashMap<Symbol, Vec<TypedSignature>>,
    /// 顶层 val/var 简单名 → 类型。
    pub top_level_vals: HashMap<Symbol, TypeId>,
    /// enum FQN → variant 名列表。
    pub enum_variants: HashMap<Symbol, Vec<Symbol>>,
    /// 类型 FQN → 类型参数约束快照（导出 type_constraints 供 MIR 单态化用）。
    pub type_constraints: HashMap<Symbol, TypeConstraintsSnapshot>,
    /// 所有 interface 类型的 FQN 集合（MIR 用以区分 itable vs class vtable 分发）。
    pub interface_fqns: HashSet<Symbol>,
    /// 所有可被继承的 class FQN 集合（`open` 或 `abstract`）。
    /// 取补集即得"具体 class"（不可继承 → 虚方法可安全退化为直接调用）。
    /// MIR 去虚化 pass 据此判断 ref 类型接收者是否 final。
    pub extensible_class_fqns: HashSet<Symbol>,
    /// 超类型 → 直接子类型 FQN 列表（反转 index.supertypes）。
    /// 供 MIR 去虚化 pass 做 CHA（class hierarchy analysis）：
    /// 若某类型在此 map 中有子类型，则 receiver 可能是子类实例，不能简单去虚化。
    /// 若无子类型（不在 key 中），则 receiver 类型是精确的（exact），可去虚化。
    pub direct_subtypes: HashMap<Symbol, Vec<Symbol>>,
    /// 每个用户文件的 typed 产物（含 expr_types + 语义事实）。
    pub files: Vec<TypedFile>,
}

impl TypedHir {
    /// 查询某表达式的推断类型。
    pub fn expr_type(&self, file_id: FileId, node: NodeId) -> Option<TypeId> {
        self.files
            .iter()
            .find(|f| f.file_id == file_id)
            .and_then(|f| f.expr_types.get(node).copied())
    }

    /// 查找某 FileId 的 typed 文件产物。
    pub fn file(&self, file_id: FileId) -> Option<&TypedFile> {
        self.files.iter().find(|f| f.file_id == file_id)
    }

    /// 查找某 FileId 的 typed 文件产物（可变）。
    pub fn file_mut(&mut self, file_id: FileId) -> Option<&mut TypedFile> {
        self.files.iter_mut().find(|f| f.file_id == file_id)
    }

    /// 查询某调用表达式的决议结果。
    pub fn call_resolution(&self, file_id: FileId, node: NodeId) -> Option<&ResolvedCall> {
        self.file(file_id)
            .and_then(|f| f.facts.call_resolutions.get(node))
    }

    /// 查询某成员访问的决议结果。
    pub fn member_ref(&self, file_id: FileId, node: NodeId) -> Option<&ResolvedMember> {
        self.file(file_id)
            .and_then(|f| f.facts.member_refs.get(node))
    }

    /// 查询某赋值目标的 place 分类。
    pub fn assign_place(&self, file_id: FileId, node: NodeId) -> Option<&ResolvedPlace> {
        self.file(file_id)
            .and_then(|f| f.facts.assign_places.get(node))
    }

    /// 查询某值引用的解析结果。
    pub fn value_ref(&self, file_id: FileId, node: NodeId) -> Option<&ResolvedValue> {
        self.file(file_id)
            .and_then(|f| f.facts.value_refs.get(node))
    }

    /// 查询某模式节点引入的全部绑定（when arm / 解构 val）。
    pub fn pattern_bindings(
        &self,
        file_id: FileId,
        node: NodeId,
    ) -> Option<&[PatternBinding]> {
        self.file(file_id)
            .and_then(|f| f.facts.pattern_bindings.get(node).map(|v| v.as_slice()))
    }

    /// 查询某表达式的 actual effect row。
    pub fn expr_effect_row(
        &self,
        file_id: FileId,
        node: NodeId,
    ) -> Option<&crate::ty::EffectRow> {
        self.file(file_id)
            .and_then(|f| f.facts.expr_effect_rows.get(node))
    }

    /// 渲染为稳定缩进树文本。
    ///
    /// `files` 是与 typecheck 输入一致顺序的解析文件（`run_typecheck` 的 inputs）；
    /// 仅渲染 User 文件（FileId 与 [`TypedFile::file_id`] 对应）。
    pub fn render<'f>(
        &self,
        files: impl Iterator<Item = (FileId, &'f crate::syntax::ast::File)>,
    ) -> String {
        render::render_hir(self, files)
    }
}

impl TypedHir {
    /// 空 HIR（测试 / 无用户文件时用）。
    pub fn empty(interner: Interner) -> Self {
        Self {
            store: TypeStore::new(),
            interner,
            top_level_funs: HashMap::new(),
            member_funs: HashMap::new(),
            members: HashMap::new(),
            ctor_signatures: HashMap::new(),
            top_level_vals: HashMap::new(),
            enum_variants: HashMap::new(),
            type_constraints: HashMap::new(),
            interface_fqns: HashSet::new(),
            extensible_class_fqns: HashSet::new(),
            direct_subtypes: HashMap::new(),
            files: Vec::new(),
        }
    }
}
