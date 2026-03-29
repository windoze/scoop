//! 名字解析（name resolution）。
//!
//! Scoop 的完整名字解析会涉及：
//! - package/import
//! - 多命名空间（type/value）
//! - 作用域（块级、类型体、泛型参数、扩展 receiver 等）
//! - 可见性（public/internal/private）
//!
//! 当前阶段先落地最小子集：**顶层符号索引**。
//! - 把每个文件的 `package` + 顶层声明名组合成 FQN（Fully Qualified Name）
//! - 构建索引并检测重复定义

mod imports;
mod scopes;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::{ast, source::SourceFile, span::Span};

pub use imports::{ImportNamespace, ImportTable};
use scopes::check_block_scopes;

/// 单个文件的“声明头（headers）”解析产物。
///
/// 两阶段解析（T0308）的目标是把“声明头收集/校验”与“body/init 解析”解耦：
/// - phase 1：构建/校验 import 表、解析签名里的类型引用等（不进入函数体）
/// - phase 2：解析函数体与 initializer 中的值引用（后续可扩展到属性 init/accessor 等）
#[derive(Debug, Clone)]
pub struct FileHeaders {
    pub imports: ImportTable,
}

/// type params 的作用域栈（用于 resolve 阶段解析 `TypeRef`）。
///
/// 说明：
/// - 目前仅用于“类型引用存在性解析”（T0309）：当 `TypeRef` 是单段路径且命中某个 type param 时视为可解析；
/// - 嵌套声明允许 shadowing（类似 block scope），但同一声明的 type param 列表内不允许重名；
/// - 解析结果暂不写回 AST（后续 typecheck/HIR lowering 可能会需要更丰富的表示）。
#[derive(Debug, Default)]
struct TypeParamScopes {
    frames: Vec<HashMap<String, Span>>,
}

impl TypeParamScopes {
    fn new() -> Self {
        Self::default()
    }

    fn contains(&self, name: &str) -> bool {
        self.frames
            .iter()
            .rev()
            .any(|frame| frame.contains_key(name))
    }

    /// 压入一个“声明级” type param 作用域帧。
    ///
    /// 当前约束：同一帧内不允许重复定义（例如 `fun f<T, T>()`）。
    fn push_decl(
        &mut self,
        source: &SourceFile,
        params: &[ast::TypeParam],
    ) -> Result<(), ResolveError> {
        let mut frame: HashMap<String, Span> = HashMap::new();
        for p in params {
            let name = source.slice(p.name.span).to_string();
            if let Some(prev) = frame.get(&name).copied() {
                return Err(ResolveError::DuplicateDefinition {
                    name,
                    first: prev.into(),
                    second: p.name.span.into(),
                });
            }
            frame.insert(name, p.name.span);
        }
        self.frames.push(frame);
        Ok(())
    }

