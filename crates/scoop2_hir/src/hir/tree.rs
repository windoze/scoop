//! HIR body 树（PLAN.md M2-2）：desugar 后的**封闭节点词汇表**（C9-1）。
//!
//! 词汇表即契约：本枚举**不含**任何糖变体——`Binary`/`Unary`/`InfixCall` 在构造时
//! 展开为 `Call`（typecheck 已给出决议），`NotNullAssert`（`!!`）展开为
//! `Option.unwrap` 调用。下游对 [`TreeExprKind`] 的穷尽 match 写不出「处理糖」的
//! 分支；尚未能正确构造的构造记入 `gaps`（transitional——M2 完成前允许非空，
//! MIR 翻转时 gaps 必须为空，届时词汇表与 builder 同步收口）。
//!
//! 决议内联（C9-2）：每个节点直接携带推断类型（`ty`，裸非 Option——合法性由
//! completeness gate 上游保证）与决议句柄；[`SemanticFacts`] 侧表在此折叠为节点
//! 字段。局部绑定是 body 内 [`LocalId`]（C3 第三层，非 def、不外泄）。
//!
//! 物理表示：每 body 一组 arena（exprs/stmts/blocks/locals），id 为下标（C7——
//! 构造顺序即 AST 遍历顺序，确定）。

use scoop2_base::{NodeId, Span, Symbol};

use crate::hir::element::TypeCategory;
use crate::resolve::output::NodeIdTable;
use crate::ty::TypeId;

use super::facts::SemanticFacts;

// ---------------------------------------------------------------------------
// id 与容器
// ---------------------------------------------------------------------------

macro_rules! tree_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub u32);

        impl $name {
            #[inline]
            fn idx(self) -> usize {
                self.0 as usize
            }
        }
    };
}

tree_id!(ExprId);
tree_id!(StmtId);
tree_id!(BlockId);
tree_id!(LocalId);

/// 一个函数体的树：arena 集合 + 根块。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeBody {
    pub exprs: Vec<TreeExpr>,
    pub stmts: Vec<TreeStmt>,
    pub blocks: Vec<TreeBlock>,
    pub locals: Vec<TreeLocal>,
    /// 根块（函数体 / init 块入口）。
    pub root: Option<BlockId>,
}

/// 一个函数的树产物（挂在 [`super::TypedFile`] 上）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FnTree {
    /// 函数 FQN 文本（跨阶段稳定身份过渡用；M2 element 体系后换 def id）。
    pub fqn: String,
    /// 参数绑定（与声明序平行）。
    pub params: Vec<LocalId>,
    pub body: TreeBody,
    /// 构造缺口（transitional）：未能构造的构造点（span + 描述）。MIR 翻转前
    /// 必须清空；非空表示该函数的树不可消费。
    pub gaps: Vec<(Span, String)>,
    /// 顶层 `val`/`var` 初始化器树标记（Some(is_var) 表示该树是初始化器——
    /// lower 为 `InitializerRoot` 而非 `FunDecl`；函数/方法/$init 树为 None）。
    #[serde(default)]
    pub val_init: Option<bool>,
}

/// 文件顶层 item 骨架（源码序）：MIR 模块 lowering 的驱动序列——树本身无
/// 声明层信息（类型类别 / 无初始化器 val），骨架补齐模块级产出所需的声明
/// 元数据，并定位每个 item 产生的树区间。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileItem {
    pub kind: FileItemKind,
    /// item FQN 文本。
    pub fqn: String,
    /// 本 item 产生的树区间 [`start`, `end`)（`trees` 下标；无树 item 为空区间）。
    pub tree_range: (u32, u32),
    /// Type/Object 的成员槽位（源码序；`$init` 合成树固定在区间尾，不入列）。
    #[serde(default)]
    pub members: Vec<MemberSlot>,
    /// Fun 的签名提示 (return, effect)：扩展函数（fqn 无 top_level_funs 表项）
    /// lower 期查不到签名时使用——构建期按 owner 全集解析。
    #[serde(default)]
    pub fun_sig: Option<(TypeId, crate::ty::EffectRow)>,
}

/// 顶层 item 种类（MIR 模块产出对应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileItemKind {
    Fun,
    /// 顶层 `val`/`var`（有初始化器 → Initializer 树；无 → ExternGlobal）。
    Val,
    /// 类型声明（metadata + 成员方法树 + `$init` 合成树）。
    Type(TypeCategory),
    Object,
}

/// 类型成员槽位（源码序）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MemberSlot {
    /// 有 body 的方法（消费 tree_range 内按本槽位出现序排列的下一棵树）。
    Tree,
    /// 无 body 方法（接口 / 效应 op / abstract：签名-only FunDecl）。
    Bodyless {
        /// 方法 FQN 文本（`<owner>.<method>`）。
        fqn: String,
    },
}

/// 局部绑定（参数 / `val`/`var` / 模式子绑定）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeLocal {
    pub name: Symbol,
    pub ty: TypeId,
    pub mutable: bool,
    pub span: Span,
}

/// 块：语句序列 + 尾表达式（块是表达式，带尾值）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeBlock {
    pub span: Span,
    pub stmts: Vec<StmtId>,
    pub tail: Option<ExprId>,
}

// ---------------------------------------------------------------------------
// 表达式（desugar 后封闭词汇表）
// ---------------------------------------------------------------------------

/// 表达式节点：kind + 推断类型 + span（类型是裸的，非 Option——C9-2）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeExpr {
    pub kind: TreeExprKind,
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeExprKind {
    /// 字面量。
    Lit(Lit),
    /// 局部绑定引用。
    LocalRef(LocalId),
    /// 顶层 `val`/`var` 引用。
    TopLevelValRef {
        fqn: Symbol,
    },
    /// 调用（普通 / 方法 / 构造 / variant / 函数值 / perform——`Binary`/`Unary`/
    /// `InfixCall`/`!!`/下标全部收敛于此）。
    Call {
        callee: TreeCallee,
        args: Vec<ExprId>,
        /// 源实参命名标记（与 args 平行）：`resolved_call_args` 路径恒 None；
        /// 无该记录的回退路径携带源命名（ctor 字段映射 + CallArg 渲染用——
        /// 镜像 AST `lower_call_args`）。
        arg_names: Vec<Option<Symbol>>,
        /// 源实参 spread 标记（与 args 平行；语义同上）。
        arg_spread: Vec<bool>,
        /// 实参槽位 → 求值下标置换（镜像 AST「先按源序 lower 实参、再按
        /// resolved 序组装」的两遍分离；回退路径为恒等）。
        #[serde(default)]
        arg_order: Vec<u32>,
    },
    /// 成员读取（字段 / 元组下标）。
    Member {
        recv: ExprId,
        member: TreeMember,
    },
    /// 块表达式（含 `do` 块）。
    Block(BlockId),
    /// `if`（分支为块表达式）。
    If {
        cond: ExprId,
        then: ExprId,
        else_: Option<ExprId>,
    },
    /// `while`（Unit 值）。
    While {
        cond: ExprId,
        body: BlockId,
    },
    /// 元组字面量。
    Tuple(Vec<ExprId>),
    /// 数组字面量。
    ArrayLit(Vec<ExprId>),
    /// `when (subject) { arms }`（模式匹配原语，非糖）。
    When {
        subject: ExprId,
        arms: Vec<WhenTreeArm>,
    },
    /// `handle { body } on { arms } finally?`（effect 处理原语）。
    Handle {
        body: BlockId,
        arms: Vec<HandleTreeArm>,
        finally_: Option<BlockId>,
    },
    /// `base with { path: value, ... }`（函数式更新原语）。
    WithUpdate {
        base: ExprId,
        updates: Vec<(TreeFieldPath, ExprId)>,
    },
    /// lambda / 闭包字面量（参数类型按位取自函数类型 `ty`；隐式 `it` 为
    /// 参数 0。env 捕获清单在 M2 后续上移——当前由 MIR 计算）。
    Lambda {
        params: Vec<LocalId>,
        body: LambdaBodyTree,
        /// 隐式 `it` 形态（无显式参数；params[0] 是注入的 it 绑定）。MIR 的
        /// 嵌套 fn_ty **不含** it 参数（AST 路径的历史形态——fn_ty 与 Param
        /// 列表不一致的 quirk，字节一致保留）。
        implicit_it: bool,
    },
    /// 短路逻辑与 / 或（控制流原语，非方法调用）。
    LogicalAnd {
        lhs: ExprId,
        rhs: ExprId,
    },
    /// Elvis `a ?: b`（控制流原语：lhs 求值后非 null 取 lhs 否则 rhs）。
    Elvis {
        lhs: ExprId,
        rhs: ExprId,
    },
    LogicalOr {
        lhs: ExprId,
        rhs: ExprId,
    },
    /// 插值字符串原语（与 MIR `InterpolatedString` 同构）。
    InterpolatedString {
        parts: Vec<InterpPart>,
    },
    /// `recv?.member`（安全访问原语；member 已决议）。
    SafeMember {
        recv: ExprId,
        member: TreeMember,
    },
    /// struct 字面量 `Point { x: 1 }`（fqn 已解析）。
    StructLit {
        fqn: String,
        fields: Vec<(Symbol, ExprId)>,
    },
    /// `expr as T` / `expr as? T`（转换原语；目标已解析为 TypeId）。
    Cast {
        expr: ExprId,
        target: TypeId,
        /// `as?`（可空转换）为 true。
        nullable: bool,
    },
    /// 非空断言 `expr!!`（控制流原语：CondBr + panic 路径——与 MIR
    /// lower_not_null_assert 同构；typecheck 记录的 Option.unwrap 决议供他途）。
    NotNullAssert {
        expr: ExprId,
    },
    /// 未解析标识符（transitional 镜像：AST 路径对无决议 ident 的回退——
    /// 扩展函数体的 `this` 等历史形态；MIR UnresolvedName 同构，C1 清理时
    /// 与 AST 路径一并消灭）。
    UnresolvedName {
        name: String,
    },
    /// Bool 取反 `!x`（原语：AST 路径不走方法决议，发射 `v.equals(false)`
    /// 形态的分派调用——quirk 字节一致保留）。
    BoolNot {
        expr: ExprId,
    },
    /// 无决议调用回退（transitional 镜像：AST 路径对 call_resolutions 缺失的
    /// 错误程序形态——lower 实参后返回 Unit temp；C1 清理时一并消灭）。
    UnresolvedCall {
        args: Vec<ExprId>,
    },
    /// `expr is T` / `expr !is T`（类型判断原语；目标已解析为 TypeId）。
    TypeCheck {
        expr: ExprId,
        target: TypeId,
        /// `!is` 为 true。
        negated: bool,
    },
}

