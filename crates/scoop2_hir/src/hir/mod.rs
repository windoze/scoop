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
mod serde_impl;
pub mod tree;
pub mod type_info;

use std::collections::{HashMap, HashSet};

use scoop2_base::{FileId, Interner, NodeId, Symbol};

use crate::resolve::output::{NodeIdTable, ResolvedValue};
use crate::ty::{EffectRow, TypeId, TypeStore};

pub use facts::{
    EffectSite, PatternBinding, PatternBindingKey, PatternBindingSource, ResolvedCall,
    ResolvedCallArg, ResolvedMember, ResolvedPlace, SemanticFacts,
};

/// 一个顶层函数 / 成员函数 / 构造器的类型化签名快照（render 用）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// 各参数默认值表达式（与 param_types 平行；None = 无默认值）。
    /// 供 MIR lower 在 delegation 调用点填充缺失参数时 lower。
    pub default_exprs: Vec<Option<crate::syntax::ast::Expr>>,
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypeConstraintsSnapshot {
    /// 类型参数名序列（按声明顺序）。
    pub type_params: Vec<Symbol>,
    /// where 约束（参数名, bound 文本）。
    pub constraints: Vec<(Symbol, crate::syntax::ast::GenericBound)>,
}

/// 一个文件的 typed 产物：源 AST 句柄 + per-NodeId 表达式类型表。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypedFile {
    pub file_id: FileId,
    /// 该文件的包前缀（点分，可能为空）。
    pub package_prefix: String,
    /// 表达式 `NodeId → TypeId`（仅 User 文件；Sysroot 不做 body 类型检查）。
    pub expr_types: NodeIdTable<TypeId>,
    /// 语义事实侧表（调用决议 / 成员 / place / effect / value_refs）。
    pub facts: SemanticFacts,
    /// HIR body 树（M2，transitional 增量）：顶层函数的 desugar 后树。MIR 翻转
    /// （M2-5）后成为唯一函数体表示，`gaps` 必须为空。
    pub trees: Vec<tree::FnTree>,
}

/// class 主构造器参数布局（typecheck 记录；MIR 继承构造链展开用）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClassCtorParamInfo {
    /// 参数名。
    pub name: Symbol,
    /// 参数类型。
    pub ty: TypeId,
    /// 是否 `val`/`var` 属性参数（为 true 才贡献对象字段）。
    pub is_property: bool,
}

/// class `: Super(args)` 主构造器委托的解析结果。
///
/// super 委托实参可以是任意表达式（函数调用、运算、ctor 参数引用、常量等），
/// 在 MIR `<Class>.$init` 合成时从 `TypeDecl.supertypes[base_index].args` 直接
/// lower（实参表达式已由 check_super_delegation_args typecheck，写回语义事实）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SuperCtorDelegation {
    /// 超类 FQN。
    pub super_fqn: Symbol,
    /// base supertype 在 `TypeDecl.supertypes` 中的索引（MIR 据此取实参 AST）。
    pub base_index: usize,
    /// 实参类型序列（按超类主构造器参数序；与 supertypes[base_index].args 平行）。
    /// 供 MIR 构造 CallArg 的 value_ty（实参表达式的推断类型）。
    pub arg_tys: Vec<TypeId>,
}

/// super 委托实参。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SuperCtorArg {
    /// 引用本类主构造器第 index 个参数（`A(tag)`）。
    CtorParam {
        /// 本类主构造器参数下标。
        index: u32,
        /// 参数类型。
        ty: TypeId,
    },
    /// 常量字面量实参（`A(1)` / `B("xyz")`）。
    Const {
        /// 字面量值。
        value: SuperCtorConst,
        /// 字面量类型。
        ty: TypeId,
    },
}

/// super 委托支持的常量字面量。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SuperCtorConst {
    Int(u128),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Unit,
}

