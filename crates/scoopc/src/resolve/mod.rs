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

use std::collections::HashMap;
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
        self.frames.iter().rev().any(|frame| frame.contains_key(name))
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

/// 可见性（visibility）。
///
/// 当前阶段（T0306）：
/// - `public` 默认可见；
/// - `private` 对顶层声明按“文件内可见”处理：跨文件引用将报错；
/// - `internal` 的 cone/module 语义留给后续任务，这里先等同于可见。
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

#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
    pub decl_file: PathBuf,
    pub visibility: Visibility,
}

/// 同一个 FQN 下按命名空间（type/value/fun）分组的符号集合。
///
/// 说明：
/// - 语言层面允许 **同名 type 与 fun/value 并存**（它们属于不同命名空间）。
/// - 同一命名空间内仍保持“当前阶段不支持重载”的约束：重复定义直接报错。
#[derive(Debug, Default, Clone)]
pub struct NamespacedSymbols {
    pub ty: Option<Symbol>,
    pub fun: Option<Symbol>,
    pub value: Option<Symbol>,
}

impl NamespacedSymbols {
    fn slot_mut(&mut self, kind: SymbolKind) -> &mut Option<Symbol> {
        match kind {
            SymbolKind::Type => &mut self.ty,
            SymbolKind::Fun => &mut self.fun,
            SymbolKind::Value => &mut self.value,
        }
    }

    fn get(&self, kind: SymbolKind) -> Option<&Symbol> {
        match kind {
            SymbolKind::Type => self.ty.as_ref(),
            SymbolKind::Fun => self.fun.as_ref(),
            SymbolKind::Value => self.value.as_ref(),
        }
    }
}

/// 一个编译单元（多个文件）的顶层符号索引。
#[derive(Debug, Default)]
pub struct Index {
    /// FQN（例如 `scoop.core.Option`）→ 按命名空间分组的符号集合。
    pub by_fqn: HashMap<String, NamespacedSymbols>,
}

impl Index {
    pub fn build(files: &[(&SourceFile, &ast::File)]) -> Result<Self, ResolveError> {
        let mut index = Index::default();
        for (source, file) in files {
            index.add_file(source, file)?;
        }
        Ok(index)
    }

    fn add_file(&mut self, source: &SourceFile, file: &ast::File) -> Result<(), ResolveError> {
        let pkg = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    // typealias 是类型命名空间的顶层符号（T0251）。
                    let visibility = visibility_from_modifiers(&ta.modifiers, ta.span)?;
                    self.insert_symbol(source, &pkg, SymbolKind::Type, ta.name.span, visibility)?;
                }
                ast::Item::Fun(fun) => {
                    let visibility = visibility_from_modifiers(&fun.modifiers, fun.span)?;
                    self.insert_symbol(source, &pkg, SymbolKind::Fun, fun.name.span, visibility)?;
                }
                ast::Item::Type(ty) => {
                    self.add_type_decl(source, &pkg, ty)?;
                }
                ast::Item::Val(v) => {
                    // 顶层 `val/var` 必须有名字；解构绑定仅在 block 内作为语句出现（T0244）。
                    if let Some(name) = v.name() {
                        let visibility = visibility_from_modifiers(&v.modifiers, v.span)?;
                        self.insert_symbol(source, &pkg, SymbolKind::Value, name.span, visibility)?;
                    }
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
        source: &SourceFile,
        prefix: &str,
        ty: &ast::TypeDecl,
    ) -> Result<(), ResolveError> {
        // 1) 先插入类型自身（type namespace）。
        let visibility = visibility_from_modifiers(&ty.modifiers, ty.span)?;
        self.insert_symbol(source, prefix, SymbolKind::Type, ty.name.span, visibility)?;

        // 2) 递归处理类型体成员：fields/methods/nested types。
        let type_name = source.slice(ty.name.span);
        let type_prefix = if prefix.is_empty() {
            type_name.to_string()
        } else {
            format!("{prefix}.{type_name}")
        };

        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    let visibility = visibility_from_modifiers(&p.modifiers, p.span)?;
                    self.insert_symbol(
                        source,
                        &type_prefix,
                        SymbolKind::Value,
                        p.name.span,
                        visibility,
                    )?;
                }
                ast::TypeMember::Fun(f) => {
                    let visibility = visibility_from_modifiers(&f.modifiers, f.span)?;
                    self.insert_symbol(
                        source,
                        &type_prefix,
                        SymbolKind::Fun,
                        f.name.span,
                        visibility,
                    )?;
                }
                ast::TypeMember::Type(nested) => {
                    self.add_type_decl(source, &type_prefix, nested)?;
                }
            }
        }

        Ok(())
    }

    fn insert_symbol(
        &mut self,
        source: &SourceFile,
        pkg_prefix: &str,
        kind: SymbolKind,
        name_span: Span,
        visibility: Visibility,
    ) -> Result<(), ResolveError> {
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
            visibility,
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

fn is_symbol_visible_from(source: &SourceFile, symbol: &Symbol) -> bool {
    match symbol.visibility {
        Visibility::Public | Visibility::Internal => true,
        Visibility::Private => symbol.decl_file == source.path(),
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

    // T0309：声明级 type params 作用域（用于签名中的 TypeRef 解析）。
    let mut type_params = TypeParamScopes::new();

    // 解析签名里的类型引用（type/function/field signatures）。
    // 说明：当前阶段仍以“存在性解析”为主；更深层的泛型/alias 语义交给 typecheck。
    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => resolve_type_ref(source, file, index, &type_params, &ta.ty)?,
            ast::Item::Fun(fun) => {
                type_params.push_decl(source, &fun.type_params)?;
                let result = (|| resolve_fun_header(source, file, index, &type_params, fun))();
                type_params.pop_decl();
                result?;
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    resolve_type_ref(source, file, index, &type_params, ty)?;
                }
            }
            ast::Item::Type(ty) => resolve_type_decl_headers(source, file, index, ty, &mut type_params)?,
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

fn resolve_fun_header(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    fun: &ast::FunDecl,
) -> Result<(), ResolveError> {
    if let Some(receiver) = &fun.receiver {
        resolve_type_ref(source, file, index, type_params, receiver)?;
    }
    for p in &fun.params {
        if let Some(ty) = &p.ty {
            resolve_type_ref(source, file, index, type_params, ty)?;
        }
    }
    if let Some(ret) = &fun.return_ty {
        resolve_type_ref(source, file, index, type_params, ret)?;
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
        // 主构造头参数（只解析类型；默认值的值解析需要更完整的 class 作用域规则，留给 T0313）。
        if let Some(primary_ctor) = &ty.primary_ctor {
            for p in &primary_ctor.params {
                if let Some(ty) = &p.ty {
                    resolve_type_ref(source, file, index, type_params, ty)?;
                }
            }
        }

        // 继承/实现列表：解析 supertype 的类型引用。
        for st in &ty.supertypes {
            resolve_type_ref(source, file, index, type_params, &st.ty)?;
        }

        // 类型体成员签名：property/fun/nested type。
        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, type_params, ty)?;
                    }
                }
                ast::TypeMember::Fun(f) => {
                    type_params.push_decl(source, &f.type_params)?;
                    let result = (|| resolve_fun_header(source, file, index, type_params, f))();
                    type_params.pop_decl();
                    result?;
                }
                ast::TypeMember::Type(nested) => {
                    // Kotlin 风格：嵌套类型默认**不捕获**外层类型参数。
                    // 若未来引入 `inner` 等语义，可在此处再决定是否继承外层作用域。
                    let mut nested_type_params = TypeParamScopes::new();
                    resolve_type_decl_headers(source, file, index, nested, &mut nested_type_params)?;
                }
            }
        }

        Ok(())
    })();

    type_params.pop_decl();
    result
}