/// 插值字符串片段。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InterpPart {
    Lit(String),
    Expr(ExprId),
}

/// lambda 主体（块或表达式）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LambdaBodyTree {
    Block(BlockId),
    Expr(ExprId),
}

/// when 分支：模式 + 可选 guard + 体。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WhenTreeArm {
    pub pat: TreePattern,
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

/// 模式（binder 已解析为 [`LocalId`]，类型来自 `pattern_bindings`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreePattern {
    Wildcard,
    /// `_`-like 的 `else`。
    Else,
    /// `..` rest（解构上下文的剩余位置占位；不绑定）。
    Rest,
    /// binder 绑定。`local.ty` 来自 pattern_bindings（绑定语义类型）；
    /// `node_ty` 是模式节点自身的 expr_types 类型（镜像 AST 路径
    /// `lower_pattern_to_mir` 对 `expr_type(pat.id)` 的取值——PatternMatch
    /// tag args 的渲染用）。
    Binder {
        local: LocalId,
        node_ty: TypeId,
    },
    Literal(Lit),
    Tuple(Vec<TreePattern>),
    /// variant 模式（fqn 文本 + variant 名；句柄化随 M2 element 体系）。
    Variant {
        enum_fqn: String,
        variant: Symbol,
        args: Vec<TreePattern>,
    },
    /// struct 解构模式 `Point { x, y: sub }`（字段按 pattern 序；简写 `x` 的
    /// binder 与显式子模式分立——AST 路径两者 lowering 形态不同：简写走
    /// MemberAccess / 声明序下标提取，显式子模式递归绑定）。
    Struct {
        fields: Vec<StructFieldPat>,
    },
    /// `is T`（T 已解析为 TypeId）。
    Is {
        ty: TypeId,
    },
    /// or 模式 `A | B`。
    Or(Vec<TreePattern>),
}

/// struct 模式的字段：简写（`x` ≡ `x: x`）携带 binder；显式子模式携带
/// `sub`（两者互斥——显式 `x: x` 走 Binder 子模式，lowering 与简写不同）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructFieldPat {
    pub name: Symbol,
    pub binder: Option<LocalId>,
    pub sub: Option<TreePattern>,
}

/// handler arm（`Effect.op(binder: T) -> body`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleTreeArm {
    /// effect 路径文本（如 `scoop.core.Raise`；句柄化随 M2 element 体系）。
    pub effect_path: String,
    pub op: Symbol,
    /// op binders（出现序）。`ascription_ty` 是 binder 标注类型；缺失时
    /// lower 期回退 op 签名参数类型 / Any（镜像 AST 的 bty 链）。`local` 是
    /// arm body 作用域 token（MIR binder local 在 lower 期分配）。
    pub binders: Vec<HandleBinderSpec>,
    /// escape continuation binder（`, k ->`；类型 Any——effect lowering pass
    /// 后续替换，镜像 AST）。
    pub escape_cont: Option<LocalId>,
    pub body: ExprId,
}

/// handle arm 的 op binder 规格。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandleBinderSpec {
    pub name: Symbol,
    /// arm body 作用域 token。
    pub local: LocalId,
    /// binder 标注类型（`e: Int` 的 ascription）；无标注为 None。
    pub ascription_ty: Option<TypeId>,
    pub span: Span,
}

/// `with` 更新的字段路径（`a.b` / `0.1`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeFieldPath {
    pub segments: Vec<TreeFieldSeg>,
}

/// 路径段：具名或元组下标。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeFieldSeg {
    Named(Symbol),
    TupleIndex(u64),
}

/// 字面量值。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Lit {
    Unit,
    Bool(bool),
    /// 整型字面量（值 + 后缀——MIR Const 携带后缀物化语义）。
    Int(u128, Option<TreeIntSuffix>),
    Float(f64),
    Char(char),
    Str(String),
}

/// 整型字面量后缀（自有枚举——下游 MIR 不依赖 syntax 类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TreeIntSuffix {
    U,
    L,
    Ul,
}

/// 已解析的被调用方（从 [`super::ResolvedCall`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeCallee {
    /// 顶层函数直接调用（含解析为顶层函数的运算符糖——receiver 为隐式首参）。
    TopLevel {
        fqn: Symbol,
        /// 类型实参（显式优先，否则推断——与 MIR 合并规则一致）。
        type_args: Vec<TypeId>,
        /// 选定重载的参数类型（overload_sig / stable key 用）。
        param_types: Vec<TypeId>,
    },
    /// 方法调用（含运算符糖展开；receiver 是参数 0 之前的接收者表达式）。
    Method {
        recv: ExprId,
        owner_fqn: Symbol,
        method: Symbol,
        /// 虚分发候选（open/abstract/override）。
        is_virtual: bool,
        /// owner 是 interface（itable 分发）。
        is_interface: bool,
        type_args: Vec<TypeId>,
        param_types: Vec<TypeId>,
    },
    /// 构造器调用（`secondary` 区分 primary/secondary callable）。
    Ctor { type_fqn: Symbol, secondary: bool },
    /// enum variant 构造。
    Variant {
        enum_fqn: Symbol,
        variant: Symbol,
        /// 限定名形态（`Color.Red(...)`）；裸名（`Some(x)`）为 false——
        /// MIR AST 路径对限定形态先 lower callee 产生死语句，镜像需要。
        qualified: bool,
    },
    /// 局部函数值调用。
    LocalValue { local: LocalId },
    /// 任意函数值表达式调用（`f()(x)` / `fns[0](1)` / lambda 调用）。
    FunValue { callee: ExprId },
    /// effect 操作（perform 站点）。
    EffectOp { effect: Symbol, op: Symbol },
    /// 构造链委托调用（目标是合成的 callable——非源码声明，无 Symbol，按
    /// **完整 FQN 文本**携带：`<Class>.$init` 或 secondary `<Class>.$ctor.s<N>`）。
    InitCall { callee_fqn: String },
}

/// 已解析的成员读取（从 [`super::ResolvedMember`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeMember {
    Field { owner_fqn: Symbol, name: Symbol },
    TupleIndex { index: u64 },
}

// ---------------------------------------------------------------------------
// 语句
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreeStmt {
    /// 表达式语句。
    Expr(ExprId),
    /// 局部 `val`/`var`（`Name` 绑定）。
    LocalVal { local: LocalId, init: ExprId },
    /// 模式解构绑定（`val Some(x) = ...`；binder 已解析为局部）。
    Destructure {
        pat: TreePattern,
        init: ExprId,
        mutable: bool,
    },
    /// 赋值（LHS 已解析为 place）。
    Assign { place: TreePlace, value: ExprId },
    /// `return expr?`。
    Return(Option<ExprId>),
    /// `break`（绑定最近 enclosing while）。
    Break,
    /// `continue`。
    Continue,
}

/// 已解析的赋值目标（从 [`super::ResolvedPlace`] 折叠）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TreePlace {
    Local(LocalId),
    TopLevelVar {
        fqn: Symbol,
    },
    /// `recv.name = v`。`value_ty_hint`：字段声明类型（$init/$ctor 合成的
    /// 属性赋值用——镜像 AST 的 field_ty；普通赋取 None → 值操作数类型）。
    MemberField {
        recv: ExprId,
        owner_fqn: Symbol,
        name: Symbol,
        #[serde(default)]
        value_ty_hint: Option<TypeId>,
    },
}

// ---------------------------------------------------------------------------
// builder：AST + facts → 树
// ---------------------------------------------------------------------------

impl<'a> TreeBuilder<'a> {
    /// 模式 AST → [`TreePattern`]；`bindings` 按出现序供给 Binder 的类型。
    /// 构造模式树。binder 类型取自 pattern_bindings 侧表——**表是每节点的**
    ///（父节点的表只含直属 binder；嵌套 binder 记在嵌套节点自己的表里），
    /// 因此递归时按「父节点表 + 按名匹配」取类型，表缺失回退 Unit。
    fn build_pattern(
        &mut self,
        pat: &crate::syntax::ast::Pattern,
        parent_bindings: &[super::PatternBinding],
    ) -> Option<TreePattern> {
        use crate::syntax::ast::PatternKind;
        // 本节点的直属 binder 表（嵌套子模式递归时以它为父表）。
        let own_bindings: Vec<super::PatternBinding> = self
            .facts
            .pattern_bindings
            .get(pat.id)
            .cloned()
            .unwrap_or_default();
        match &pat.kind {
            PatternKind::Wildcard => Some(TreePattern::Wildcard),
            PatternKind::Else => Some(TreePattern::Else),
            PatternKind::Rest => Some(TreePattern::Rest),
            PatternKind::Bind(ident) => {
                let b = parent_bindings
                    .iter()
                    .find(|b| b.name == ident.symbol)
                    .cloned()
                    .unwrap_or(super::PatternBinding {
                        name: ident.symbol,
                        ty: self.unit_ty,
                        source: super::PatternBindingSource::WhenArm,
                        span: ident.span,
                    });
                let local = self.push_local(b.name, b.ty, false, b.span);
                // 模式节点自身类型（PatternMatch tag args 的渲染用——镜像
                // AST `expr_type(pat.id)` + any 回退；Any 已由 typecheck intern）。
                let node_ty = self.expr_types.get(pat.id).copied().unwrap_or_else(|| {
                    let n = crate::ty::NominalType {
                        fqn: self.types.any_fqn(),
                        args: vec![],
                        eff: None,
                    };
                    self.types
                        .find_interned(&crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                            n,
                        )))
                        .unwrap_or(self.unit_ty)
                });
                Some(TreePattern::Binder { local, node_ty })
            }
            PatternKind::Literal(l) => {
                use crate::syntax::ast::PatternLiteral;
                let lit = match l {
                    PatternLiteral::Int(v) => Lit::Int(v.value, v.suffix.map(tree_int_suffix)),
                    PatternLiteral::Char(c) => Lit::Char(c.value),
                    PatternLiteral::String(s) => Lit::Str(s.value.clone()),
                    PatternLiteral::Bool { value, .. } => Lit::Bool(*value),
                };
                Some(TreePattern::Literal(lit))
            }
            PatternKind::Tuple(els) => {
                let mut out = Vec::with_capacity(els.len());
                for e in els {
                    out.push(self.build_pattern(e, &own_bindings)?);
                }
                Some(TreePattern::Tuple(out))
            }
            PatternKind::Struct { fields, .. } => {
                // `Point { x, y }`：简写 `x` 等价 `x: x`（binder 类型来自本节点
                // 表，按名匹配）；显式子模式递归构造。
                let mut out = Vec::with_capacity(fields.len());
                for f in fields {
                    match &f.pattern {
                        Some(sub) => out.push(StructFieldPat {
                            name: f.name.symbol,
                            binder: None,
                            sub: Some(self.build_pattern(sub, &own_bindings)?),
                        }),
                        None => {
                            let b = own_bindings
                                .iter()
                                .find(|b| b.name == f.name.symbol)
                                .cloned()?;
                            let local = self.push_local(b.name, b.ty, false, b.span);
                            out.push(StructFieldPat {
                                name: f.name.symbol,
                                binder: Some(local),
                                sub: None,
                            });
                        }
                    }
                }
                Some(TreePattern::Struct { fields: out })
            }
            PatternKind::Variant { path, args } => {
                let enum_fqn: String = path
                    .segments
                    .iter()
                    .map(|s| self.interner.resolve(s.symbol))
                    .collect::<Vec<_>>()
                    .join(".");
                let variant = path.segments.last().map(|s| s.symbol).unwrap_or_default();
                let mut tree_args = Vec::new();
                if let Some(args) = args {
                    for a in args {
                        tree_args.push(self.build_pattern(a, &own_bindings)?);
                    }
                }
                Some(TreePattern::Variant {
                    enum_fqn,
                    variant,
                    args: tree_args,
                })
            }
            PatternKind::Is(ty_ref) => {
                let ty = self
                    .facts
                    .type_ref_resolutions
                    .get(ty_ref.id)
                    .copied()
                    .or_else(|| self.expr_types.get(ty_ref.id).copied())?;
                Some(TreePattern::Is { ty })
            }
            PatternKind::Or(els) => {
                let mut out = Vec::with_capacity(els.len());
                for e in els {
                    out.push(self.build_pattern(e, &own_bindings)?);
                }
                Some(TreePattern::Or(out))
            }
        }
    }
}