    fn pop_decl(&mut self) {
        let _ = self.frames.pop();
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum ResolveError {
    #[error("重复定义：{name}")]
    #[diagnostic(code(scoop::resolve::duplicate_definition))]
    DuplicateDefinition {
        name: String,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
    },

    #[error("未解析的 import：{import}")]
    #[diagnostic(code(scoop::resolve::unresolved_import))]
    UnresolvedImport {
        import: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未解析的类型：{name}")]
    #[diagnostic(code(scoop::resolve::unresolved_type))]
    UnresolvedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未解析的类型参数：{name}")]
    #[diagnostic(code(scoop::resolve::unresolved_type_param))]
    UnresolvedTypeParam {
        name: String,
        #[label("这里的类型参数不在当前声明的泛型参数列表中")]
        span: miette::SourceSpan,
    },

    #[error("未解析的值：{name}")]
    #[diagnostic(code(scoop::resolve::unresolved_value))]
    UnresolvedValue {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未解析的成员：{name}")]
    #[diagnostic(code(scoop::resolve::unresolved_member))]
    UnresolvedMember {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型没有 companion object：{ty}")]
    #[diagnostic(code(scoop::resolve::missing_companion_object))]
    MissingCompanionObject {
        ty: String,
        #[label("这里的类型没有 companion object，无法通过 `TypeName.member` 访问")]
        span: miette::SourceSpan,
    },

    #[error("初始化阶段非法前向引用：{name}")]
    #[diagnostic(code(scoop::resolve::forward_reference))]
    ForwardReference {
        name: String,
        #[label("这里引用了尚未初始化的成员")]
        use_span: miette::SourceSpan,
        #[label("该成员定义在这里")]
        def_span: miette::SourceSpan,
    },

    #[error("调用解析歧义：{name}")]
    #[diagnostic(code(scoop::resolve::ambiguous_call))]
    AmbiguousCall {
        name: String,
        #[label("这里的调用存在多个候选函数")]
        span: miette::SourceSpan,
    },

    #[error("符号不可见：{name}（{visibility}）")]
    #[diagnostic(code(scoop::resolve::not_visible))]
    NotVisible {
        name: String,
        visibility: Visibility,
        #[label("这里引用了不可见符号")]
        use_span: miette::SourceSpan,
        #[label("该符号定义在这里")]
        def_span: miette::SourceSpan,
    },

    #[error("非法的可见性修饰符组合（public/internal/private 只能出现一个）")]
    #[diagnostic(code(scoop::resolve::invalid_visibility))]
    InvalidVisibility {
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Fun,
    Type,
    Value,
}

/// Cone（编译包/分发单元）标识。
///
/// 说明：
/// - 该概念用于实现 `internal` 的“仅 cone 内可见”语义（spec §13.6）。
/// - 当前阶段 cone 仍是一个轻量概念：在同一次编译/resolve 构建的 `Index` 中，
///   不同 cone 只是用于可见性过滤与 fixtures 模拟依赖边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConeId(u32);

impl ConeId {
    pub const DEFAULT: ConeId = ConeId(0);

    pub fn new(raw: u32) -> ConeId {
        ConeId(raw)
    }
}

/// 可见性（visibility）。
///
/// 当前阶段（T0321a）：
/// - `public`：跨 cone 可见（公共 API）；
/// - `internal`：仅 cone 内可见（实现细节，不导出）；
/// - `private`：文件内可见（最小规则：顶层按 file-private 处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Internal,
    Private,
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Visibility::Public => "public",
            Visibility::Internal => "internal",
            Visibility::Private => "private",
        };
        write!(f, "{s}")
    }
}

/// 用于继承/override 语义检查的最小 modifiers 集合（T0439）。
///
/// 说明：
/// - `ast::Modifier` 在 parser 阶段仅做“解析并存储”，不带 span 信息；
/// - resolver 的 `Index` 需要在不依赖 AST 的情况下支持后续阶段查询（例如：override 目标是否 `open`）；
/// - 因此这里把少数关键修饰符降维为布尔标记，并存入 `Symbol`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierSet {
    pub open: bool,
    pub abstract_: bool,
    pub sealed: bool,
    pub override_: bool,
}

impl ModifierSet {
    pub fn from_modifiers(modifiers: &[ast::Modifier]) -> Self {
        let mut out = ModifierSet::default();
        for m in modifiers {
            match m {
                ast::Modifier::Open => out.open = true,
                ast::Modifier::Abstract => out.abstract_ = true,
                ast::Modifier::Sealed => out.sealed = true,
                ast::Modifier::Override => out.override_ = true,
                _ => {}
            }
        }
        out
    }

    /// 该符号是否允许被继承（class 语义）。
    pub fn is_inheritable(&self) -> bool {
        self.open || self.abstract_ || self.sealed
    }

    /// 该符号是否允许被 override（member 语义）。
    pub fn is_overridable(&self) -> bool {
        self.open || self.abstract_
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
    pub decl_file: PathBuf,
    pub decl_cone: ConeId,
    pub visibility: Visibility,
    pub modifiers: ModifierSet,
}

/// 用于 overload resolution 的“参数签名信息”（仅声明头）。
///
/// 说明：
/// - 这里刻意不保留 `default_value: Expr`，避免把表达式树复制进索引；
/// - `has_default` 足以支持后续“默认值参与候选可用性”的规则（PLAN §3.2 / TODO T03xx）。
#[derive(Debug, Clone)]
pub struct ParamSig {
    pub name: String,
    pub name_span: Span,
    pub ty: Option<ast::TypeRef>,
    pub has_default: bool,
}

/// Index 侧记录的函数 type parameter（仅名字与 span）。
///
/// 说明：
/// - `Index` 会被跨文件调用点查询，因此需要保留 type param 的名字，
///   以便后续 typecheck lowering 将 `T` 解析为 `TypeKind::Param`（而不是顶层同名 type）。
#[derive(Debug, Clone)]
pub struct TypeParamSig {
    pub name: String,
    pub name_span: Span,
}

/// Index 侧记录的“内建注解标记位”。
///
/// 说明：
/// - 仅覆盖 `@Unsafe/@NoGC/@Extern/@Intrinsic` 四个会影响早期 typecheck 的注解；
/// - `@Extern` 在语义上隐含 `@NoGC`（spec §15.8.3），因此这里会折叠到 `is_nogc = true`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinFunFlags {
    pub is_unsafe: bool,
    pub is_nogc: bool,
    pub is_extern: bool,
    pub is_intrinsic: bool,
}

fn builtin_fun_flags_from_annotations(
    source: &SourceFile,
    anns: &[ast::AnnotationUse],
) -> BuiltinFunFlags {
    let mut out = BuiltinFunFlags::default();

    for ann in anns {
        let segs = ann
            .path
            .iter()
            .map(|id| id.text(source))
            .collect::<Vec<_>>();

        match segs.as_slice() {
            ["Unsafe"] | ["scoop", "core", "Unsafe"] => out.is_unsafe = true,
            ["NoGC"] | ["scoop", "core", "NoGC"] => out.is_nogc = true,
            ["Extern"] | ["scoop", "core", "Extern"] => out.is_extern = true,
            ["Intrinsic"] | ["scoop", "core", "Intrinsic"] => out.is_intrinsic = true,
            _ => {}
        }
    }

    // spec §15.8.3：`@Extern` 默认视为 `@NoGC`。
    if out.is_extern {
        out.is_nogc = true;
    }

    out
}

/// 一个函数声明的“可用于重载决议”的签名信息（仅声明头）。
#[derive(Debug, Clone)]
pub struct FunSig {
    pub kind: ast::FunDeclKind,
    /// 是否为 `const fun`（spec §6.2）。
    ///
    /// 说明：
    /// - 该标记用于让后续 typecheck/lowering 在**不依赖 AST** 的情况下判断“跨文件调用点”
    ///   是否允许出现在 `const fun` 语境中；
    /// - `const fun` 的更完整静态约束由 typecheck 负责（TODO T1211）。
    pub is_const: bool,
    pub receiver: Option<ast::TypeRef>,
    pub type_params: Vec<TypeParamSig>,
    pub eff_param: Option<ast::EffectRowParam>,
    pub params: Vec<ParamSig>,
    pub return_ty: Option<ast::TypeRef>,
    pub effects: Option<ast::EffectRowExpr>,
    pub builtin_flags: BuiltinFunFlags,
}

/// fun 命名空间中的一个 overload 候选。
#[derive(Debug, Clone)]
pub struct FunOverload {
    pub symbol: Symbol,
    pub sig: FunSig,
    /// 该函数是否带有 body（`{ ... }` / `= expr`）。
    ///
    /// 用途：
    /// - interface 的默认方法 vs 抽象方法区分（T0440）：
    ///   - `has_body = false` → 需要实现（抽象成员）
    ///   - `has_body = true` → 可不实现（默认实现）
    pub has_body: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorKind {
    Primary,
    Secondary,
}

/// 构造函数 overload 候选（primary + secondary）。
#[derive(Debug, Clone)]
pub struct ConstructorOverload {
    pub kind: ConstructorKind,
    pub decl_file: PathBuf,
    pub decl_cone: ConeId,
    pub visibility: Visibility,
    pub span: Span,
    pub params: Vec<ParamSig>,
}

/// Index 侧记录的扩展函数信息（T0312）。
///
/// 说明：
/// - 扩展函数在语法上仍是“顶层 fun 声明”，因此其符号本体仍存放在 `by_fqn` 的 fun 命名空间里；
/// - 这里额外记录 receiver 的“可用于匹配的类型 FQN”，用于把 `receiver.member()` 的 `member`
///   在无同名 member 时解析到 extension；
/// - 当前阶段仅用于 name resolution（不做重载/最具体匹配），并且只会在**同包**内查找扩展声明。
#[derive(Debug, Clone)]
pub struct ExtensionFunSymbol {
    /// 扩展函数自身的 FQN（例如 `a.ext`）。
    pub fqn: String,
    /// 该扩展函数声明所在文件的 package 前缀（例如 `a`；无 package 时为空）。
    pub pkg_prefix: String,
    /// 该扩展函数所在 cone（用于 `internal` 过滤与同包扩展查找的 cone 边界）。
    pub decl_cone: ConeId,
    /// member 名称（即扩展函数的声明名，例如 `ext`）。
    pub name: String,
    /// receiver 的类型 FQN（例如 `a.Point` / `scoop.core.Any`）；无法解析时为 None。
    pub receiver_ty_fqn: Option<String>,
    /// receiver 是否为声明处的类型参数（例如 `fun <T> T.ext()`）。
    ///
    /// 说明：
    /// - 当前 resolver 的扩展函数匹配主要依赖 receiver 的类型 FQN；
    /// - 但当 receiver 是 type param 时无法映射到具体 FQN（`receiver_ty_fqn=None`），
    ///   语义上它应当可作用于任意 receiver，因此这里把它标记为“通配 receiver”以参与候选收集。
    pub receiver_is_type_param: bool,
}

/// 同一个 FQN 下按命名空间（type/value/fun）分组的符号集合。
///
/// 说明：
/// - 语言层面允许 **同名 type 与 fun/value 并存**（它们属于不同命名空间）。
/// - type/value 命名空间内：同名符号仍视为重复定义并报错；
/// - fun 命名空间内：同名函数允许作为 overload set 共存（T0318），真正冲突留给后续签名比较与 typecheck。
#[derive(Debug, Default, Clone)]
pub struct NamespacedSymbols {
    pub ty: Option<Symbol>,
    pub fun: Vec<FunOverload>,
    pub value: Option<Symbol>,
}

impl NamespacedSymbols {
    fn slot_mut(&mut self, kind: SymbolKind) -> &mut Option<Symbol> {
        match kind {
            SymbolKind::Type => &mut self.ty,
            SymbolKind::Value => &mut self.value,
            SymbolKind::Fun => {
                unreachable!("fun 命名空间使用 overload set（Vec<FunOverload>）存储")
            }
        }
    }

    fn get(&self, kind: SymbolKind) -> Option<&Symbol> {
        match kind {
            SymbolKind::Type => self.ty.as_ref(),
            SymbolKind::Value => self.value.as_ref(),
            SymbolKind::Fun => self.fun.first().map(|o| &o.symbol),
        }
    }

    fn has_fun(&self) -> bool {
        !self.fun.is_empty()
    }

    fn any_visible_fun(&self, use_cone: ConeId, use_source: &SourceFile) -> Option<&FunOverload> {
        self.fun
            .iter()
            .find(|o| is_symbol_visible_from(use_cone, use_source, &o.symbol))
    }

    fn first_fun(&self) -> Option<&FunOverload> {
        self.fun.first()
    }
}

/// 一个编译单元（多个文件）的顶层符号索引。
#[derive(Debug, Default)]
pub struct Index {
    /// FQN（例如 `scoop.core.Option`）→ 按命名空间分组的符号集合。
    pub by_fqn: HashMap<String, NamespacedSymbols>,
    /// 每个源文件所属的 cone（用于可见性过滤）。
    file_cones: HashMap<PathBuf, ConeId>,
    /// program boundary：额外的入口函数集合（库导出入口 / host entry points，T0629b）。
    ///
    /// 说明：
    /// - 该集合的来源由 driver 决定（例如 Cone.toml 的 `[entry-points].exports`）；
    /// - 当前仅在 typecheck 阶段用于决定哪些顶层 `fun` 需要按 entry point 规则强制 `Pure!`。
    export_entry_points: HashSet<String>,
    /// 构造函数 overload set：type FQN → primary/secondary constructors（T0318）。
    pub constructors: HashMap<String, Vec<ConstructorOverload>>,
    /// 扩展函数集合（用于成员访问的 extension fallback，T0312）。
    pub extension_funs: Vec<ExtensionFunSymbol>,
    /// 类型（class/struct/...）的 companion object FQN 列表（T0317）。
    ///
    /// key：宿主类型的 FQN（例如 `a.C`）
    /// value：该类型声明中的 companion object 的 FQN（例如 `a.C.Companion` / `a.C.Named`）
    pub companion_objects: HashMap<String, Vec<String>>,
    /// 全部 `object`（含 companion object）的类型 FQN 集合（T0317）。
    ///
    /// 用途：在成员访问解析时区分：
    /// - object 单例值：`Obj.member`
    /// - enum 作为 value namespace：`Enum.Variant`（其符号注入留给后续任务）
    pub object_types: HashSet<String>,
}

/// `Index` 构建输入：一个源文件 + AST，以及它所属的 cone。
#[derive(Debug, Clone, Copy)]
pub struct IndexedFile<'a> {
    pub cone: ConeId,
    pub source: &'a SourceFile,
    pub file: &'a ast::File,
}

impl Index {
    pub fn build(files: &[(&SourceFile, &ast::File)]) -> Result<Self, ResolveError> {
        let owned = files
            .iter()
            .map(|(source, file)| IndexedFile {
                cone: ConeId::DEFAULT,
                source,
                file,
            })
            .collect::<Vec<_>>();
        Index::build_with_cones(&owned)
    }

    pub fn build_with_cones(files: &[IndexedFile<'_>]) -> Result<Self, ResolveError> {
        let mut index = Index::default();
        for f in files {
            index
                .file_cones
                .insert(f.source.path().to_path_buf(), f.cone);
            index.add_file_in_cone(f.cone, f.source, f.file)?;
        }
        index.collect_extension_funs(files);
        Ok(index)
    }

    /// 设置“导出入口（export entry points）”集合（T0629b）。
    pub fn set_export_entry_points<I, S>(&mut self, fqns: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.export_entry_points = fqns.into_iter().map(Into::into).collect();
    }

    pub fn is_export_entry_point(&self, fqn: &str) -> bool {
        self.export_entry_points.contains(fqn)
    }

    fn collect_extension_funs(&mut self, files: &[IndexedFile<'_>]) {
        self.extension_funs.clear();

        for f in files {
            let pkg_prefix = package_prefix(f.source, f.file.package.as_ref());

            for item in &f.file.items {
                let ast::Item::Fun(fun) = item else {
                    continue;
                };

                let Some(receiver) = &fun.receiver else {
                    continue;
                };

                let name = f.source.slice(fun.name.span).to_string();
                let fqn = if pkg_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{pkg_prefix}.{name}")
                };

                let receiver_ty_fqn = self.type_ref_to_fqn_in_file(f.source, f.file, receiver);
                let receiver_is_type_param = receiver_ty_fqn.is_none()
                    && match receiver {
                        ast::TypeRef::Path(p) => {
                            // `fun <T> T.ext()`：receiver 是 type param（通配）。
                            if p.segments.len() != 1 || !p.args.is_empty() {
                                false
                            } else {
                                let seg = &p.segments[0];
                                let receiver_name = match seg.text {
                                    Some(t) => t,
                                    None => f.source.slice(seg.span),
                                };
                                fun.type_params.iter().any(|tp| {
                                    let tp_name = match tp.name.text {
                                        Some(t) => t,
                                        None => f.source.slice(tp.name.span),
                                    };
                                    tp_name == receiver_name
                                })
                            }
                        }
                        _ => false,
                    };

                self.extension_funs.push(ExtensionFunSymbol {
                    fqn,
                    pkg_prefix: pkg_prefix.clone(),
                    decl_cone: f.cone,
                    name,
                    receiver_ty_fqn,
                    receiver_is_type_param,
                });
            }
        }
    }

    pub(crate) fn cone_of_source(&self, source: &SourceFile) -> ConeId {
        self.file_cones
            .get(source.path())
            .copied()
            .unwrap_or(ConeId::DEFAULT)
    }

    /// 返回当前编译单元的“consumer cone”（用于 program boundary / entry point 规则）。
    ///
    /// 约定（与 fixtures runner 对齐）：
    /// - `ConeId::DEFAULT`（0）通常用于“无 cone 区分”的单包编译，或 sysroot；
    /// - 若存在非 0 cone，则选择 **最小的非 0 cone id** 作为 consumer cone（稳定、可预测）。
    ///
    /// 说明：真实的 build/link 流程（TODO T1107）未来可能会显式指定“当前被构建的 cone”，
    /// 但在早期阶段（含 fixtures 模拟）我们先用该稳定规则避免把依赖 cone 的 `main` 误判为 entry point。
    pub(crate) fn consumer_cone(&self) -> ConeId {
        let mut min: Option<ConeId> = None;
        for cone in self.file_cones.values().copied() {
            if cone == ConeId::DEFAULT {
                continue;
            }
            min = Some(match min {
                Some(prev) if prev.0 <= cone.0 => prev,
                _ => cone,
            });
        }
        min.unwrap_or(ConeId::DEFAULT)
    }

    pub(crate) fn type_ref_to_fqn_in_file(
        &self,
        source: &SourceFile,
        file: &ast::File,
        ty: &ast::TypeRef,
    ) -> Option<String> {
        match ty {
            ast::TypeRef::Path(p) => self.type_path_to_fqn_in_file(source, file, p),
            _ => None,
        }
    }

    fn type_path_to_fqn_in_file(
        &self,
        source: &SourceFile,
        file: &ast::File,
        path: &ast::TypePath,
    ) -> Option<String> {
        let segments = path
            .segments
            .iter()
            // 支持 parser 生成的合成 Ident（例如 try/catch lowering 中的 `scoop.core.Raise`）：
            // 后续 resolve/typecheck 应优先使用 Ident 内部携带的字面文本，而不是 span 回切。
            .map(|id| id.text(source))
            .collect::<Vec<_>>();
        let local = segments.join(".");

        let pkg_prefix = package_prefix(source, file.package.as_ref());
        let mut candidates: Vec<String> = Vec::new();

        // 1) 同包优先：pkg + local
        if !pkg_prefix.is_empty() {
            candidates.push(format!("{pkg_prefix}.{local}"));
        }

        // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
        candidates.push(local.clone());

        // 3) 对单段名字，应用 import 规则（显式 import / star import）
        if segments.len() == 1 {
            let name = segments[0];
            for import in &file.imports {
                let import_path = import
                    .path
                    .iter()
                    .map(|id| id.text(source))
                    .collect::<Vec<_>>()
                    .join(".");

                if import.has_star {
                    candidates.push(format!("{import_path}.{name}"));
                } else {
                    let local = import
                        .alias
                        .as_ref()
                        .map(|id| id.text(source))
                        .or_else(|| import.path.last().map(|id| id.text(source)))
                        .unwrap_or("");
                    if local == name {
                        candidates.push(import_path);
                    }
                }
            }
        }

        candidates.sort();
        candidates.dedup();

        let use_cone = self.cone_of_source(source);
        for fqn in candidates {
            let Some(syms) = self.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.get(SymbolKind::Type) else {
                continue;
            };
            if is_symbol_visible_from(use_cone, source, sym) {
                return Some(fqn);
            }
        }

        None
    }

    fn add_file_in_cone(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        file: &ast::File,
    ) -> Result<(), ResolveError> {
        let pkg = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    // typealias 是类型命名空间的顶层符号（T0251）。
                    let visibility = visibility_from_modifiers(&ta.modifiers, ta.span)?;
                    self.insert_symbol(
                        cone,
                        source,
                        &pkg,
                        SymbolKind::Type,
                        ta.name.span,
                        visibility,
                        &ta.modifiers,
                    )?;
                }
                ast::Item::Fun(fun) => {
                    let visibility = visibility_from_modifiers(&fun.modifiers, fun.span)?;
                    self.insert_fun_overload(cone, source, &pkg, fun, visibility)?;
                }
                ast::Item::Type(ty) => {
                    self.add_type_decl(cone, source, &pkg, ty)?;
                }
                ast::Item::Object(obj) => {
                    self.add_object_decl(cone, source, &pkg, obj)?;
                }
                ast::Item::Val(v) => {
                    // 顶层 `val/var` 必须有名字；解构绑定仅在 block 内作为语句出现（T0244）。
                    if let Some(name) = v.name() {
                        let visibility = visibility_from_modifiers(&v.modifiers, v.span)?;
                        self.insert_symbol(
                            cone,
                            source,
                            &pkg,
                            SymbolKind::Value,
                            name.span,
                            visibility,
                            &v.modifiers,
                        )?;
                    }
                }
                ast::Item::ExtensionProperty(_p) => {
                    // 顶层扩展属性本身不引入“顶层 value 名称”（它只能通过 member access 访问）。
                    // extension fallback 与 lowering 规则由后续任务补齐（TODO T0433/T0436）。
                }
            }
        }

        Ok(())
    }

