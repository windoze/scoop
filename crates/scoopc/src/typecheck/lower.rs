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
use crate::ty::{BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

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
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut ctx = TypeLowering {
        source,
        index,
        imports,
        env,
        types,
        builtins,
        pkg_prefix,
    };

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
        }
    }

    Ok(())
}

struct TypeLowering<'a> {
    source: &'a SourceFile,
    index: &'a Index,
    imports: &'a ImportTable,
    env: &'a TypeEnv,
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    pkg_prefix: String,
}

impl<'a> TypeLowering<'a> {
    fn lower_type_ref(&mut self, ty: &ast::TypeRef) -> Result<TypeId, TypeLowerError> {
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
            ast::TypeRef::Function(f) => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "function type",
                span: f.span.into(),
            }),
        }
    }

    fn lower_type_path(&mut self, path: &ast::TypePath) -> Result<TypeId, TypeLowerError> {
        let fqn = self.resolve_type_path_fqn(path)?;

        let expected = self
            .env
            .type_param_count(&fqn)
            .ok_or_else(|| TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: path.span.into(),
            })?;
        let found = path.args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: path.span.into(),
            });
        }

        // 先对少数 builtin/special-case 做 lowering。
        match fqn.as_str() {
            // `Any`：引用类型的顶层 supertype。
            "scoop.core.Any" => return Ok(self.builtins.any),
            // `Option<T>`：值类型；同时也是 `T?` 的 desugar 目标。
            "scoop.core.Option" => {
                let inner = self.lower_type_ref(&path.args[0])?;
                return Ok(self.types.ty_option(inner));
            }
            _ => {}
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: path.span.into(),
            });
        };

        let args = path
            .args
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
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self.types.intern(TypeKind::Value(
                        ValueTypeKind::Nominal(nominal),
                    )),
                    ast::TypeKind::Class
                    | ast::TypeKind::Interface
                    | ast::TypeKind::Effect => self.types.intern(TypeKind::Ref(RefTypeKind::Nominal(
                        nominal,
                    ))),
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
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
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
            }
        }

        Ok(())
    }

    fn resolve_type_path_fqn(&self, path: &ast::TypePath) -> Result<String, TypeLowerError> {
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