/// 从 typecheck 产物构造一个函数体的树。
///
/// - `body`：函数体 AST（typecheck 后的形态——for-loop 等已在 typecheck desugar）。
/// - `params`：参数 (名, 类型) 序列（声明序）。
/// - `unit_ty`：Unit 的 `TypeId`（while 等无值构造的类型）。
/// - `expr_types` / `facts`：该文件的 typecheck 侧表。
///
/// 未覆盖构造记入 `gaps`（不静默、不猜测）。
pub fn build_fn_tree(
    fqn: String,
    body: &crate::syntax::ast::FunBody,
    params: &[(Symbol, TypeId)],
    this_ty: Option<TypeId>,
    unit_ty: TypeId,
    expr_types: &NodeIdTable<TypeId>,
    facts: &SemanticFacts,
    interner: &scoop2_base::Interner,
    types: &crate::ty::TypeStore,
) -> FnTree {
    let mut b = TreeBuilder {
        expr_types,
        facts,
        unit_ty,
        interner,
        types,
        out: TreeBody::default(),
        scopes: vec![std::collections::HashMap::new()],
        gaps: Vec::new(),
    };
    // `this` 先于声明参数入 locals / params（MIR 方法参数序 [<this>, ...] 对齐）。
    let this_local = this_ty.and_then(|ty| {
        let sym = b.interner.get("this")?;
        Some(b.push_local(sym, ty, false, Span::new(0, 0)))
    });
    let mut param_locals: Vec<LocalId> = params
        .iter()
        .map(|&(name, ty)| {
            b.push_local(
                name,
                ty,
                false,
                // 参数 span：局部无精确 span 时用 body 起点近似（诊断用途）。
                Span::new(0, 0),
            )
        })
        .collect();
    if let Some(t) = this_local {
        param_locals.insert(0, t);
    }
    let root = match body {
        crate::syntax::ast::FunBody::Block(block) => b.build_block(block),
        crate::syntax::ast::FunBody::Expr(e) => {
            // `= expr` 短体：等价于 [return expr]。
            let block_id = b.fresh_block(Span::new(0, 0));
            if let Some(expr) = b.build_expr(e) {
                let stmt = b.push_stmt(TreeStmt::Return(Some(expr)));
                b.out.blocks[block_id.idx()].stmts.push(stmt);
            }
            block_id
        }
    };
    b.out.root = Some(root);
    FnTree {
        fqn,
        params: param_locals,
        body: b.out,
        gaps: b.gaps,
        val_init: None,
    }
}

/// 顶层 `val`/`var` 初始化器的树（fqn = val FQN；根块为**尾表达式**——与
/// MIR `InitializerRoot` 的「lower init + Return 终结」结构同构，不包
/// Return 语句）。
pub fn build_val_init_tree(
    fqn: String,
    init: &crate::syntax::ast::Expr,
    is_var: bool,
    unit_ty: TypeId,
    expr_types: &NodeIdTable<TypeId>,
    facts: &SemanticFacts,
    interner: &scoop2_base::Interner,
    types: &crate::ty::TypeStore,
) -> FnTree {
    let mut b = TreeBuilder {
        expr_types,
        facts,
        unit_ty,
        interner,
        types,
        out: TreeBody::default(),
        scopes: vec![std::collections::HashMap::new()],
        gaps: Vec::new(),
    };
    let root = b.fresh_block(init.span);
    if let Some(expr) = b.build_expr(init) {
        b.out.blocks[root.idx()].tail = Some(expr);
    }
    b.out.root = Some(root);
    FnTree {
        fqn,
        params: Vec::new(),
        body: b.out,
        gaps: b.gaps,
        val_init: Some(is_var),
    }
}

struct TreeBuilder<'a> {
    expr_types: &'a NodeIdTable<TypeId>,
    facts: &'a SemanticFacts,
    unit_ty: TypeId,
    interner: &'a scoop2_base::Interner,
    types: &'a crate::ty::TypeStore,
    out: TreeBody,
    /// 词法作用域栈（块进栈/出栈）。
    scopes: Vec<std::collections::HashMap<Symbol, LocalId>>,
    gaps: Vec<(Span, String)>,
}

impl<'a> TreeBuilder<'a> {
    fn gap(&mut self, span: Span, what: &str) {
        self.gaps.push((span, what.to_string()));
    }

    fn push_local(&mut self, name: Symbol, ty: TypeId, mutable: bool, span: Span) -> LocalId {
        let id = LocalId(self.out.locals.len() as u32);
        self.out.locals.push(TreeLocal {
            name,
            ty,
            mutable,
            span,
        });
        self.scopes
            .last_mut()
            .expect("作用域栈非空")
            .insert(name, id);
        id
    }

