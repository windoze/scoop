//! `TypeRef` → `Type` lowering（T0403）。
//!
//! 当前阶段的目标：
//! - 把 parser 产出的 AST `TypeRef` 转换为编译器内部类型表示（`ty::TypeId`）
//! - 在 lowering 过程中做最小语义校验：类型存在性（应由 resolve 保证）与泛型 arity 检查
//! - 先覆盖 `Path` / `Tuple` / `Nullable`，其它类型语法在后续任务逐步补齐

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};

use super::{TypeEnv, TypeSymbolKind};

#[derive(Debug, Error, Diagnostic)]
pub enum TypeLowerError {
    #[error("未解析的类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unresolved_type))]
    UnresolvedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型参数数量不匹配：{name} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::type_arity_mismatch))]
    TypeArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的类型语法：{kind}")]
    #[diagnostic(code(scoop::typecheck::unsupported_type_ref))]
    UnsupportedTypeRef {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型环境缺少符号：{fqn}")]
    #[diagnostic(code(scoop::typecheck::missing_type_symbol_in_env))]
    MissingTypeSymbolInEnv {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 对一个文件内出现的所有 “type position” 的 `TypeRef` 执行 lowering 并做最小校验。
///
/// 说明：
/// - 该函数是早期 typecheck phase 的一块可独立回归的最小能力（fixtures 会直接调用）；
/// - 当前只走声明头（fun/val/type/typealias）的类型引用，不进入函数体的表达式类型检查。
pub fn check_file_type_refs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), TypeLowerError> {
    let mut ctx = TypeLowering::new(source, file, index, imports, env, types, builtins);

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                let _ = ctx.lower_type_ref(&ta.ty)?;
            }
            ast::Item::Fun(fun) => {
                if let Some(receiver) = &fun.receiver {
                    let _ = ctx.lower_type_ref(receiver)?;
                }
                for p in &fun.params {
                    if let Some(ty) = &p.ty {
                        let _ = ctx.lower_type_ref(ty)?;
                    }
                }
                if let Some(ret) = &fun.return_ty {
                    let _ = ctx.lower_type_ref(ret)?;
                }
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
            }
            ast::Item::Type(ty) => {
                ctx.check_type_decl_headers(ty)?;
            }
            ast::Item::Object(obj) => {
                ctx.check_object_decl_headers(obj)?;
            }
        }
    }

    Ok(())
}

pub(super) struct TypeLowering<'a> {
    source: &'a SourceFile,
    index: &'a Index,
    imports: &'a ImportTable,
    env: &'a TypeEnv,
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    pkg_prefix: String,
}