    /// 把一个类型声明（顶层或嵌套）加入索引，并递归纳入其类型体成员（T0302）。
    ///
    /// `prefix` 表示该类型所在的容器前缀：
    /// - 顶层类型：prefix = package 前缀（可能为空）
    /// - 嵌套类型：prefix = 外层类型的 FQN（例如 `a.Outer`）
    fn add_type_decl(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        prefix: &str,
        ty: &ast::TypeDecl,
    ) -> Result<(), ResolveError> {
        // 1) 先插入类型自身（type namespace）。
        let visibility = visibility_from_modifiers(&ty.modifiers, ty.span)?;
        self.insert_symbol(
            cone,
            source,
            prefix,
            SymbolKind::Type,
            ty.name.span,
            visibility,
            &ty.modifiers,
        )?;

        // enum 在语义上需要暴露一个“类型名同名的 value”（类似 Kotlin 的 `EnumClass` 作为命名空间），
        // 以支持 `RuntimeError.NullAssertionFailed` 这类对枚举值的限定引用（spec §5.7）。
        //
        // 当前阶段我们仅把这个符号放入 value namespace 以解锁名字解析；更完整的 enum/variant 语义
        // 会在后续 rich enum 任务中补齐（T0425+）。
        if matches!(ty.kind, ast::TypeKind::Enum) {
            self.insert_symbol(
                cone,
                source,
                prefix,
                SymbolKind::Value,
                ty.name.span,
                visibility,
                &ty.modifiers,
            )?;
        }

        // 2) 递归处理类型体成员：fields/methods/nested types。
        let type_name = source.slice(ty.name.span);
        let type_prefix = if prefix.is_empty() {
            type_name.to_string()
        } else {
            format!("{prefix}.{type_name}")
        };

        // T0318：构造函数 overload set（primary + secondary）。
        if let Some(primary_ctor) = &ty.primary_ctor {
            self.insert_constructor_overload(
                &type_prefix,
                cone,
                source,
                ConstructorKind::Primary,
                visibility,
                primary_ctor.params_span,
                &primary_ctor.params,
            );
        }

        // struct 的主构造参数在语义上等价于字段（spec §2.3.1），
        // 允许通过 `p.x` 的成员访问读取；因此需要把它们纳入 value namespace 索引。
        //
        // 说明：
        // - 当前 parser 允许但不在 AST 中表达 `val/var` ctor param；对 struct 而言我们先保守地把
        //   所有 ctor params 视为字段（后续若扩展 class 语义再细化规则）。
        // - ctor params 暂不支持显式可见性修饰符，因此默认 public（与无修饰的成员一致）。
        if matches!(ty.kind, ast::TypeKind::Struct) {
            if let Some(primary_ctor) = &ty.primary_ctor {
                for p in &primary_ctor.params {
                    self.insert_symbol(
                        cone,
                        source,
                        &type_prefix,
                        SymbolKind::Value,
                        p.name.span,
                        Visibility::Public,
                        &[],
                    )?;
                }
            }
        }

        // class 的主构造参数仅在带 `val/var` 前缀时才声明字段/属性：
        // - `class C(x: Int)`：`x` 只是构造参数，不应可通过 `this.x` 成员访问读取
        // - `class C(val x: Int)`：`x` 作为字段/属性，需进入 value namespace 索引
        //
        // 说明：
        // - 当前阶段暂不处理 ctor param 上的可见性修饰符（语法也未支持），因此默认 public。
        if matches!(ty.kind, ast::TypeKind::Class) {
            if let Some(primary_ctor) = &ty.primary_ctor {
                for p in &primary_ctor.params {
                    if p.kind.is_none() {
                        continue;
                    }
                    self.insert_symbol(
                        cone,
                        source,
                        &type_prefix,
                        SymbolKind::Value,
                        p.name.span,
                        Visibility::Public,
                        &[],
                    )?;
                }
            }
        }

        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    // enum variant 值需要注入到 value namespace：
                    // - `EnumName.Variant`（限定名引用）应当可解析（spec §5.7 / T0419）。
                    //
                    // 当前阶段（最小落点）：
                    // - 仅注入 0-参数（unit）variant 作为“值”；
                    // - 带 payload 的 variant 构造（`Some(x)` / `Enum.Some(x)`）的完整符号建模与重载规则
                    //   留给后续 rich enum 任务（T0425+）。
                    if v.params.is_empty() {
                        // 注意：这里刻意不在 resolver 阶段对“重复 variant 名称”报错：
                        // - typecheck 的 `TypeEnv` 会以更稳定的错误码（`duplicate_enum_variant`）报告该问题；
                        // - resolver 侧只需要保证“可解析的最小符号骨架”存在即可。
                        //
                        // 因此若插入时遇到同名冲突（DuplicateDefinition），这里选择忽略并继续，
                        // 让 typecheck 再给出更精确的诊断。
                        let inserted = self.insert_symbol(
                            cone,
                            source,
                            &type_prefix,
                            SymbolKind::Value,
                            v.name.span,
                            visibility,
                            &[],
                        );
                        match inserted {
                            Ok(()) => {}
                            Err(ResolveError::DuplicateDefinition { .. }) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    let visibility = visibility_from_modifiers(&p.modifiers, p.span)?;
                    self.insert_symbol(
                        cone,
                        source,
                        &type_prefix,
                        SymbolKind::Value,
                        p.name.span,
                        visibility,
                        &p.modifiers,
                    )?;
                }
                ast::TypeMember::InitBlock(_b) => {
                    // init block 不引入命名空间符号；它属于初始化执行体（Appendix B.2.2）。
                }
                ast::TypeMember::SecondaryCtor(_ctor) => {
                    // T0318：secondary constructor 进入该 type 的 constructors overload set。
                    let ctor = _ctor;
                    let ctor_visibility = visibility_from_modifiers(&ctor.modifiers, ctor.span)?;
                    self.insert_constructor_overload(
                        &type_prefix,
                        cone,
                        source,
                        ConstructorKind::Secondary,
                        ctor_visibility,
                        ctor.span,
                        &ctor.params,
                    );
                }
                ast::TypeMember::Fun(f) => {
                    let visibility = visibility_from_modifiers(&f.modifiers, f.span)?;
                    self.insert_fun_overload(cone, source, &type_prefix, f, visibility)?;
                }
                ast::TypeMember::Type(nested) => {
                    self.add_type_decl(cone, source, &type_prefix, nested)?;
                }
                ast::TypeMember::Object(obj) => {
                    if matches!(obj.kind, ast::ObjectKind::Companion) {
                        let companion_name = obj
                            .name
                            .as_ref()
                            .map(|id| source.slice(id.span).to_string())
                            .unwrap_or_else(|| "Companion".to_string());
                        let companion_fqn = format!("{type_prefix}.{companion_name}");
                        self.companion_objects
                            .entry(type_prefix.clone())
                            .or_default()
                            .push(companion_fqn);
                    }
                    self.add_object_decl(cone, source, &type_prefix, obj)?;
                }
            }
        }