    fn lookup_local(&self, name: Symbol) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|s| s.get(&name).copied())
    }

    fn fresh_block(&mut self, span: Span) -> BlockId {
        let id = BlockId(self.out.blocks.len() as u32);
        self.out.blocks.push(TreeBlock {
            span,
            stmts: Vec::new(),
            tail: None,
        });
        id
    }

    fn push_stmt(&mut self, stmt: TreeStmt) -> StmtId {
        let id = StmtId(self.out.stmts.len() as u32);
        self.out.stmts.push(stmt);
        id
    }

    fn push_expr(&mut self, kind: TreeExprKind, ty: TypeId, span: Span) -> ExprId {
        let id = ExprId(self.out.exprs.len() as u32);
        self.out.exprs.push(TreeExpr { kind, ty, span });
        id
    }

    /// 节点类型（completeness 上游保证存在；缺失记 gap 并用 Unit 兜底构造失败路径）。
    fn ty_of(&mut self, id: NodeId, span: Span) -> Option<TypeId> {
        match self.expr_types.get(id) {
            Some(&ty) => Some(ty),
            None => {
                self.gap(span, "expr_types 缺失（completeness 泄漏）");
                None
            }
        }
    }

    fn build_block(&mut self, block: &crate::syntax::ast::Block) -> BlockId {
        self.scopes.push(std::collections::HashMap::new());
        let block_id = self.fresh_block(block.span);
        for stmt in &block.stmts {
            let stmt_id = self.build_stmt(stmt);
            if let Some(sid) = stmt_id {
                self.out.blocks[block_id.idx()].stmts.push(sid);
            }
        }
        // 尾值：块无尾语句时 stmts 末尾是表达式语句——AST 把尾表达式解析为
        // StmtKind::Expr；这里把「最后一个表达式语句」提升为 tail。
        let stmts_len = self.out.blocks[block_id.idx()].stmts.len();
        if stmts_len > 0 {
            let last = *self.out.blocks[block_id.idx()].stmts.last().expect("非空");
            if let TreeStmt::Expr(e) = self.out.stmts[last.idx()] {
                // 仅当源块没有尾分号时才是尾值；保守起见一律提升（语义等价：
                // 表达式语句的值被丢弃 vs 尾值——函数体需要尾值，普通块提升
                // 不改变 MIR lower 结果）。
                self.out.blocks[block_id.idx()].tail = Some(e);
                self.out.blocks[block_id.idx()]
                    .stmts
                    .truncate(stmts_len - 1);
            }
        }
        self.scopes.pop();
        block_id
    }

    fn build_stmt(&mut self, stmt: &crate::syntax::ast::Stmt) -> Option<StmtId> {
        use crate::syntax::ast::StmtKind;
        match &stmt.kind {
            StmtKind::Empty => None,
            StmtKind::Expr(e) => self
                .build_expr(e)
                .map(|id| self.push_stmt(TreeStmt::Expr(id))),
            StmtKind::Return { value } => {
                let v = value.as_ref().and_then(|e| self.build_expr(e));
                Some(self.push_stmt(TreeStmt::Return(v)))
            }
            StmtKind::Break => Some(self.push_stmt(TreeStmt::Break)),
            StmtKind::Continue => Some(self.push_stmt(TreeStmt::Continue)),
            StmtKind::LocalVal(d) => {
                let init_ast = match &d.init {
                    Some(e) => e,
                    None => {
                        self.gap(stmt.span, "局部声明缺初始化表达式");
                        return None;
                    }
                };
                let init = match self.build_expr(init_ast) {
                    Some(id) => id,
                    None => return None,
                };
                match &d.binding {
                    crate::syntax::ast::ValBinding::Name(name) => {
                        let Some(&init_ty_raw) = self.expr_types.get(init_ast.id) else {
                            self.gap(stmt.span, "局部声明类型缺失（completeness 泄漏）");
                            return None;
                        };
                        // `val ys: MutableArray<T> = [a, b]` / 空数组 `val a: T = []`：
                        // 声明类型覆盖字面量类型（镜像 lower_local_val 的上下文
                        // 转换——MakeArray 不 freeze / local 与 set 分派布局一致）。
                        let declared_ty =
                            d.ty.as_ref()
                                .and_then(|t| self.facts.type_ref_resolutions.get(t.id).copied());
                        let init_is_array_lit =
                            matches!(&init_ast.kind, crate::syntax::ast::ExprKind::ArrayLit(_));
                        let init_is_empty_array = matches!(
                            &init_ast.kind,
                            crate::syntax::ast::ExprKind::ArrayLit(els) if els.is_empty()
                        );
                        let declared_is_mutable_array = declared_ty.is_some_and(|t| {
                            let fqn = match self.types.kind(t) {
                                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                                    self.interner.resolve(n.fqn)
                                }
                                _ => "",
                            };
                            fqn.ends_with(".MutableArray")
                        });
                        let use_declared_for_lit = declared_ty.is_some()
                            && (init_is_empty_array
                                || (init_is_array_lit && declared_is_mutable_array));
                        let ty = if use_declared_for_lit {
                            let decl = declared_ty.expect("use_declared_for_lit 蕴含 Some");
                            if init_is_array_lit {
                                // 字面量节点类型同步覆盖（MakeArray temp 与结果
                                // 类型在 lowering 取节点 ty）。
                                self.out.exprs[init.idx()].ty = decl;
                            }
                            decl
                        } else {
                            init_ty_raw
                        };
                        let mutable = d.kind == crate::syntax::ast::ValKind::Var;
                        let local = self.push_local(name.symbol, ty, mutable, name.span);
                        Some(self.push_stmt(TreeStmt::LocalVal { local, init }))
                    }
                    crate::syntax::ast::ValBinding::Pattern(pat) => {
                        // 绑定类型：facts.pattern_bindings（每节点表，父表含直属
                        // binder——build_pattern 内部按节点取表）。
                        let tree_pat = self.build_pattern(pat, &[])?;
                        let mutable = d.kind == crate::syntax::ast::ValKind::Var;
                        Some(self.push_stmt(TreeStmt::Destructure {
                            pat: tree_pat,
                            init,
                            mutable,
                        }))
                    }
                }
            }
            StmtKind::Assign { target, value } => {
                let Some(value) = self.build_expr(value) else {
                    return None;
                };
                let place = self.build_place(target);
                place.map(|p| self.push_stmt(TreeStmt::Assign { place: p, value }))
            }
            StmtKind::While { cond, body } => {
                let Some(cond) = self.build_expr(cond) else {
                    return None;
                };
                let body_block = self.build_block(body);
                let while_expr = self.push_expr(
                    TreeExprKind::While {
                        cond,
                        body: body_block,
                    },
                    self.unit_ty,
                    stmt.span,
                );
                Some(self.push_stmt(TreeStmt::Expr(while_expr)))
            }
            StmtKind::For { .. } => {
                // for 已在 typecheck desugar；到达这里说明 desugar 未运行（C9：
                // 不写防御分支，gap 显式暴露）。
                self.gap(stmt.span, "for-loop（应已在 typecheck desugar）");
                None
            }
        }
    }

    fn build_place(&mut self, target: &crate::syntax::ast::AssignTarget) -> Option<TreePlace> {
        use crate::syntax::ast::AssignTargetKind;
        let Some(place) = self.facts.assign_places.get(target.id).cloned() else {
            self.gap(target.span, "赋值目标 place 决议缺失（completeness 泄漏）");
            return None;
        };
        match place {
            super::ResolvedPlace::Local { name, .. } => match self.lookup_local(name) {
                Some(local) => Some(TreePlace::Local(local)),
                None => self.gap_ret(target.span, "赋值局部超出词法作用域"),
            },
            super::ResolvedPlace::TopLevelVar { fqn, .. } => Some(TreePlace::TopLevelVar { fqn }),
            super::ResolvedPlace::MemberField {
                owner_fqn,
                member_name,
                ..
            } => {
                let recv = match &target.kind {
                    AssignTargetKind::Member { receiver, .. } => self.build_expr(receiver)?,
                    _ => return self.gap_ret(target.span, "成员赋值目标缺接收者"),
                };
                Some(TreePlace::MemberField {
                    recv,
                    owner_fqn,
                    name: member_name,
                    value_ty_hint: None,
                })
            }
            super::ResolvedPlace::Index { .. } => {
                self.gap_ret(target.span, "下标赋值（operator set 展开，M2 后续覆盖）")
            }
        }
    }

    fn build_expr(&mut self, expr: &crate::syntax::ast::Expr) -> Option<ExprId> {
        use crate::syntax::ast::ExprKind;
        let span = expr.span;
        // 字面量的类型缺失时回退 Unit（构造器默认值等未走 typecheck 的字面量
        // ——镜像 AST expr_ty 的 unit 回退；Const operand 不消费类型）。
        let lit_ty = self
            .expr_types
            .get(expr.id)
            .copied()
            .unwrap_or(self.unit_ty);
        match &expr.kind {
            ExprKind::UnitLit => Some(self.push_expr(TreeExprKind::Lit(Lit::Unit), lit_ty, span)),
            ExprKind::IntLit(l) => {
                Some(self.push_expr(
                    TreeExprKind::Lit(Lit::Int(l.value, l.suffix.map(tree_int_suffix))),
                    lit_ty,
                    span,
                ))
            }
            ExprKind::FloatLit(l) => {
                Some(self.push_expr(TreeExprKind::Lit(Lit::Float(l.value)), lit_ty, span))
            }
            ExprKind::CharLit(l) => {
                Some(self.push_expr(TreeExprKind::Lit(Lit::Char(l.value)), lit_ty, span))
            }
            ExprKind::StringLit(l) => {
                Some(self.push_expr(TreeExprKind::Lit(Lit::Str(l.value.clone())), lit_ty, span))
            }
            ExprKind::Ident(id) => self.build_ident(expr, id),
            ExprKind::Binary { lhs, op, rhs } => {
                // `&&` / `||` / `?:` 是控制流原语（非方法调用，无 call 决议）。
                match op {
                    crate::syntax::ast::BinaryOp::LogAnd | crate::syntax::ast::BinaryOp::LogOr => {
                        let ty = self.ty_of(expr.id, span)?;
                        let l = self.build_expr(lhs)?;
                        let r = self.build_expr(rhs)?;
                        let kind = if matches!(op, crate::syntax::ast::BinaryOp::LogAnd) {
                            TreeExprKind::LogicalAnd { lhs: l, rhs: r }
                        } else {
                            TreeExprKind::LogicalOr { lhs: l, rhs: r }
                        };
                        Some(self.push_expr(kind, ty, span))
                    }
                    crate::syntax::ast::BinaryOp::Elvis => {
                        let ty = self.ty_of(expr.id, span)?;
                        let l = self.build_expr(lhs)?;
                        let r = self.build_expr(rhs)?;
                        Some(self.push_expr(TreeExprKind::Elvis { lhs: l, rhs: r }, ty, span))
                    }
                    _ => self.build_desugared_call(expr, &[lhs, rhs]),
                }
            }
            ExprKind::InfixCall {
                receiver: lhs,
                arg: rhs,
                ..
            } => {
                // 运算符糖 → 方法调用（决议已由 typecheck 写入 call_resolutions）。
                self.build_desugared_call(expr, &[lhs, rhs])
            }
            ExprKind::Unary { op, expr: inner } => {
                if matches!(op, crate::syntax::ast::UnaryOp::Not) {
                    // `!x`：Bool 取反原语（AST 路径不走方法决议）。
                    let ty = self.ty_of(expr.id, span)?;
                    let inner_id = self.build_expr(inner)?;
                    Some(self.push_expr(TreeExprKind::BoolNot { expr: inner_id }, ty, span))
                } else {
                    self.build_desugared_call(expr, &[inner])
                }
            }
            ExprKind::NotNullAssert { expr: inner } => {
                let ty = self.ty_of(expr.id, span)?;
                let inner = self.build_expr(inner)?;
                Some(self.push_expr(TreeExprKind::NotNullAssert { expr: inner }, ty, span))
            }
            ExprKind::Index { receiver, indices } => {
                let mut args: Vec<&crate::syntax::ast::Expr> = vec![receiver];
                args.extend(indices.iter());
                self.build_desugared_call(expr, &args)
            }
            ExprKind::Call { callee, args } => self.build_call_expr(expr, callee, args),
            ExprKind::MemberAccess { receiver, .. } => self.build_member_expr(expr, receiver),
            ExprKind::Block(b) | ExprKind::DoBlock(b) => {
                let ty = self.ty_of(expr.id, span)?;
                let block = self.build_block(b);
                Some(self.push_expr(TreeExprKind::Block(block), ty, span))
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let ty = self.ty_of(expr.id, span)?;
                let cond = self.build_expr(cond)?;
                let then = self.build_expr(then_branch)?;
                let else_ = else_branch.as_ref().and_then(|e| self.build_expr(e));
                Some(self.push_expr(TreeExprKind::If { cond, then, else_ }, ty, span))
            }
            ExprKind::TupleLit(els) => {
                let ty = self.ty_of(expr.id, span)?;
                let items: Vec<ExprId> = els.iter().filter_map(|e| self.build_expr(e)).collect();
                if items.len() != els.len() {
                    return None;
                }
                Some(self.push_expr(TreeExprKind::Tuple(items), ty, span))
            }
            ExprKind::ArrayLit(els) => {
                let ty = self.ty_of(expr.id, span)?;
                let items: Vec<ExprId> = els.iter().filter_map(|e| self.build_expr(e)).collect();
                if items.len() != els.len() {
                    return None;
                }
                Some(self.push_expr(TreeExprKind::ArrayLit(items), ty, span))
            }
            // 注解表达式：语义在 typecheck 消费（@Suppress 等），树取内层表达式。
            ExprKind::Annotated { expr: inner, .. } => self.build_expr(inner),
            // ---- 尚未覆盖（记 gap，不静默）----
            ExprKind::When { subject, arms } => {
                let ty = self.ty_of(expr.id, span)?;
                let subject = self.build_expr(subject)?;
                let mut tree_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    self.scopes.push(std::collections::HashMap::new());
                    // 绑定类型：facts.pattern_bindings 每节点表（build_pattern
                    // 内部按节点取表、按名匹配）。
                    let pat = self.build_pattern(&arm.pat, &[])?;
                    let guard = arm.guard.as_ref().and_then(|g| self.build_expr(g));
                    let body = self.build_expr(&arm.body)?;
                    self.scopes.pop();
                    tree_arms.push(WhenTreeArm { pat, guard, body });
                }
                Some(self.push_expr(
                    TreeExprKind::When {
                        subject,
                        arms: tree_arms,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                let ty = self.ty_of(expr.id, span)?;
                let body_block = self.build_block(body);
                let mut tree_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    self.scopes.push(std::collections::HashMap::new());
                    let escape_cont = arm.escape_continuation.as_ref().and_then(|k| {
                        let binders = self.facts.handle_escape_binders.get(arm.id)?;
                        let &(sym, ty) = binders.first()?;
                        Some(self.push_local(sym, ty, false, k.span))
                    });
                    let effect_path: String = arm
                        .op
                        .effect_path
                        .segments
                        .iter()
                        .map(|s| self.interner.resolve(s.symbol))
                        .collect::<Vec<_>>()
                        .join(".");
                    // op binders（出现序）：ascription 类型缺失为 None（lower 期
                    // 回退 op 签名 / Any——镜像 AST bty 链）；local 的 ty 只是
                    // 作用域 token 占位（MIR binder local 在 lower 期按 bty 链分配）。
                    let mut binders = Vec::with_capacity(arm.op.binders.len());
                    for b in &arm.op.binders {
                        let ascription_ty =
                            b.ty.as_ref()
                                .and_then(|tr| self.expr_types.get(tr.id).copied());
                        let local = self.push_local(
                            b.name.symbol,
                            ascription_ty.unwrap_or(self.unit_ty),
                            false,
                            b.name.span,
                        );
                        binders.push(HandleBinderSpec {
                            name: b.name.symbol,
                            local,
                            ascription_ty,
                            span: b.name.span,
                        });
                    }
                    let body_expr = self.build_expr(&arm.body)?;
                    self.scopes.pop();
                    tree_arms.push(HandleTreeArm {
                        effect_path,
                        op: arm.op.op.symbol,
                        binders,
                        escape_cont,
                        body: body_expr,
                    });
                }
                let finally_block = finally.as_ref().map(|f| self.build_block(f));
                Some(self.push_expr(
                    TreeExprKind::Handle {
                        body: body_block,
                        arms: tree_arms,
                        finally_: finally_block,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::WithUpdate { base, updates } => {
                let ty = self.ty_of(expr.id, span)?;
                let base = self.build_expr(base)?;
                let mut tree_updates = Vec::with_capacity(updates.len());
                for u in updates {
                    let value = self.build_expr(&u.value)?;
                    let segments = u
                        .path
                        .segments
                        .iter()
                        .map(|s| match s {
                            crate::syntax::ast::MemberName::Named(id) => {
                                TreeFieldSeg::Named(id.symbol)
                            }
                            crate::syntax::ast::MemberName::TupleIndex { value, .. } => {
                                TreeFieldSeg::TupleIndex(*value as u64)
                            }
                        })
                        .collect();
                    tree_updates.push((TreeFieldPath { segments }, value));
                }
                Some(self.push_expr(
                    TreeExprKind::WithUpdate {
                        base,
                        updates: tree_updates,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::Lambda(lambda) => {
                let ty = self.ty_of(expr.id, span)?;
                // 参数类型按位取自函数类型；无标注参数（含隐式 `it`）由此定型。
                let fn_params: Vec<TypeId> = match self.types.kind(ty) {
                    crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Function(f)) => {
                        f.params.clone()
                    }
                    _ => return self.gap_ret(span, "lambda 类型不是函数类型"),
                };
                self.scopes.push(std::collections::HashMap::new());
                let mut param_locals = Vec::new();
                let mut implicit_it = false;
                if lambda.params.is_empty() {
                    // 隐式 `it`：函数类型参数 0。
                    if let Some(&it_ty) = fn_params.first() {
                        if let Some(it_sym) = self.interner.get("it") {
                            param_locals.push(self.push_local(it_sym, it_ty, false, span));
                            implicit_it = true;
                        }
                    }
                } else {
                    for (i, prm) in lambda.params.iter().enumerate() {
                        let pty = fn_params.get(i).copied().unwrap_or(self.unit_ty);
                        param_locals.push(self.push_local(
                            prm.name.symbol,
                            pty,
                            false,
                            prm.name.span,
                        ));
                    }
                }
                let body = match &lambda.body {
                    crate::syntax::ast::LambdaBody::Block(b) => {
                        LambdaBodyTree::Block(self.build_block(b))
                    }
                    crate::syntax::ast::LambdaBody::Expr(e) => {
                        LambdaBodyTree::Expr(self.build_expr(e)?)
                    }
                };
                self.scopes.pop();
                Some(self.push_expr(
                    TreeExprKind::Lambda {
                        params: param_locals,
                        body,
                        implicit_it,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::Cast {
                expr: inner,
                op,
                ty: ty_ref,
            } => {
                let ty = self.ty_of(expr.id, span)?;
                let inner = self.build_expr(inner)?;
                let Some(&target) = self.facts.type_ref_resolutions.get(ty_ref.id) else {
                    return self.gap_ret(span, "as 目标类型未解析（completeness 泄漏）");
                };
                let nullable = matches!(op, crate::syntax::ast::CastOp::AsSafe);
                Some(self.push_expr(
                    TreeExprKind::Cast {
                        expr: inner,
                        target,
                        nullable,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::TypeCheck {
                expr: inner,
                op,
                ty: ty_ref,
            } => {
                let ty = self.ty_of(expr.id, span)?;
                let inner = self.build_expr(inner)?;
                let Some(&target) = self.facts.type_ref_resolutions.get(ty_ref.id) else {
                    return self.gap_ret(span, "is 目标类型未解析（completeness 泄漏）");
                };
                let negated = matches!(op, crate::syntax::ast::TypeCheckOp::NotIs);
                Some(self.push_expr(
                    TreeExprKind::TypeCheck {
                        expr: inner,
                        target,
                        negated,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::InterpolatedString { parts, .. } => {
                let ty = self.ty_of(expr.id, span)?;
                let mut tree_parts = Vec::with_capacity(parts.len());
                for part in parts {
                    match part {
                        crate::syntax::ast::StringPart::Text(s) => {
                            tree_parts.push(InterpPart::Lit(s.clone()))
                        }
                        crate::syntax::ast::StringPart::Expr(e) => {
                            tree_parts.push(InterpPart::Expr(self.build_expr(e)?))
                        }
                    }
                }
                Some(self.push_expr(
                    TreeExprKind::InterpolatedString { parts: tree_parts },
                    ty,
                    span,
                ))
            }
            ExprKind::SafeMemberAccess { receiver, member } => {
                // `?.` 原语：null 短路路径（与 MIR lower_safe_member_access 同构；
                // 决议在 member_refs，按 Option 内层解析——typecheck 已修）。
                let ty = self.ty_of(expr.id, span)?;
                let recv = self.build_expr(receiver)?;
                let member = match self.resolve_tree_member(expr, receiver) {
                    Some(m) => m,
                    // member_refs 缺失（`?.` 扩展成员等）：镜像 AST——按名构造
                    //（MIR 元数据只消费名与 receiver 类型）。
                    None => match &member {
                        crate::syntax::ast::MemberName::Named(id) => TreeMember::Field {
                            owner_fqn: Symbol::default(),
                            name: id.symbol,
                        },
                        _ => return self.gap_ret(span, "?. 元组下标无决议"),
                    },
                };
                Some(self.push_expr(TreeExprKind::SafeMember { recv, member }, ty, span))
            }
            ExprKind::TypeApply { callee: inner, .. } => {
                // 显式类型应用：类型已 baked 进 callee——树直接取内层（构造即 desugar）。
                self.build_expr(inner)
            }
            ExprKind::StructLit { name, fields } => {
                let ty = self.ty_of(expr.id, span)?;
                let mut tree_fields = Vec::with_capacity(fields.len());
                for f in fields {
                    let v = self.build_expr(&f.value)?;
                    tree_fields.push((f.name.symbol, v));
                }
                // FQN 解析与 MIR resolve_struct_fqn 同规则：裸名 → scoop.core 前缀。
                let name_text = self.interner.resolve(name.symbol);
                let fqn = [name_text.to_string(), format!("scoop.core.{name_text}")]
                    .iter()
                    .find_map(|c| self.interner.get(c))
                    .unwrap_or(name.symbol);
                let fqn_text = self.interner.resolve(fqn).to_string();
                Some(self.push_expr(
                    TreeExprKind::StructLit {
                        fqn: fqn_text,
                        fields: tree_fields,
                    },
                    ty,
                    span,
                ))
            }
            ExprKind::UnsafeBlock(b) | ExprKind::SafeBlock(b) => {
                // MIR 对 @Unsafe/@Safe 块与普通块同构 lower——树同样折叠为 Block。
                let ty = self.ty_of(expr.id, span)?;
                let block = self.build_block(b);
                Some(self.push_expr(TreeExprKind::Block(block), ty, span))
            }
            ExprKind::ClassLit { .. } => self.gap_ret(span, "T::class 反射字面量（M2 后续）"),
            ExprKind::SpliceField { .. } => self.gap_ret(span, "splice field（特性已移除）"),
        }
    }

    fn gap_ret<T>(&mut self, span: Span, what: &str) -> Option<T> {
        self.gap(span, what);
        None
    }

    fn build_ident(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        id: &crate::syntax::ast::Ident,
    ) -> Option<ExprId> {
        let span = expr.span;
        let ty = self.ty_of(expr.id, span)?;
        // `true`/`false`：resolve 有意不写 value_refs（body.rs：由 typecheck 解释
        // 为 Bool 字面量）——树按字面量原语构造（lang-items 句柄化随 M2 后续）。
        let name_text = self.interner.resolve(id.symbol);
        if name_text == "true" || name_text == "false" {
            return Some(self.push_expr(
                TreeExprKind::Lit(Lit::Bool(name_text == "true")),
                ty,
                span,
            ));
        }
        match self.facts.value_refs.get(expr.id) {
            Some(super::ResolvedValue::Local { .. }) => match self.lookup_local(id.symbol) {
                Some(local) => Some(self.push_expr(TreeExprKind::LocalRef(local), ty, span)),
                // 作用域未命中（顶层 val 模式绑定跨初始化器引用等）：镜像
                // AST lower_ident 的回退链终态——UnresolvedName。
                None => Some(self.push_expr(
                    TreeExprKind::UnresolvedName {
                        name: name_text.to_string(),
                    },
                    ty,
                    span,
                )),
            },
            Some(super::ResolvedValue::TopLevelValue { fqn }) => {
                Some(self.push_expr(TreeExprKind::TopLevelValRef { fqn: *fqn }, ty, span))
            }
            Some(super::ResolvedValue::TopLevelFun { .. }) => {
                self.gap_ret(span, "函数名做值（FunVal 变体，M2 后续）")
            }
            None => match self.lookup_local(id.symbol) {
                // typecheck 注入的绑定（隐式 `it` 等）不走 value_refs（body.rs
                // 有意 defer 到 typecheck）——按词法作用域查找。
                Some(local) => Some(self.push_expr(TreeExprKind::LocalRef(local), ty, span)),
                None => Some(self.push_expr(
                    TreeExprKind::UnresolvedName {
                        name: name_text.to_string(),
                    },
                    ty,
                    span,
                )),
            },
        }
    }

    /// 糖表达式（Binary/Unary/InfixCall/!!/Index）→ Call：决议在 call_resolutions。
    fn build_desugared_call(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        arg_exprs: &[&crate::syntax::ast::Expr],
    ) -> Option<ExprId> {
        let span = expr.span;
        let resolution = match self.facts.call_resolutions.get(expr.id) {
            Some(r) => r.clone(),
            // 无决议（错误程序形态——顶层 val 模式绑定上下文的运算符等）：
            // 镜像 AST 回退——lower 实参后返回 Unit temp。
            None => {
                let ty = self.ty_of(expr.id, span)?;
                let mut ids = Vec::with_capacity(arg_exprs.len());
                for e in arg_exprs {
                    ids.push(self.build_expr(e)?);
                }
                return Some(self.push_expr(TreeExprKind::UnresolvedCall { args: ids }, ty, span));
            }
        };
        let ty = self.ty_of(expr.id, span)?;
        let all_args: Vec<ExprId> = arg_exprs
            .iter()
            .filter_map(|e| self.build_expr(e))
            .collect();
        if all_args.len() != arg_exprs.len() {
            return None;
        }
        // 糖调用的 callee：Method 的接收者就是首个实参表达式（`a + b` → `a.plus(b)`）。
        let merged = |explicit: &Vec<TypeId>, inferred: &Vec<TypeId>| -> Vec<TypeId> {
            if !explicit.is_empty() {
                explicit.clone()
            } else {
                inferred.clone()
            }
        };
        // Method 糖：args 去接收者（recv 独立）；其余形态保持全表。
        let is_method = matches!(resolution, super::ResolvedCall::Method { .. });
        let recv_id = all_args.first().copied();
        let args: Vec<ExprId> = if is_method {
            all_args[1.min(all_args.len())..].to_vec()
        } else {
            all_args
        };
        let callee = match &resolution {
            super::ResolvedCall::Method {
                owner_fqn,
                method_name,
                is_virtual,
                is_interface,
                explicit_type_args,
                inferred_type_args,
                param_types,
                ..
            } if recv_id.is_some() => TreeCallee::Method {
                recv: recv_id.expect("guard 保证非空"),
                owner_fqn: *owner_fqn,
                method: *method_name,
                is_virtual: *is_virtual,
                is_interface: *is_interface,
                type_args: merged(explicit_type_args, inferred_type_args),
                param_types: param_types.clone(),
            },
            super::ResolvedCall::TopLevelFun {
                fqn,
                explicit_type_args,
                inferred_type_args,
                param_types,
                ..
            } => TreeCallee::TopLevel {
                fqn: *fqn,
                type_args: merged(explicit_type_args, inferred_type_args),
                param_types: param_types.clone(),
            },
            super::ResolvedCall::Constructor { type_fqn, .. } => TreeCallee::Ctor {
                type_fqn: *type_fqn,
                secondary: false,
            },
            super::ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name,
                ..
            } => TreeCallee::Variant {
                enum_fqn: *enum_fqn,
                variant: *variant_name,
                qualified: false,
            },
            super::ResolvedCall::EffectOp {
                effect_name,
                op_name,
                ..
            } => TreeCallee::EffectOp {
                effect: *effect_name,
                op: *op_name,
            },
            _ => return self.gap_ret(span, "糖调用出现非预期决议形态"),
        };
        let n = args.len();
        Some(self.push_expr(
            TreeExprKind::Call {
                callee,
                args,
                arg_names: vec![None; n],
                arg_spread: vec![false; n],
                arg_order: (0..n as u32).collect(),
            },
            ty,
            span,
        ))
    }

    fn build_call_expr(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        callee: &crate::syntax::ast::Expr,
        args: &[crate::syntax::ast::CallArg],
    ) -> Option<ExprId> {
        use crate::syntax::ast::ExprKind;
        let span = expr.span;
        let resolution = match self.facts.call_resolutions.get(expr.id) {
            Some(r) => r.clone(),
            None => return self.gap_ret(span, "call 决议缺失（completeness 泄漏）"),
        };
        let ty = self.ty_of(expr.id, span)?;
        let callee_id = self.callee_from_resolution(&resolution, callee, span)?;
        let mut arg_ids: Vec<ExprId> = Vec::with_capacity(args.len());
        for a in args {
            let Some(id) = self.build_expr(&a.value) else {
                return None;
            };
            arg_ids.push(id);
        }
        // 默认值填充 + 位置排序（消费 resolved_call_args——MIR 不再做 resolution /
        // default filling）：Provided → 源实参；Default → 构造默认值表达式。
        // 命名/spread 元数据只在回退路径携带（resolved 路径按位无名——镜像
        // AST lower_resolved_call_args）。
        // 求值序 = 源实参序 + 默认值（按 resolved 序追加）；组装序（槽位→
        // 求值下标）镜像 lower_resolved_call_args 的两遍结构。
        let (final_args, arg_names, arg_spread, arg_order) =
            match self.facts.resolved_call_args.get(expr.id) {
                Some(resolved) => {
                    let mut evals = arg_ids;
                    let mut order: Vec<u32> = Vec::with_capacity(resolved.len());
                    for ra in resolved {
                        match ra {
                            super::ResolvedCallArg::Provided { original_index } => {
                                if *original_index >= evals.len() {
                                    return self.gap_ret(
                                        span,
                                        "resolved_call_args 越界（completeness 泄漏）",
                                    );
                                }
                                order.push(*original_index as u32);
                            }
                            super::ResolvedCallArg::Default { expr } => {
                                let Some(id) = self.build_expr(expr) else {
                                    return None;
                                };
                                order.push(evals.len() as u32);
                                evals.push(id);
                            }
                        }
                    }
                    let n = order.len();
                    (evals, vec![None; n], vec![false; n], order)
                }
                None => {
                    let names = args
                        .iter()
                        .map(|a| a.name.as_ref().map(|n| n.symbol))
                        .collect();
                    let spreads = args.iter().map(|a| a.is_spread).collect();
                    let order = (0..arg_ids.len() as u32).collect();
                    (arg_ids, names, spreads, order)
                }
            };
        Some(self.push_expr(
            TreeExprKind::Call {
                callee: callee_id,
                args: final_args,
                arg_names,
                arg_spread,
                arg_order,
            },
            ty,
            span,
        ))
    }

    fn callee_from_resolution(
        &mut self,
        resolution: &super::ResolvedCall,
        callee: &crate::syntax::ast::Expr,
        span: Span,
    ) -> Option<TreeCallee> {
        use crate::syntax::ast::ExprKind;
        // 类型实参合并：显式优先，否则推断（与 MIR emit_call_resolution 一致）。
        let merged = |explicit: &Vec<TypeId>, inferred: &Vec<TypeId>| -> Vec<TypeId> {
            if !explicit.is_empty() {
                explicit.clone()
            } else {
                inferred.clone()
            }
        };
        match resolution {
            super::ResolvedCall::TopLevelFun {
                fqn,
                explicit_type_args,
                inferred_type_args,
                param_types,
                ..
            } => Some(TreeCallee::TopLevel {
                fqn: *fqn,
                type_args: merged(explicit_type_args, inferred_type_args),
                param_types: param_types.clone(),
            }),
            super::ResolvedCall::Method {
                owner_fqn,
                method_name,
                is_virtual,
                is_interface,
                explicit_type_args,
                inferred_type_args,
                param_types,
                ..
            } => {
                // 接收者 = callee（MemberAccess）的 receiver 子表达式。
                let recv = match &callee.kind {
                    ExprKind::MemberAccess { receiver, .. } => self.build_expr(receiver)?,
                    _ => return self.gap_ret(span, "方法调用的 callee 不是成员访问"),
                };
                Some(TreeCallee::Method {
                    recv,
                    owner_fqn: *owner_fqn,
                    method: *method_name,
                    is_virtual: *is_virtual,
                    is_interface: *is_interface,
                    type_args: merged(explicit_type_args, inferred_type_args),
                    param_types: param_types.clone(),
                })
            }
            // primary/secondary 区分：MIR 侧由 ctor_selections（声明 span）判定；
            // element 体系后续携带显式标记，此处保守 primary。
            super::ResolvedCall::Constructor { type_fqn, .. } => Some(TreeCallee::Ctor {
                type_fqn: *type_fqn,
                secondary: false,
            }),
            super::ResolvedCall::EnumVariant {
                enum_fqn,
                variant_name,
                ..
            } => Some(TreeCallee::Variant {
                enum_fqn: *enum_fqn,
                variant: *variant_name,
                qualified: matches!(callee.kind, ExprKind::MemberAccess { .. }),
            }),
            super::ResolvedCall::LocalValue { local_name, .. } => {
                match self.lookup_local(*local_name) {
                    Some(local) => Some(TreeCallee::LocalValue { local }),
                    None => self.gap_ret(span, "LocalValue 调用目标超出词法作用域"),
                }
            }
            super::ResolvedCall::FunValue { .. } => {
                // callee 是任意函数类型表达式（`f()(x)` / `fns[0](1)` / lambda 调用）。
                let callee_expr = self.build_expr(callee)?;
                Some(TreeCallee::FunValue {
                    callee: callee_expr,
                })
            }
            super::ResolvedCall::EffectOp {
                effect_name,
                op_name,
                ..
            } => Some(TreeCallee::EffectOp {
                effect: *effect_name,
                op: *op_name,
            }),
        }
    }

    /// 成员决议 → TreeMember（Member/SafeMember 共用）。
    fn resolve_tree_member(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        _receiver: &crate::syntax::ast::Expr,
    ) -> Option<TreeMember> {
        match self.facts.member_refs.get(expr.id) {
            Some(super::ResolvedMember::Field {
                owner_fqn,
                member_name,
                ..
            }) => Some(TreeMember::Field {
                owner_fqn: *owner_fqn,
                name: *member_name,
            }),
            Some(super::ResolvedMember::TupleIndex { index, .. }) => Some(TreeMember::TupleIndex {
                index: *index as u64,
            }),
            Some(super::ResolvedMember::Method { .. }) => {
                self.gap_ret(expr.span, "方法引用做值（FunVal 变体，M2 后续）")
            }
            None => self.gap_ret(expr.span, "member 决议缺失（completeness 泄漏）"),
        }
    }

    fn build_member_expr(
        &mut self,
        expr: &crate::syntax::ast::Expr,
        receiver: &crate::syntax::ast::Expr,
    ) -> Option<ExprId> {
        let span = expr.span;
        let ty = self.ty_of(expr.id, span)?;
        let recv = self.build_expr(receiver)?;
        let member = self.resolve_tree_member(expr, receiver)?;
        Some(self.push_expr(TreeExprKind::Member { recv, member }, ty, span))
    }
}

// ---------------------------------------------------------------------------
// class `$init` 合成（M2-3：从 MIR lower_class_init_callable 上移）
// ---------------------------------------------------------------------------

/// 合成 class 的 `<Fqn>.$init` 树（无则 None）。
///
/// Kotlin 语义顺序（与 MIR lower_class_init_callable 一致，字节级对齐目标）：
/// 1. super 委托 `: Super(args)` → `<Super>.$init(this, args...)`；
/// 2. 首个 property 初始化器 / init 块之前：主构造 `val/var` 属性参数 →
///    `this.field = param`（无任何初始化体时在末尾发出）；
/// 3. property 初始化器 / init 块按源码顺序交错执行。
#[allow(clippy::too_many_arguments)]
pub fn synthesize_class_init_tree(
    hir: &super::TypedHir,
    tf: &super::TypedFile,
    interner: &scoop2_base::Interner,
    unit_ty: TypeId,
    owner_fqn_text: &str,
    owner_fqn_sym: Symbol,
    d: &crate::syntax::ast::TypeDecl,
) -> Option<FnTree> {
    use crate::syntax::ast::TypeMemberKind;

    if !matches!(d.kind, crate::syntax::ast::TypeKind::Class) {
        return None;
    }
    let body = d.body.as_ref()?;
    let has_init_block = body
        .members
        .iter()
        .any(|m| matches!(m.kind, TypeMemberKind::InitBlock(_)));
    let has_property_init = body
        .members
        .iter()
        .any(|m| matches!(&m.kind, TypeMemberKind::Property(p) if p.init.is_some()));
    let has_super = hir.super_ctor_delegations.contains_key(&owner_fqn_sym);
    if !(has_init_block || has_property_init || has_super) {
        return None;
    }

    let this_ty = super_decl_ty(hir, owner_fqn_sym)?;
    let ctor_params: Vec<super::ClassCtorParamInfo> = hir
        .class_ctor_params
        .get(&owner_fqn_sym)
        .cloned()
        .unwrap_or_default();
    let primary_param_names: Vec<Symbol> = d
        .primary_ctor
        .as_ref()
        .map(|pc| pc.params.iter().map(|p| p.name.symbol).collect())
        .unwrap_or_default();

    let mut b = TreeBuilder {
        expr_types: &tf.expr_types,
        facts: &tf.facts,
        unit_ty,
        interner,
        types: &hir.store,
        out: TreeBody::default(),
        scopes: vec![std::collections::HashMap::new()],
        gaps: Vec::new(),
    };

    // this + 构造参数绑定（init 块 / 属性初始化器可引用参数名）。
    let this_sym = interner.get("this")?;
    let this_local = b.push_local(this_sym, this_ty, false, d.name.span);
    let mut param_locals = Vec::with_capacity(ctor_params.len() + 1);
    // $init 树的 params = [this, ctor_params...]（镜像 lower_class_init_callable
    // 的参数序——MIR fn_ty 与 FunDecl.params 都含 this）。
    param_locals.push(this_local);
    for (i, cp) in ctor_params.iter().enumerate() {
        let name_sym = primary_param_names.get(i).copied().unwrap_or(cp.name);
        param_locals.push(b.push_local(name_sym, cp.ty, false, d.name.span));
    }

    let root = b.fresh_block(d.name.span);
    let this_ref = b.push_expr(TreeExprKind::LocalRef(this_local), this_ty, d.name.span);

    // 1. super 委托。
    if let Some(super_del) = hir.super_ctor_delegations.get(&owner_fqn_sym) {
        let target_class = hir.interner.resolve(super_del.super_fqn).to_string();
        let base_args: &[crate::syntax::ast::CallArg] = d
            .supertypes
            .get(super_del.base_index)
            .map(|st| st.args.as_slice())
            .unwrap_or(&[]);
        // 超类 ctor 签名（按参数数匹配——镜像 emit_super_init_call）+
        // 默认值填充 / 命名实参排序（镜像 lower_delegation_args）。
        let super_sig = hir
            .ctor_signatures
            .get(&super_del.super_fqn)
            .and_then(|sigs| {
                let n_args = base_args.len();
                sigs.iter()
                    .find(|sg| {
                        let min_arity = sg
                            .has_defaults
                            .iter()
                            .position(|d| *d)
                            .unwrap_or(sg.param_types.len());
                        n_args >= min_arity && n_args <= sg.param_types.len()
                    })
                    .or_else(|| sigs.first())
            });
        let mut args = vec![this_ref];
        match super_sig {
            Some(sig) => {
                let all_positional = base_args.iter().all(|a| a.name.is_none());
                if all_positional && base_args.len() == sig.param_types.len() {
                    for a in base_args {
                        if let Some(id) = b.build_expr(&a.value) {
                            args.push(id);
                        }
                    }
                } else {
                    let mut positional = base_args.iter().filter(|a| a.name.is_none());
                    for (param_idx, &pname) in sig.param_names.iter().enumerate() {
                        let named = base_args
                            .iter()
                            .find(|a| a.name.as_ref().is_some_and(|n| n.symbol == pname));
                        if let Some(a) = named {
                            if let Some(id) = b.build_expr(&a.value) {
                                args.push(id);
                            }
                        } else if let Some(a) = positional.next() {
                            if let Some(id) = b.build_expr(&a.value) {
                                args.push(id);
                            }
                        } else if let Some(Some(default_expr)) = sig.default_exprs.get(param_idx)
                            && let Some(id) = b.build_expr(default_expr)
                        {
                            args.push(id);
                        }
                    }
                }
            }
            None => {
                for a in base_args {
                    if let Some(id) = b.build_expr(&a.value) {
                        args.push(id);
                    }
                }
            }
        }
        let n = args.len();
        let call = b.push_expr(
            TreeExprKind::Call {
                callee: TreeCallee::InitCall {
                    callee_fqn: format!("{}.$init", target_class),
                },
                args,
                arg_names: vec![None; n],
                arg_spread: vec![false; n],
                arg_order: (0..n as u32).collect(),
            },
            unit_ty,
            d.name.span,
        );
        let stmt = b.push_stmt(TreeStmt::Expr(call));
        b.out.blocks[root.idx()].stmts.push(stmt);
    }

    // 2+3. 属性参数赋值 + property 初始化器 / init 块（源码序交错）。
    let mut emitted_param_props = false;
    let mut emit_param_props = |b: &mut TreeBuilder, root: BlockId, this_ref: ExprId| {
        if emitted_param_props {
            return;
        }
        emitted_param_props = true;
        for (i, cp) in ctor_params.iter().enumerate() {
            if !cp.is_property {
                continue;
            }
            let value = b.push_expr(
                TreeExprKind::LocalRef(param_locals[i + 1]),
                cp.ty,
                d.name.span,
            );
            let stmt = b.push_stmt(TreeStmt::Assign {
                place: TreePlace::MemberField {
                    recv: this_ref,
                    owner_fqn: owner_fqn_sym,
                    name: cp.name,
                    value_ty_hint: Some(cp.ty),
                },
                value,
            });
            b.out.blocks[root.idx()].stmts.push(stmt);
        }
    };

    for m in &body.members {
        match &m.kind {
            TypeMemberKind::Property(p) => {
                let Some(init) = &p.init else { continue };
                emit_param_props(&mut b, root, this_ref);
                let Some(value) = b.build_expr(init) else {
                    continue;
                };
                let field_ty = b.expr_types.get(init.id).copied().unwrap_or(unit_ty);
                // 字段声明类型（HIR members——镜像 AST 的 field_ty 查询）。
                let field_ty = hir
                    .members
                    .get(&owner_fqn_sym)
                    .and_then(|mm| mm.get(&p.name.symbol))
                    .copied()
                    .unwrap_or(unit_ty);
                let stmt = b.push_stmt(TreeStmt::Assign {
                    place: TreePlace::MemberField {
                        recv: this_ref,
                        owner_fqn: owner_fqn_sym,
                        name: p.name.symbol,
                        value_ty_hint: Some(field_ty),
                    },
                    value,
                });
                let _ = field_ty;
                b.out.blocks[root.idx()].stmts.push(stmt);
            }
            TypeMemberKind::InitBlock(ib) => {
                emit_param_props(&mut b, root, this_ref);
                let block = b.build_block(&ib.body);
                // init 块的内联：把块语句接到根（保留块内尾值丢弃语义——
                // build_block 会把末尾表达式语句提升为 tail，此处作为表达式
                // 语句回填，值丢弃但副作用发生）。
                for s in b.out.blocks[block.idx()].stmts.clone() {
                    b.out.blocks[root.idx()].stmts.push(s);
                }
                if let Some(tail) = b.out.blocks[block.idx()].tail {
                    let stmt = b.push_stmt(TreeStmt::Expr(tail));
                    b.out.blocks[root.idx()].stmts.push(stmt);
                }
            }
            _ => {}
        }
    }
    // 无任何初始化体时，属性参数赋值在末尾发出（保证字段已初始化）。
    emit_param_props(&mut b, root, this_ref);

    b.out.root = Some(root);
    Some(FnTree {
        fqn: format!("{owner_fqn_text}.$init"),
        params: param_locals,
        body: b.out,
        gaps: b.gaps,
        val_init: None,
    })
}

/// secondary 构造器合成树：`<Class>.$ctor.s<span.start>`（镜像
/// `lower_secondary_ctor_callable`——delegation（this/super/无）+ super 路径的
/// 属性参数赋值 / property 初始化器 / init 块 + body）。
#[allow(clippy::too_many_arguments)]
pub fn synthesize_secondary_ctor_tree(
    hir: &super::TypedHir,
    tf: &super::TypedFile,
    interner: &scoop2_base::Interner,
    unit_ty: TypeId,
    owner_fqn_text: &str,
    owner_fqn_sym: Symbol,
    d: &crate::syntax::ast::TypeDecl,
    sc: &crate::syntax::ast::SecondaryCtorDecl,
) -> Option<FnTree> {
    use crate::syntax::ast::TypeMemberKind;

    let this_ty = super_decl_ty(hir, owner_fqn_sym)?;
    let owner_fqn = owner_fqn_text.to_string();

    // secondary ctor 参数类型（ctor_signatures 按 span 匹配）。
    let sig_params: Vec<TypeId> = hir
        .ctor_signatures
        .get(&owner_fqn_sym)
        .and_then(|sigs| sigs.iter().find(|s| s.decl_span == sc.span))
        .map(|s| s.param_types.clone())
        .unwrap_or_default();

    let mut b = TreeBuilder {
        expr_types: &tf.expr_types,
        facts: &tf.facts,
        unit_ty,
        interner,
        types: &hir.store,
        out: TreeBody::default(),
        scopes: vec![std::collections::HashMap::new()],
        gaps: Vec::new(),
    };

    // this + secondary 参数绑定。
    let this_sym = interner.get("this")?;
    let this_local = b.push_local(this_sym, this_ty, false, sc.span);
    let mut param_locals = vec![this_local];
    for (i, p) in sc.params.iter().enumerate() {
        let pty = sig_params.get(i).copied().unwrap_or(unit_ty);
        param_locals.push(b.push_local(p.name.symbol, pty, false, p.name.span));
    }

    let root = b.fresh_block(sc.span);
    let this_ref = b.push_expr(TreeExprKind::LocalRef(this_local), this_ty, sc.span);

    // primary ctor 参数布局（super 路径的属性参数赋值用）。
    let ctor_params: Vec<super::ClassCtorParamInfo> = hir
        .class_ctor_params
        .get(&owner_fqn_sym)
        .cloned()
        .unwrap_or_default();
    let primary_param_names: Vec<Symbol> = d
        .primary_ctor
        .as_ref()
        .map(|pc| pc.params.iter().map(|p| p.name.symbol).collect())
        .unwrap_or_default();

    // ---- delegation ----
    let mut emit_init_call =
        |b: &mut TreeBuilder, root: BlockId, callee_fqn: String, args: Vec<ExprId>| {
            let n = args.len();
            let call = b.push_expr(
                TreeExprKind::Call {
                    callee: TreeCallee::InitCall { callee_fqn },
                    args,
                    arg_names: vec![None; n],
                    arg_spread: vec![false; n],
                    arg_order: (0..n as u32).collect(),
                },
                unit_ty,
                sc.span,
            );
            let stmt = b.push_stmt(TreeStmt::Expr(call));
            b.out.blocks[root.idx()].stmts.push(stmt);
        };
    // delegation 实参填充（镜像 lower_delegation_args：命名按位 / 位置顺填 /
    // 默认表达式补齐——目标签名按参数数匹配）。
    let fill_delegation_args = |b: &mut TreeBuilder,
                                del: &crate::syntax::ast::CtorDelegation,
                                target_sig: Option<&crate::hir::TypedSignature>|
     -> Vec<ExprId> {
        let args = &del.args;
        let Some(sig) = target_sig else {
            return args.iter().filter_map(|a| b.build_expr(&a.value)).collect();
        };
        let all_positional = args.iter().all(|a| a.name.is_none());
        if all_positional && args.len() == sig.param_types.len() {
            return args.iter().filter_map(|a| b.build_expr(&a.value)).collect();
        }
        let mut out = Vec::with_capacity(sig.param_types.len());
        let mut positional = args.iter().filter(|a| a.name.is_none());
        for (param_idx, &pname) in sig.param_names.iter().enumerate() {
            let named = args
                .iter()
                .find(|a| a.name.as_ref().is_some_and(|n| n.symbol == pname));
            if let Some(a) = named {
                if let Some(id) = b.build_expr(&a.value) {
                    out.push(id);
                }
            } else if let Some(a) = positional.next() {
                if let Some(id) = b.build_expr(&a.value) {
                    out.push(id);
                }
            } else if let Some(Some(default_expr)) = sig.default_exprs.get(param_idx)
                && let Some(id) = b.build_expr(default_expr)
            {
                out.push(id);
            }
        }
        out
    };

    let mut emit_class_steps = |b: &mut TreeBuilder, root: BlockId, this_ref: ExprId| {
        // 属性参数赋值 + property 初始化器 / init 块（源码序交错；主参数名不在
        // secondary 作用域时回退 this local——镜像 AST 的 this_lid 兜底）。
        let mut emitted = false;
        let mut emit_props = |b: &mut TreeBuilder, root: BlockId| {
            if emitted {
                return;
            }
            emitted = true;
            for (i, cp) in ctor_params.iter().enumerate() {
                if !cp.is_property {
                    continue;
                }
                let name_sym = primary_param_names.get(i).copied().unwrap_or(cp.name);
                let value_local = param_locals
                    .iter()
                    .copied()
                    .find(|&l| b.out.locals[l.idx()].name == name_sym)
                    .unwrap_or(this_local);
                let value = b.push_expr(TreeExprKind::LocalRef(value_local), cp.ty, sc.span);
                let stmt = b.push_stmt(TreeStmt::Assign {
                    place: TreePlace::MemberField {
                        recv: this_ref,
                        owner_fqn: owner_fqn_sym,
                        name: cp.name,
                        value_ty_hint: Some(cp.ty),
                    },
                    value,
                });
                b.out.blocks[root.idx()].stmts.push(stmt);
            }
        };
        let Some(body) = &d.body else {
            emit_props(b, root);
            return;
        };
        for m in &body.members {
            match &m.kind {
                TypeMemberKind::Property(p) => {
                    let Some(init) = &p.init else { continue };
                    emit_props(b, root);
                    if let Some(value) = b.build_expr(init) {
                        let field_ty = hir
                            .members
                            .get(&owner_fqn_sym)
                            .and_then(|mm| mm.get(&p.name.symbol))
                            .copied()
                            .unwrap_or(unit_ty);
                        let stmt = b.push_stmt(TreeStmt::Assign {
                            place: TreePlace::MemberField {
                                recv: this_ref,
                                owner_fqn: owner_fqn_sym,
                                name: p.name.symbol,
                                value_ty_hint: Some(field_ty),
                            },
                            value,
                        });
                        b.out.blocks[root.idx()].stmts.push(stmt);
                    }
                }
                TypeMemberKind::InitBlock(ib) => {
                    emit_props(b, root);
                    let block = b.build_block(&ib.body);
                    for st in b.out.blocks[block.idx()].stmts.clone() {
                        b.out.blocks[root.idx()].stmts.push(st);
                    }
                    if let Some(tail) = b.out.blocks[block.idx()].tail {
                        let stmt = b.push_stmt(TreeStmt::Expr(tail));
                        b.out.blocks[root.idx()].stmts.push(stmt);
                    }
                }
                _ => {}
            }
        }
        emit_props(b, root);
    };

    let primary_init_fqn = format!("{owner_fqn}.$init");
    match &sc.delegation {
        Some(del) => {
            use crate::syntax::ast::CtorDelegationKind;
            let (target_fqn, target_sig_owner) = match del.kind {
                CtorDelegationKind::This => {
                    let n_args = del.args.len();
                    let fqn =
                        resolve_this_delegation_target_tree(hir, owner_fqn_sym, &owner_fqn, n_args);
                    (fqn, owner_fqn_sym)
                }
                CtorDelegationKind::Super => {
                    let sd = hir.super_ctor_delegations.get(&owner_fqn_sym);
                    let super_fqn_text = sd
                        .map(|sd| hir.interner.resolve(sd.super_fqn).to_string())
                        .unwrap_or_default();
                    let fqn = format!("{}.$init", super_fqn_text);
                    let super_sym = sd.map(|sd| sd.super_fqn).unwrap_or_default();
                    (fqn, super_sym)
                }
            };
            let target_sig = hir.ctor_signatures.get(&target_sig_owner).and_then(|sigs| {
                let n_args = del.args.len();
                sigs.iter()
                    .find(|s| {
                        let min_arity = s
                            .has_defaults
                            .iter()
                            .position(|d| *d)
                            .unwrap_or(s.param_types.len());
                        n_args >= min_arity && n_args <= s.param_types.len()
                    })
                    .or_else(|| sigs.first())
            });
            let mut args = vec![this_ref];
            args.extend(fill_delegation_args(&mut b, del, target_sig));
            let is_super = matches!(del.kind, CtorDelegationKind::Super);
            emit_init_call(&mut b, root, target_fqn, args);
            if is_super {
                emit_class_steps(&mut b, root, this_ref);
            }
        }
        None => {
            // 无 delegation：调 primary $init（只传 this）。
            emit_init_call(&mut b, root, primary_init_fqn, vec![this_ref]);
        }
    }

    // secondary ctor body（块内联；尾值丢弃——镜像 lower_block 后弃值）。
    let body_block = b.build_block(&sc.body);
    for st in b.out.blocks[body_block.idx()].stmts.clone() {
        b.out.blocks[root.idx()].stmts.push(st);
    }
    if let Some(tail) = b.out.blocks[body_block.idx()].tail {
        let stmt = b.push_stmt(TreeStmt::Expr(tail));
        b.out.blocks[root.idx()].stmts.push(stmt);
    }

    b.out.root = Some(root);
    Some(FnTree {
        fqn: format!("{owner_fqn}.$ctor.s{}", sc.span.start),
        params: param_locals,
        body: b.out,
        gaps: b.gaps,
        val_init: None,
    })
}

/// 解析 `this(args)` 委托目标（镜像 resolve_this_delegation_target）。
fn resolve_this_delegation_target_tree(
    hir: &super::TypedHir,
    owner_fqn_sym: Symbol,
    owner_fqn: &str,
    n_args: usize,
) -> String {
    let primary_init_fqn = format!("{owner_fqn}.$init");
    let Some(sigs) = hir.ctor_signatures.get(&owner_fqn_sym) else {
        return primary_init_fqn;
    };
    let has_primary = hir.class_ctor_params.contains_key(&owner_fqn_sym);
    for (i, sig) in sigs.iter().enumerate() {
        let applicable = n_args <= sig.param_types.len()
            && sig.param_types.len() - n_args
                <= sig.has_defaults.iter().skip(n_args).filter(|d| **d).count();
        if !applicable {
            continue;
        }
        if i == 0 && has_primary {
            return primary_init_fqn;
        }
        return format!("{owner_fqn}.$ctor.s{}", sig.decl_span.start);
    }
    primary_init_fqn
}

/// owner 的声明态 nominal TypeId（`this` 类型）。
fn super_decl_ty(hir: &super::TypedHir, owner_sym: Symbol) -> Option<TypeId> {
    hir.type_infos
        .iter()
        .find(|(ty, _)| {
            matches!(
                hir.store.kind(**ty),
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                    | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
                    if n.fqn == owner_sym && n.args.is_empty()
            )
        })
        .map(|(&ty, _)| ty)
}

/// AST 整型后缀 → 树后缀。
fn tree_int_suffix(s: crate::syntax::ast::IntSuffix) -> TreeIntSuffix {
    match s {
        crate::syntax::ast::IntSuffix::U => TreeIntSuffix::U,
        crate::syntax::ast::IntSuffix::L => TreeIntSuffix::L,
        crate::syntax::ast::IntSuffix::UL => TreeIntSuffix::Ul,
    }
}