/// typecheck 的完整产出：自包含的 typed HIR。
///
/// 由 [`crate::typecheck::run_typecheck`] 构造并返回。所有 `Symbol` 句柄通过内置
/// [`interner`](Self::interner) 解析；所有 `TypeId` 句柄通过内置
/// [`store`](Self::store) 查询。
/// 手动 serde 见 [`serde_impl`]（HashMap 字段按 key 排序序列化）。
#[derive(Clone, Debug)]
pub struct TypedHir {
    /// 类型存储（从 `TypeEnv` move 出；所有 TypeId 的唯一来源）。
    pub store: TypeStore,
    /// interner 副本（解析 Symbol → 文本）。
    pub interner: Interner,
    /// 顶层函数 FQN → 签名重载集。
    pub top_level_funs: HashMap<Symbol, Vec<TypedSignature>>,
    /// 类型 FQN → (方法名 → 签名重载集)。成员函数 / 扩展。
    pub member_funs: HashMap<Symbol, HashMap<Symbol, Vec<TypedSignature>>>,
    /// 类型 FQN → 成员函数名列表（按声明顺序；与 `member_funs` 同步填充）。
    ///
    /// `member_funs` 内层是 HashMap（迭代序不确定），vtable / itable slot 分配
    /// 必须以此表为准，保证逐次构建的方法序一致（参照 `member_order` 模式）。
    pub member_fun_order: HashMap<Symbol, Vec<Symbol>>,
    /// 类型 FQN → (成员名 → 成员类型)。属性 / 字段。
    pub members: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    /// 类型 FQN → 成员名列表（按声明顺序）。
    ///
    /// `members` 是 HashMap（迭代序不确定），字段布局 / 偏移计算必须以此表
    /// 为准：主构造器 val/var 参数按参数序在前，类型体内属性按声明序在后。
    pub member_order: HashMap<Symbol, Vec<Symbol>>,
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
    /// 所有 class FQN 集合（含 final/open/abstract）；用于 MIR lower 判定成员函数 receiver 是 ref。
    pub class_fqns: HashSet<Symbol>,
    /// 所有可被继承的 class FQN 集合（`open` 或 `abstract`）。
    /// 取补集即得"具体 class"（不可继承 → 虚方法可安全退化为直接调用）。
    /// MIR 去虚化 pass 据此判断 ref 类型接收者是否 final。
    pub extensible_class_fqns: HashSet<Symbol>,
    /// 超类型 → 直接子类型 FQN 列表（反转 index.supertypes）。
    /// 供 MIR 去虚化 pass 做 CHA（class hierarchy analysis）：
    /// 若某类型在此 map 中有子类型，则 receiver 可能是子类实例，不能简单去虚化。
    /// 若无子类型（不在 key 中），则 receiver 类型是精确的（exact），可去虚化。
    pub direct_subtypes: HashMap<Symbol, Vec<Symbol>>,
    /// 子类型 → 直接超类型 FQN 列表（正向 index.supertypes）。
    /// 供 MIR 收集 class × interface itable 契约。
    pub supertypes: HashMap<Symbol, Vec<Symbol>>,
    /// class FQN → 主构造器参数布局（含非属性参数；MIR 构造链展开用）。
    pub class_ctor_params: HashMap<Symbol, Vec<ClassCtorParamInfo>>,
    /// class FQN → `: Super(args)` 委托（可静态解析时记录）。
    pub super_ctor_delegations: HashMap<Symbol, SuperCtorDelegation>,
    /// 类型声明信息表：每个 `TypeId`（nominal / primitive / tuple / function）→
    /// 对应的 [`type_info::TypeInfo`]。由 `into_typed_hir` 在 freeze 边界构建，
    /// 将上述按 FQN 分散的声明信息合并为按 `TypeId` 索引的单条信息。
    ///
    /// 当前与上方 16 个旧字段并存（增量迁移阶段）；消费者仍用旧字段，此表待后续迁移。
    pub type_infos: HashMap<TypeId, type_info::TypeInfo>,
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

    /// 查询某构造器调用点选中的 ctor 声明 span（区分 primary/secondary）。
    pub fn ctor_selection(&self, file_id: FileId, node: NodeId) -> Option<scoop2_base::Span> {
        self.file(file_id)
            .and_then(|f| f.facts.ctor_selections.get(node).copied())
    }

    /// 查询某调用点的解析后实参列表（默认值填充 + 按位置排序）。
    pub fn resolved_call_args(&self, file_id: FileId, node: NodeId) -> Option<&[ResolvedCallArg]> {
        self.file(file_id)
            .and_then(|f| f.facts.resolved_call_args.get(node).map(|v| v.as_slice()))
    }