        Ok(())
    }

    fn add_object_decl(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        prefix: &str,
        obj: &ast::ObjectDecl,
    ) -> Result<(), ResolveError> {
        let visibility = visibility_from_modifiers(&obj.modifiers, obj.span)?;

        let (obj_name, obj_name_span) = match &obj.name {
            Some(name) => (source.slice(name.span).to_string(), Some(name.span)),
            None => {
                if !matches!(obj.kind, ast::ObjectKind::Companion) {
                    // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底。
                    return Ok(());
                }
                // Kotlin-like：未命名 companion object 具有隐式名字 `Companion`，用于索引与成员访问（T0317）。
                ("Companion".to_string(), None)
            }
        };

        // Kotlin-like：object 声明同时引入一个“类型名”与一个“单例值名”。
        if let Some(name_span) = obj_name_span {
            self.insert_symbol(
                cone,
                source,
                prefix,
                SymbolKind::Type,
                name_span,
                visibility,
                &obj.modifiers,
            )?;
            self.insert_symbol(
                cone,
                source,
                prefix,
                SymbolKind::Value,
                name_span,
                visibility,
                &obj.modifiers,
            )?;
        } else {
            self.insert_synth_symbol(
                cone,
                source,
                prefix,
                SymbolKind::Type,
                &obj_name,
                obj.span,
                visibility,
                &obj.modifiers,
            )?;
            self.insert_synth_symbol(
                cone,
                source,
                prefix,
                SymbolKind::Value,
                &obj_name,
                obj.span,
                visibility,
                &obj.modifiers,
            )?;
        }

        let obj_prefix = if prefix.is_empty() {
            obj_name.clone()
        } else {
            format!("{prefix}.{obj_name}")
        };

        // 记录 object 的“类型身份”，用于成员访问时判断该值是否为 object 单例（T0317）。
        self.object_types.insert(obj_prefix.clone());

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(_v) => {}
                ast::TypeMember::Property(p) => {
                    let visibility = visibility_from_modifiers(&p.modifiers, p.span)?;
                    self.insert_symbol(
                        cone,
                        source,
                        &obj_prefix,
                        SymbolKind::Value,
                        p.name.span,
                        visibility,
                        &p.modifiers,
                    )?;
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(_ctor) => {
                    // object 不应有构造器；这里作为防御性兜底忽略。
                }
                ast::TypeMember::Fun(f) => {
                    let visibility = visibility_from_modifiers(&f.modifiers, f.span)?;
                    self.insert_fun_overload(cone, source, &obj_prefix, f, visibility)?;
                }
                ast::TypeMember::Type(nested) => {
                    self.add_type_decl(cone, source, &obj_prefix, nested)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.add_object_decl(cone, source, &obj_prefix, nested)?;
                }
            }
        }

        Ok(())
    }

    fn insert_synth_symbol(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        pkg_prefix: &str,
        kind: SymbolKind,
        local: &str,
        decl_span: Span,
        visibility: Visibility,
        modifiers: &[ast::Modifier],
    ) -> Result<(), ResolveError> {
        debug_assert!(
            kind != SymbolKind::Fun,
            "fun 命名空间必须使用 insert_fun_overload"
        );
        let fqn = if pkg_prefix.is_empty() {
            local.to_string()
        } else {
            format!("{pkg_prefix}.{local}")
        };

        let symbol = Symbol {
            kind,
            name: local.to_string(),
            span: decl_span,
            decl_file: source.path().to_path_buf(),
            decl_cone: cone,
            visibility,
            modifiers: ModifierSet::from_modifiers(modifiers),
        };

        let entry = self.by_fqn.entry(fqn.clone()).or_default();
        if let Some(prev) = entry.get(kind) {
            return Err(ResolveError::DuplicateDefinition {
                name: fqn,
                first: prev.span.into(),
                second: decl_span.into(),
            });
        }

        *entry.slot_mut(kind) = Some(symbol);
        Ok(())
    }

    fn insert_constructor_overload(
        &mut self,
        type_fqn: &str,
        cone: ConeId,
        source: &SourceFile,
        kind: ConstructorKind,
        visibility: Visibility,
        span: Span,
        params: &[ast::Param],
    ) {
        let decl_file = source.path().to_path_buf();
        let params = params
            .iter()
            .map(|p| ParamSig {
                name: source.slice(p.name.span).to_string(),
                name_span: p.name.span,
                ty: p.ty.clone(),
                has_default: p.default_value.is_some(),
            })
            .collect::<Vec<_>>();

        self.constructors
            .entry(type_fqn.to_string())
            .or_default()
            .push(ConstructorOverload {
                kind,
                decl_file,
                decl_cone: cone,
                visibility,
                span,
                params,
            });
    }

    fn insert_fun_overload(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        pkg_prefix: &str,
        fun: &ast::FunDecl,
        visibility: Visibility,
    ) -> Result<(), ResolveError> {
        let local = source.slice(fun.name.span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            local.clone()
        } else {
            format!("{pkg_prefix}.{local}")
        };

        let symbol = Symbol {
            kind: SymbolKind::Fun,
            name: local,
            span: fun.name.span,
            decl_file: source.path().to_path_buf(),
            decl_cone: cone,
            visibility,
            modifiers: ModifierSet::from_modifiers(&fun.modifiers),
        };

        let params = fun
            .params
            .iter()
            .map(|p| ParamSig {
                name: source.slice(p.name.span).to_string(),
                name_span: p.name.span,
                ty: p.ty.clone(),
                has_default: p.default_value.is_some(),
            })
            .collect::<Vec<_>>();

        // T0623：`async fun foo(): T` 对外暴露 `Task<T>`（而不是 `T / Async`）。
        //
        // 说明：
        // - `Index` 会被跨文件调用点查询，因此这里需要把“降糖后的返回类型”写入签名；
        // - 函数体内部的返回类型检查仍以 AST 上的 `return_ty`（T）为准（由 typecheck 处理）。
        let return_ty = if fun.modifiers.contains(&ast::Modifier::Async) {
            let synth_span = fun.return_ty.as_ref().map(|t| t.span()).unwrap_or(fun.span);
            let inner = fun.return_ty.clone().unwrap_or_else(|| {
                ast::TypeRef::Path(ast::TypePath {
                    span: synth_span,
                    segments: vec![
                        ast::Ident::synthetic(synth_span, "scoop"),
                        ast::Ident::synthetic(synth_span, "core"),
                        ast::Ident::synthetic(synth_span, "Unit"),
                    ],
                    args: Vec::new(),
                })
            });

            Some(ast::TypeRef::Path(ast::TypePath {
                span: synth_span,
                segments: vec![
                    ast::Ident::synthetic(synth_span, "scoop"),
                    ast::Ident::synthetic(synth_span, "core"),
                    ast::Ident::synthetic(synth_span, "Task"),
                ],
                args: vec![inner],
            }))
        } else {
            fun.return_ty.clone()
        };

        let sig = FunSig {
            kind: fun.kind,
            is_const: fun.modifiers.contains(&ast::Modifier::Const),
            receiver: fun.receiver.clone(),
            type_params: fun
                .type_params
                .iter()
                .map(|p| TypeParamSig {
                    name: p.name.text(source).to_string(),
                    name_span: p.name.span,
                })
                .collect::<Vec<_>>(),
            eff_param: fun.eff_param.clone(),
            params,
            return_ty,
            effects: fun.effects.clone(),
            builtin_flags: builtin_fun_flags_from_annotations(source, &fun.annotations),
        };

        let entry = self.by_fqn.entry(fqn).or_default();
        let has_body = !matches!(fun.body, ast::FunBody::Missing);
        entry.fun.push(FunOverload {
            symbol,
            sig,
            has_body,
        });
        Ok(())
    }

    fn insert_symbol(
        &mut self,
        cone: ConeId,
        source: &SourceFile,
        pkg_prefix: &str,
        kind: SymbolKind,
        name_span: Span,
        visibility: Visibility,
        modifiers: &[ast::Modifier],
    ) -> Result<(), ResolveError> {
        debug_assert!(
            kind != SymbolKind::Fun,
            "fun 命名空间必须使用 insert_fun_overload"
        );
        let local = source.slice(name_span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            local.clone()
        } else {
            format!("{pkg_prefix}.{local}")
        };

        let symbol = Symbol {
            kind,
            name: local,
            span: name_span,
            decl_file: source.path().to_path_buf(),
            decl_cone: cone,
            visibility,
            modifiers: ModifierSet::from_modifiers(modifiers),
        };

        let entry = self.by_fqn.entry(fqn.clone()).or_default();
        if let Some(prev) = entry.get(kind) {
            return Err(ResolveError::DuplicateDefinition {
                name: fqn,
                first: prev.span.into(),
                second: name_span.into(),
            });
        }

        *entry.slot_mut(kind) = Some(symbol);
        Ok(())
    }
}