impl<'a> TypeLowering<'a> {
    pub(super) fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        Self {
            source,
            index,
            imports,
            env,
            types,
            builtins,
            pkg_prefix,
        }
    }

    pub(super) fn pkg_prefix(&self) -> &str {
        &self.pkg_prefix
    }

    pub(super) fn env(&self) -> &TypeEnv {
        self.env
    }

    pub(super) fn lower_type_ref(&mut self, ty: &ast::TypeRef) -> Result<TypeId, TypeLowerError> {
        match ty {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(t) => {
                if t.elements.is_empty() {
                    return Ok(self.builtins.unit);
                }
                let mut elements = Vec::with_capacity(t.elements.len());
                for e in &t.elements {
                    elements.push(self.lower_type_ref(e)?);
                }
                Ok(self.types.ty_tuple(elements))
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner)?;
                Ok(self.types.ty_option(inner))
            }
            ast::TypeRef::Star { span } => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "star projection (*)",
                span: (*span).into(),
            }),
            ast::TypeRef::EffectRowArg { span, .. } => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "use-site effect row arg (`eff ...`)",
                span: (*span).into(),
            }),
            ast::TypeRef::Function(f) => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "function type",
                span: f.span.into(),
            }),
        }
    }

    pub(super) fn fmt_type(&self, id: TypeId) -> String {
        self.types.display(id).to_string()
    }

    /// 返回给定 `TypeId` 在 `TypeStore` 中的具体 kind（clone）。
    ///
    /// 说明：typecheck 的某些表达式语义（例如 `with` 更新）需要区分：
    /// - 是否为值类型/引用类型
    /// - 是否为名义值类型（struct/enum）
    pub(super) fn type_kind(&self, id: TypeId) -> TypeKind {
        self.types.kind(id).clone()
    }

    /// 若给定 FQN 对应 nominal type，返回其声明的 `TypeKind`（struct/enum/class/interface/effect）。
    ///
    /// 用途：对“语义上只对某类 nominal type 生效”的规则做最小判定，例如：
    /// - `with` 更新当前阶段仅支持 `struct`
    pub(super) fn nominal_decl_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        let sym = self.env.type_symbol(fqn)?;
        match sym.kind {
            TypeSymbolKind::Nominal(kind) => Some(kind),
            TypeSymbolKind::TypeAlias => None,
        }
    }

    pub(super) fn is_ref(&self, id: TypeId) -> bool {
        self.types.is_ref(id)
    }

    pub(super) fn ty_option(&mut self, inner: TypeId) -> TypeId {
        self.types.ty_option(inner)
    }

    pub(super) fn ty_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        self.types.ty_tuple(elements)
    }

    /// 将“已解析出的类型 FQN + 已 lowering 的 type args”构造成 `TypeId`。
    ///
    /// 说明：
    /// - `lower_type_ref`/`lower_type_path` 以 AST 为入口，会递归 lowering type args；
    /// - enum variant 构造等场景（T0426）需要先对 type args 做 substitution/推断，
    ///   再把结果组装回一个 `TypeId`，因此提供该辅助方法。
    pub(super) fn lower_type_fqn_with_args(
        &mut self,
        fqn: String,
        args: Vec<TypeId>,
        span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            "scoop.core.Any" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.any);
            }
            "scoop.core.String" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.string);
            }
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.unit);
            }
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.nothing);
            }
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.bool_);
            }
            "scoop.core.Int" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.uint);
            }
            "scoop.core.Option" => {
                check_arity(&fqn, 1, args.len(), span)?;
                return Ok(self.types.ty_option(args[0]));
            }
            _ => {}
        }

        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: span.into(),
            }
        })?;
        let found = args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: span.into(),
            });
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: span.into(),
            });
        };

        match sym.kind {
            TypeSymbolKind::TypeAlias => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "typealias (type-level alias)",
                span: span.into(),
            }),
            TypeSymbolKind::Nominal(kind) => {
                let nominal = NominalType { fqn, args };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                Ok(id)
            }
        }
    }

    fn lower_type_path(&mut self, path: &ast::TypePath) -> Result<TypeId, TypeLowerError> {
        let fqn = match self.resolve_type_path_fqn(path) {
            Ok(fqn) => fqn,
            Err(TypeLowerError::UnresolvedType { name, span }) => {
                let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                    return Err(TypeLowerError::UnresolvedType { name, span });
                };
                builtin_fqn.to_string()
            }
            Err(other) => return Err(other),
        };

        // 说明（T0253）：
        // - use-site effect row 实参（`eff ...`）在 AST 中被建模为 `TypeRef::EffectRowArg`；
        // - 它不属于“类型参数”（arity）的一部分，因此在 typecheck 的 type args lowering 中暂时忽略。
        let type_args = path
            .args
            .iter()
            .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
            .collect::<Vec<_>>();

        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            // `Any`：引用类型的顶层 supertype。
            "scoop.core.Any" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.any);
            }
            // `String`：内建字符串类型。
            "scoop.core.String" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.string);
            }
            // `Unit`：0 元 tuple。
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.unit);
            }
            // `Nothing`：bottom type。
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.nothing);
            }
            // `Bool`：内建布尔类型。
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.bool_);
            }
            // `Int/UInt`：word-sized 整数。
            "scoop.core.Int" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                return Ok(self.builtins.uint);
            }
            // `Option<T>`：值类型；同时也是 `T?` 的 desugar 目标。
            "scoop.core.Option" => {
                check_arity(&fqn, 1, type_args.len(), path.span)?;
                let inner = self.lower_type_ref(type_args[0])?;
                return Ok(self.types.ty_option(inner));
            }
            _ => {}
        }

        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: path.span.into(),
            }
        })?;
        let found = type_args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: path.span.into(),
            });
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: path.span.into(),
            });
        };

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        match sym.kind {
            TypeSymbolKind::TypeAlias => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "typealias (type-level alias)",
                span: path.span.into(),
            }),
            TypeSymbolKind::Nominal(kind) => {
                let nominal = NominalType { fqn, args };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                Ok(id)
            }
        }
    }

    fn check_type_decl_headers(&mut self, ty: &ast::TypeDecl) -> Result<(), TypeLowerError> {
        // 主构造头参数类型
        if let Some(primary_ctor) = &ty.primary_ctor {
            for p in &primary_ctor.params {
                if let Some(ty) = &p.ty {
                    let _ = self.lower_type_ref(ty)?;
                }
            }
        }

        // 继承/实现列表类型
        for st in &ty.supertypes {
            let _ = self.lower_type_ref(&st.ty)?;
        }

        // 成员签名类型（property/fun/nested type）
        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {
                    // init block 属于初始化执行体；当前阶段 type lowering 仅处理声明头与成员签名。
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    // 次构造器参数类型同样属于成员签名类型（T0257）。
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(obj) => {
                    self.check_object_decl_headers(obj)?;
                }
            }
        }

        Ok(())
    }

    fn check_object_decl_headers(&mut self, obj: &ast::ObjectDecl) -> Result<(), TypeLowerError> {
        // 继承/实现列表类型
        for st in &obj.supertypes {
            let _ = self.lower_type_ref(&st.ty)?;
        }

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.check_object_decl_headers(nested)?;
                }
            }
        }

        Ok(())
    }

    pub(super) fn resolve_type_path_fqn(&self, path: &ast::TypePath) -> Result<String, TypeLowerError> {
        let segments = path
            .segments
            .iter()
            .map(|id| self.source.slice(id.span))
            .collect::<Vec<_>>();
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
        if segments.len() == 1 {
            let name = segments[0];

            if let Some(fqns) = self.imports.ty.explicit.get(name) {
                candidates.extend(fqns.iter().cloned());
            }

            for prefix in &self.imports.star {
                candidates.push(format!("{prefix}.{name}"));
            }
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: path.span.into(),
        })
    }

    /// 在 typecheck 阶段按“路径段名”解析 type path 对应的 FQN。
    ///
    /// 说明：
    /// - `resolve_type_path_fqn` 依赖 `TypePath` 里的 `Ident.span` 从 `self.source` 切片；
    /// - 但某些场景（例如 sysroot enum variant 的字段类型，T0426）持有的是“来自其它源文件”的
    ///   `TypeRef`/`TypePath`，其 span 不能再用于当前文件切片；
    /// - 因此提供该按字符串段名解析的辅助入口（仍复用当前使用点的 package/import/可见性规则）。
    pub(super) fn resolve_type_path_fqn_by_name(
        &self,
        segments: &[String],
        use_span: Span,
    ) -> Result<String, TypeLowerError> {
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
        if segments.len() == 1 {
            let name = segments[0].as_str();

            if let Some(fqns) = self.imports.ty.explicit.get(name) {
                candidates.extend(fqns.iter().cloned());
            }

            for prefix in &self.imports.star {
                candidates.push(format!("{prefix}.{name}"));
            }
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: use_span.into(),
        })
    }
}

fn is_symbol_visible_from(source: &SourceFile, symbol: &crate::resolve::Symbol) -> bool {
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

fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
    match local_or_fqn {
        // allow both `Int` and `scoop.core.Int` spellings
        "Any" | "scoop.core.Any" => Some("scoop.core.Any"),
        "String" | "scoop.core.String" => Some("scoop.core.String"),
        "Unit" | "scoop.core.Unit" => Some("scoop.core.Unit"),
        "Nothing" | "scoop.core.Nothing" => Some("scoop.core.Nothing"),
        "Bool" | "scoop.core.Bool" => Some("scoop.core.Bool"),
        "Int" | "scoop.core.Int" => Some("scoop.core.Int"),
        "UInt" | "scoop.core.UInt" => Some("scoop.core.UInt"),
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        _ => None,
    }
}

fn check_arity(fqn: &str, expected: usize, found: usize, span: Span) -> Result<(), TypeLowerError> {
    if expected == found {
        return Ok(());
    }
    Err(TypeLowerError::TypeArityMismatch {
        name: fqn.to_string(),
        expected,
        found,
        span: span.into(),
    })
}