    /// 查询某 TypeRef 节点解析后的 TypeId（is/as 模式类型引用）。
    pub fn type_ref_resolution(&self, file_id: FileId, node: NodeId) -> Option<crate::ty::TypeId> {
        self.file(file_id)
            .and_then(|f| f.facts.type_ref_resolutions.get(node).copied())
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
    pub fn pattern_bindings(&self, file_id: FileId, node: NodeId) -> Option<&[PatternBinding]> {
        self.file(file_id)
            .and_then(|f| f.facts.pattern_bindings.get(node).map(|v| v.as_slice()))
    }

    /// 查询某表达式的 actual effect row。
    pub fn expr_effect_row(&self, file_id: FileId, node: NodeId) -> Option<&crate::ty::EffectRow> {
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

    /// 按声明序返回类型 `fqn` 的成员 `(name, ty)` 列表。
    ///
    /// 字段布局 / 偏移计算必须走这里（`members` HashMap 迭代序不确定）。
    /// `member_order` 缺失时回退为按成员名排序——仍然确定，只是不一定等于声明序。
    pub fn ordered_members(&self, fqn: &Symbol) -> Vec<(Symbol, TypeId)> {
        let Some(members) = self.members.get(fqn) else {
            return Vec::new();
        };
        match self.member_order.get(fqn) {
            Some(order) => order
                .iter()
                .filter_map(|&name| members.get(&name).map(|&ty| (name, ty)))
                .collect(),
            None => {
                let mut sorted: Vec<(Symbol, TypeId)> =
                    members.iter().map(|(&n, &t)| (n, t)).collect();
                sorted.sort_by(|a, b| self.interner.resolve(a.0).cmp(self.interner.resolve(b.0)));
                sorted
            }
        }
    }

    /// 按声明序返回类型 `fqn` 的成员函数名列表。
    ///
    /// vtable / itable slot 分配必须走这里（`member_funs` HashMap 迭代序不确定）。
    /// `member_fun_order` 缺失时回退为按方法名排序——仍然确定，只是不一定等于声明序。
    pub fn ordered_member_fun_names(&self, fqn: &Symbol) -> Vec<Symbol> {
        let Some(methods) = self.member_funs.get(fqn) else {
            return Vec::new();
        };
        match self.member_fun_order.get(fqn) {
            Some(order) => order
                .iter()
                .filter(|name| methods.contains_key(*name))
                .copied()
                .collect(),
            None => {
                let mut sorted: Vec<Symbol> = methods.keys().copied().collect();
                sorted.sort_by(|a, b| self.interner.resolve(*a).cmp(self.interner.resolve(*b)));
                sorted
            }
        }
    }

    /// class 的完整字段列表（`(name, ty)`）：超类字段在前（沿 `supertypes` 链
    /// 自顶向下），自身字段按声明序在后。同名遮蔽字段去重（子类优先……此处
    /// 保留首次出现 = 最顶层超类的声明，与单继承布局一致）。
    ///
    /// 非 class 类型（struct / enum variant）等价于 [`Self::ordered_members`]。
    pub fn ordered_class_fields(&self, fqn: Symbol) -> Vec<(Symbol, TypeId)> {
        let mut out: Vec<(Symbol, TypeId)> = Vec::new();
        let mut visited: HashSet<Symbol> = HashSet::new();
        // 先收集超类链（直接超类型中为 class 的第一个），自顶向下追加字段。
        let mut chain: Vec<Symbol> = Vec::new();
        let mut cur = fqn;
        while visited.insert(cur) {
            let next = self
                .supertypes
                .get(&cur)
                .and_then(|supers| supers.iter().find(|s| self.class_fqns.contains(s)).copied());
            match next {
                Some(sup) => {
                    chain.push(sup);
                    cur = sup;
                }
                None => break,
            }
        }
        for sup in chain.iter().rev() {
            out.extend(self.ordered_members(sup));
        }
        out.extend(self.ordered_members(&fqn));
        out
    }

    /// 判断某 FQN 是否为引用类型（class / interface / object）。
    /// 用于 MIR resolve_typeref 判断 ref vs value nominal（不查 TypeEnv）。
    pub fn is_reference_nominal(&self, fqn: Symbol) -> bool {
        self.class_fqns.contains(&fqn) || self.interface_fqns.contains(&fqn)
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
            member_fun_order: HashMap::new(),
            members: HashMap::new(),
            member_order: HashMap::new(),
            ctor_signatures: HashMap::new(),
            top_level_vals: HashMap::new(),
            enum_variants: HashMap::new(),
            type_constraints: HashMap::new(),
            interface_fqns: HashSet::new(),
            class_fqns: HashSet::new(),
            extensible_class_fqns: HashSet::new(),
            direct_subtypes: HashMap::new(),
            supertypes: HashMap::new(),
            class_ctor_params: HashMap::new(),
            super_ctor_delegations: HashMap::new(),
            type_infos: HashMap::new(),
            files: Vec::new(),
        }
    }
}