fn visibility_from_modifiers(
    modifiers: &[ast::Modifier],
    decl_span: Span,
) -> Result<Visibility, ResolveError> {
    let mut found: Option<Visibility> = None;
    for m in modifiers {
        let vis = match m {
            ast::Modifier::Public => Some(Visibility::Public),
            ast::Modifier::Internal => Some(Visibility::Internal),
            ast::Modifier::Private => Some(Visibility::Private),
            _ => None,
        };

        let Some(vis) = vis else {
            continue;
        };

        if let Some(prev) = found {
            if prev != vis {
                return Err(ResolveError::InvalidVisibility {
                    span: decl_span.into(),
                });
            }
        } else {
            found = Some(vis);
        }
    }

    Ok(found.unwrap_or(Visibility::Public))
}

fn is_symbol_visible_from(use_cone: ConeId, use_source: &SourceFile, symbol: &Symbol) -> bool {
    match symbol.visibility {
        Visibility::Public => true,
        Visibility::Internal => symbol.decl_cone == use_cone,
        Visibility::Private => symbol.decl_file == use_source.path(),
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

/// 在 `Index` 的基础上，做最小的文件级名字绑定检查：
/// - import 的目标是否存在
/// - 函数签名/顶层 val/var 的类型引用是否可解析（仅 TypeRef::Path）
/// - （T0305）对表达式中的裸标识符（`ExprKind::Ident`）做解析并写回到 AST
///
/// 当前阶段的简化：
/// - 类型引用：只做存在性解析（type namespace），不做泛型 arity/alias 展开等深层语义
/// - 值引用：仅解析裸 `ident`（先局部/参数，再同包或 import 引入的顶层 fun/value），不解析成员访问与调用目标
/// - 可见性（T0306）：仅实现顶层 `private` 的“文件内可见”规则（跨文件引用报错）；`internal` 的 cone/module 语义后续补齐
/// - 不做重载/跨文件编译单元等复杂规则（后续任务补齐）
pub fn check_file_bindings(
    source: &SourceFile,
    file: &mut ast::File,
    index: &Index,
) -> Result<(), ResolveError> {
    // T0308：两阶段解析（headers → bodies/init）。
    let headers = check_file_headers(source, file, index)?;
    check_file_bodies(source, file, index, &headers)?;

    Ok(())
}

/// Phase 1：解析并校验“声明头”信息（不进入函数体与 initializer）。
pub fn check_file_headers(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
) -> Result<FileHeaders, ResolveError> {
    // T0303：构建 import 表并验证 import 目标存在性（type/value 两套命名空间）。
    let imports = ImportTable::build(source, file, index)?;

    // T1015：命名空间注解（`@A.B`）的最小存在性解析。
    resolve_namespaced_annotations(source, file, index, &file.file_annotations)?;

    // T0309：声明级 type params 作用域（用于签名中的 TypeRef 解析）。
    let mut type_params = TypeParamScopes::new();

    // 解析签名里的类型引用（type/function/field signatures）。
    // 说明：当前阶段仍以“存在性解析”为主；更深层的泛型/alias 语义交给 typecheck。
    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                resolve_namespaced_annotations(source, file, index, &ta.annotations)?;
                type_params.push_decl(source, &ta.type_params)?;
                let result = resolve_type_ref(source, file, index, &type_params, None, &ta.ty);
                type_params.pop_decl();
                result?
            }
            ast::Item::Fun(fun) => {
                type_params.push_decl(source, &fun.type_params)?;
                let eff_param = fun.eff_param.as_ref().map(|p| source.slice(p.name.span));
                let result =
                    (|| resolve_fun_header(source, file, index, &type_params, eff_param, fun))();
                type_params.pop_decl();
                result?;
            }
            ast::Item::ExtensionProperty(p) => {
                type_params.push_decl(source, &p.type_params)?;
                let result = (|| {
                    resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                    resolve_type_ref(source, file, index, &type_params, None, &p.receiver)?;
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, &type_params, None, ty)?;
                    }
                    Ok(())
                })();
                type_params.pop_decl();
                result?;
            }
            ast::Item::Val(v) => {
                resolve_namespaced_annotations(source, file, index, &v.annotations)?;
                if let Some(ty) = &v.ty {
                    resolve_type_ref(source, file, index, &type_params, None, ty)?;
                }
            }
            ast::Item::Type(ty) => {
                resolve_type_decl_headers(source, file, index, ty, &mut type_params)?
            }
            ast::Item::Object(obj) => resolve_object_decl_headers(source, file, index, obj)?,
        }
    }

    Ok(FileHeaders { imports })
}