fn resolve_type_ref(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    ty: &ast::TypeRef,
) -> Result<(), ResolveError> {
    match ty {
        ast::TypeRef::Path(p) => resolve_type_path(source, file, index, type_params, p),
        ast::TypeRef::Tuple(t) => {
            for e in &t.elements {
                resolve_type_ref(source, file, index, type_params, e)?;
            }
            Ok(())
        }
        // 星投影不引入可解析的符号引用：`List<*>` 中的 `*` 由 typecheck 决定具体含义。
        ast::TypeRef::Star { .. } => Ok(()),
        ast::TypeRef::Function(f) => {
            if let Some(receiver) = &f.receiver {
                resolve_type_ref(source, file, index, type_params, receiver)?;
            }
            for p in &f.params {
                resolve_type_ref(source, file, index, type_params, p)?;
            }
            resolve_type_ref(source, file, index, type_params, &f.return_ty)?;

            if let Some(effects) = &f.effects {
                for term in &effects.terms {
                    resolve_type_path(source, file, index, type_params, term)?;
                }
            }

            Ok(())
        }
        ast::TypeRef::Nullable { inner, .. } => resolve_type_ref(source, file, index, type_params, inner),
    }
}

fn resolve_type_path(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_params: &TypeParamScopes,
    path: &ast::TypePath,
) -> Result<(), ResolveError> {
    // 先解析类型实参（如 `Option<T>`），确保其中的 TypeRef 也会被递归解析。
    for arg in &path.args {
        resolve_type_ref(source, file, index, type_params, arg)?;
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
                let last = import
                    .path
                    .last()
                    .map(|id| source.slice(id.span))
                    .unwrap_or("");
                if last == name {
                    candidates.push(import_path);
                }
            }
        }
    }

    // 去重并尝试匹配 type namespace
    candidates.sort();
    candidates.dedup();

    let mut not_visible: Option<(String, Visibility, Span)> = None;
    for fqn in candidates {
        let Some(syms) = index.by_fqn.get(&fqn) else {
            continue;
        };

        let Some(sym) = syms.get(SymbolKind::Type) else {
            continue;
        };

        if is_symbol_visible_from(source, sym) {
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

    Err(ResolveError::UnresolvedType {
        name: local,
        span: path.span.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::session::Session;

    #[test]
    fn duplicate_top_level_is_error() {
        let s1 = SourceFile::new_virtual("<mem1>", "package a\nfun f() {}");
        let s2 = SourceFile::new_virtual("<mem2>", "package a\nfun f() {}");
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let err = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("重复定义"));
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