/// Phase 2：解析函数体与 initializer 中的值引用（以及块级作用域）。
pub fn check_file_bodies(
    source: &SourceFile,
    file: &mut ast::File,
    index: &Index,
    headers: &FileHeaders,
) -> Result<(), ResolveError> {
    // T0304/T0305：在函数体/表达式块中建立块级作用域（val/var）并做最小值名字解析；
    // T0308：扩展到顶层 `val/var` 的 initializer（见 scopes.rs 的实现）。
    check_block_scopes(source, file, index, &headers.imports)?;
    Ok(())
}

fn resolve_namespaced_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    anns: &[ast::AnnotationUse],
) -> Result<(), ResolveError> {
    for ann in anns {
        resolve_namespaced_annotation_use(source, file, index, ann)?;
    }
    Ok(())
}

fn resolve_namespaced_annotation_use(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ann: &ast::AnnotationUse,
) -> Result<(), ResolveError> {
    // T1015：仅对 `@A.B` 这类“命名空间注解”做最小存在性解析：
    // - `@A`（单段）仍由后续更完整的注解/typecheck 规则统一处理，避免破坏当前 fixtures 的默认行为。
    if ann.path.len() <= 1 {
        return Ok(());
    }

    let Some(first) = ann.path.first() else {
        return Ok(());
    };
    let Some(last) = ann.path.last() else {
        return Ok(());
    };

    let path = ast::TypePath {
        span: Span::new(first.span.start, last.span.end),
        segments: ann.path.clone(),
        args: Vec::new(),
    };

    // 注解名解析不引入声明级 type param 作用域（它解析的是注解类的名字路径）。
    let type_params = TypeParamScopes::new();
    resolve_type_path(source, file, index, &type_params, None, &path)
}

fn resolve_fun_header(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    eff_param: Option<&str>,
    fun: &ast::FunDecl,
) -> Result<(), ResolveError> {
    resolve_namespaced_annotations(source, file, index, &fun.annotations)?;
    for p in &fun.params {
        resolve_namespaced_annotations(source, file, index, &p.annotations)?;
    }

    if let Some(receiver) = &fun.receiver {
        resolve_type_ref(source, file, index, type_params, eff_param, receiver)?;
    }
    for p in &fun.params {
        if let Some(ty) = &p.ty {
            resolve_type_ref(source, file, index, type_params, eff_param, ty)?;
        }
    }
    if let Some(ret) = &fun.return_ty {
        resolve_type_ref(source, file, index, type_params, eff_param, ret)?;
    }
    if let Some(w) = &fun.where_clause {
        resolve_where_clause(source, file, index, type_params, eff_param, w)?;
    }
    Ok(())
}

fn resolve_type_decl_headers(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty: &ast::TypeDecl,
    type_params: &mut TypeParamScopes,
) -> Result<(), ResolveError> {
    // T0309：在该类型声明的 header/body 范围内，type params 作为 type namespace 的“局部符号”可见。
    type_params.push_decl(source, &ty.type_params)?;

    let result = (|| {
        resolve_namespaced_annotations(source, file, index, &ty.annotations)?;
        let ty_eff_param = ty.eff_param.as_ref().map(|p| source.slice(p.name.span));

        if let Some(w) = &ty.where_clause {
            resolve_where_clause(source, file, index, type_params, ty_eff_param, w)?;
        }

        // 主构造头参数（只解析类型；默认值的值解析需要更完整的 class 作用域规则，留给 T0313）。
        if let Some(primary_ctor) = &ty.primary_ctor {
            for p in &primary_ctor.params {
                resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                if let Some(ty) = &p.ty {
                    resolve_type_ref(source, file, index, type_params, ty_eff_param, ty)?;
                }
            }
        }

        // 继承/实现列表：解析 supertype 的类型引用。
        for st in &ty.supertypes {
            resolve_type_ref(source, file, index, type_params, ty_eff_param, &st.ty)?;
        }

        // 类型体成员签名：property/fun/nested type。
        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    resolve_namespaced_annotations(source, file, index, &v.annotations)?;
                    // enum variant payload 字段类型也属于 “签名里的类型引用” 范畴；
                    // 这里复用 TypeRef 的存在性解析规则（包含 type params）。
                    for p in &v.params {
                        resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                        if let Some(ty) = &p.ty {
                            resolve_type_ref(source, file, index, type_params, ty_eff_param, ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, type_params, ty_eff_param, ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {
                    // init block 的类型/值解析属于初始化执行体语境（T0313），当前阶段先跳过。
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    resolve_namespaced_annotations(source, file, index, &ctor.annotations)?;
                    // 次构造器参数类型也属于“签名里的类型引用”范畴；
                    // 默认值与 body 的值解析规则依赖完整构造/初始化语义（T0313），当前先不处理。
                    for p in &ctor.params {
                        resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                        if let Some(ty) = &p.ty {
                            resolve_type_ref(source, file, index, type_params, ty_eff_param, ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    type_params.push_decl(source, &f.type_params)?;
                    let fun_eff_param = f
                        .eff_param
                        .as_ref()
                        .map(|p| source.slice(p.name.span))
                        .or(ty_eff_param);
                    let result = (|| {
                        resolve_fun_header(source, file, index, type_params, fun_eff_param, f)
                    })();
                    type_params.pop_decl();
                    result?;
                }
                ast::TypeMember::Type(nested) => {
                    // Kotlin 风格：嵌套类型默认**不捕获**外层类型参数。
                    // 若未来引入 `inner` 等语义，可在此处再决定是否继承外层作用域。
                    let mut nested_type_params = TypeParamScopes::new();
                    resolve_type_decl_headers(
                        source,
                        file,
                        index,
                        nested,
                        &mut nested_type_params,
                    )?;
                }
                ast::TypeMember::Object(obj) => {
                    resolve_object_decl_headers(source, file, index, obj)?;
                }
            }
        }

        Ok(())
    })();

    type_params.pop_decl();
    result
}

/// 解析 `where` 子句：
///
/// - 约束左侧必须是当前可见的类型参数名（type param scope）；
/// - 约束右侧的 `TypeRef` 复用现有的类型引用解析规则（包前缀 + import + 可见性）。
fn resolve_where_clause(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    eff_param: Option<&str>,
    where_clause: &ast::WhereClause,
) -> Result<(), ResolveError> {
    for c in &where_clause.constraints {
        let name = source.slice(c.ty_param.span);
        if !type_params.contains(name) {
            return Err(ResolveError::UnresolvedTypeParam {
                name: name.to_string(),
                span: c.ty_param.span.into(),
            });
        }
        resolve_type_ref(source, file, index, type_params, eff_param, &c.bound)?;
    }
    Ok(())
}

fn resolve_object_decl_headers(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    obj: &ast::ObjectDecl,
) -> Result<(), ResolveError> {
    // object 自身不引入类型参数作用域（当前语法不支持 object 的 `<T>`）；成员可各自声明 type params。
    let mut type_params = TypeParamScopes::new();

    resolve_namespaced_annotations(source, file, index, &obj.annotations)?;

    // 超类型列表：解析类型引用（若存在）。
    for st in &obj.supertypes {
        resolve_type_ref(source, file, index, &type_params, None, &st.ty)?;
    }

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                resolve_namespaced_annotations(source, file, index, &v.annotations)?;
                for p in &v.params {
                    resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, &type_params, None, ty)?;
                    }
                }
            }
            ast::TypeMember::Property(p) => {
                resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                if let Some(ty) = &p.ty {
                    resolve_type_ref(source, file, index, &type_params, None, ty)?;
                }
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                resolve_namespaced_annotations(source, file, index, &ctor.annotations)?;
                for p in &ctor.params {
                    resolve_namespaced_annotations(source, file, index, &p.annotations)?;
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, &type_params, None, ty)?;
                    }
                }
            }
            ast::TypeMember::Fun(f) => {
                type_params.push_decl(source, &f.type_params)?;
                let eff_param = f.eff_param.as_ref().map(|p| source.slice(p.name.span));
                let result =
                    (|| resolve_fun_header(source, file, index, &type_params, eff_param, f))();
                type_params.pop_decl();
                result?;
            }
            ast::TypeMember::Type(nested) => {
                let mut nested_type_params = TypeParamScopes::new();
                resolve_type_decl_headers(source, file, index, nested, &mut nested_type_params)?;
            }
            ast::TypeMember::Object(nested) => {
                resolve_object_decl_headers(source, file, index, nested)?;
            }
        }
    }

    Ok(())
}

fn resolve_type_ref(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    eff_param: Option<&str>,
    ty: &ast::TypeRef,
) -> Result<(), ResolveError> {
    match ty {
        ast::TypeRef::Path(p) => resolve_type_path(source, file, index, type_params, eff_param, p),
        ast::TypeRef::Tuple(t) => {
            for e in &t.elements {
                resolve_type_ref(source, file, index, type_params, eff_param, e)?;
            }
            Ok(())
        }
        // 星投影不引入可解析的符号引用：`List<*>` 中的 `*` 由 typecheck 决定具体含义。
        ast::TypeRef::Star { .. } => Ok(()),
        ast::TypeRef::EffectRowArg { row, .. } => {
            // use-site effect row 语法本身不引入 type position 的引用，但 row expr 的项
            // 与函数类型上的 `/ RowExpr` 一样需要做存在性解析（effect 名 / row 变量等）。
            for term in &row.terms {
                if eff_param.is_some_and(|name| {
                    term.segments.len() == 1
                        && term.args.is_empty()
                        && source.slice(term.segments[0].span) == name
                }) {
                    continue;
                }
                resolve_type_path(source, file, index, type_params, eff_param, term)?;
            }
            Ok(())
        }
        ast::TypeRef::Function(f) => {
            if let Some(receiver) = &f.receiver {
                resolve_type_ref(source, file, index, type_params, eff_param, receiver)?;
            }
            for p in &f.params {
                resolve_type_ref(source, file, index, type_params, eff_param, p)?;
            }
            resolve_type_ref(source, file, index, type_params, eff_param, &f.return_ty)?;

            if let Some(effects) = &f.effects {
                for term in &effects.terms {
                    if eff_param.is_some_and(|name| {
                        term.segments.len() == 1
                            && term.args.is_empty()
                            && source.slice(term.segments[0].span) == name
                    }) {
                        continue;
                    }
                    resolve_type_path(source, file, index, type_params, eff_param, term)?;
                }
            }

            Ok(())
        }
        ast::TypeRef::Nullable { inner, .. } => {
            resolve_type_ref(source, file, index, type_params, eff_param, inner)
        }
    }
}

fn resolve_type_path(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    eff_param: Option<&str>,
    path: &ast::TypePath,
) -> Result<(), ResolveError> {
    // 先解析类型实参（如 `Option<T>`），确保其中的 TypeRef 也会被递归解析。
    for arg in &path.args {
        resolve_type_ref(source, file, index, type_params, eff_param, arg)?;
    }

    let segments = path
        .segments
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>();
    let local = segments.join(".");

    // T0309：单段路径优先解析为当前声明的 type param（type param 会 shadow 顶层同名 type）。
    if segments.len() == 1 && type_params.contains(segments[0]) {
        return Ok(());
    }

    let pkg = package_prefix(source, file.package.as_ref());
    let mut candidates = Vec::new();

    // 1) 同包优先：pkg + local
    if !pkg.is_empty() {
        candidates.push(format!("{pkg}.{local}"));
    }

    // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
    candidates.push(local.clone());

    // 3) 对单段名字，应用 import 规则（显式 import / star import）
    if segments.len() == 1 {
        let name = segments[0];
        for import in &file.imports {
            let import_path = import
                .path
                .iter()
                .map(|id| source.slice(id.span))
                .collect::<Vec<_>>()
                .join(".");

            if import.has_star {
                candidates.push(format!("{import_path}.{name}"));
            } else {
                let local = import
                    .alias
                    .as_ref()
                    .map(|id| source.slice(id.span))
                    .or_else(|| import.path.last().map(|id| source.slice(id.span)))
                    .unwrap_or("");
                if local == name {
                    candidates.push(import_path);
                }
            }
        }
    }

    // 去重并尝试匹配 type namespace
    candidates.sort();
    candidates.dedup();

    let mut not_visible: Option<(String, Visibility, Span)> = None;
    let use_cone = index.cone_of_source(source);
    for fqn in candidates {
        let Some(syms) = index.by_fqn.get(&fqn) else {
            continue;
        };

        let Some(sym) = syms.get(SymbolKind::Type) else {
            continue;
        };

        if is_symbol_visible_from(use_cone, source, sym) {
            // TODO: 在后续阶段把解析结果写回 AST/HIR
            return Ok(());
        }

        // 若只有不可见的候选，报“不可见”而不是“未解析”。
        // 但依旧继续尝试其它候选（例如同名但来自其它 import 的 public type）。
        if not_visible.is_none() {
            not_visible = Some((fqn.clone(), sym.visibility, sym.span));
        }
    }

    if let Some((name, visibility, def_span)) = not_visible {
        return Err(ResolveError::NotVisible {
            name,
            visibility,
            use_span: path.span.into(),
            def_span: def_span.into(),
        });
    }

    // 内建标量类型：允许在 sysroot 尚未显式声明时也能被解析。
    // 说明：这些类型的布局/语义由编译器固定；sysroot 仅提供“可见声明”用于 IDE/文档一致性。
    if is_implicit_builtin_type_name(&local) {
        return Ok(());
    }

    Err(ResolveError::UnresolvedType {
        name: local,
        span: path.span.into(),
    })
}

fn is_implicit_builtin_type_name(local_or_fqn: &str) -> bool {
    matches!(
        local_or_fqn,
        "Unit"
            | "Nothing"
            | "Bool"
            | "String"
            | "Int"
            | "UInt"
            | "scoop.core.Unit"
            | "scoop.core.Nothing"
            | "scoop.core.Bool"
            | "scoop.core.String"
            | "scoop.core.Int"
            | "scoop.core.UInt"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::session::Session;

    #[test]
    fn duplicate_top_level_type_is_error() {
        let s1 = SourceFile::new_virtual("<mem1>", "package a\nstruct S {}");
        let s2 = SourceFile::new_virtual("<mem2>", "package a\nstruct S {}");
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let err = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("重复定义"));
    }

    #[test]
    fn overloaded_top_level_funs_are_collected() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nfun f(x: Any): Any {}\nfun f(x: String): Any {}",
        );
        let ast = parse_file(&src).unwrap();

        let index = Index::build(&[(&src, &ast)]).unwrap();
        let syms = index.by_fqn.get("a.f").unwrap();
        assert_eq!(syms.fun.len(), 2);
    }

    #[test]
    fn constructors_are_collected_in_overload_set() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nimport scoop.core.*\nclass C(x: Any) { constructor(y: Any) {} }",
        );
        let ast = parse_file(&src).unwrap();

        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();
        let ctors = index.constructors.get("a.C").unwrap();
        assert_eq!(ctors.len(), 2);
        assert!(ctors.iter().any(|c| c.kind == ConstructorKind::Primary));
        assert!(ctors.iter().any(|c| c.kind == ConstructorKind::Secondary));
    }

    #[test]
    fn resolve_types_with_import_star() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nimport scoop.core.*\nfun f(x: Option<Any>): Any {}",
        );
        let mut ast = parse_file(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();
        check_file_bindings(&src, &mut ast, &index).unwrap();
    }

    #[test]
    fn invalid_visibility_modifiers_is_error() {
        let src = SourceFile::new_virtual("<mem>", "package a\npublic private fun f() {}");
        let ast = parse_file(&src).unwrap();

        let err = Index::build(&[(&src, &ast)]).unwrap_err();
        assert!(matches!(err, ResolveError::InvalidVisibility { .. }));
    }

    #[test]
    fn unresolved_type_is_error() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun f(x: Missing) {}");
        let mut ast = parse_file(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();
        let err = check_file_bindings(&src, &mut ast, &index).unwrap_err();
        assert!(matches!(err, ResolveError::UnresolvedType { .. }));
    }
}
