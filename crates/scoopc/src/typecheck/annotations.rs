//! 注解系统的最小语义检查（T1002）。
//!
//! 当前阶段目标：
//! - 识别 `annotation class X(...)` 并对其施加最小形态约束（data-only）；
//! - 在 `@Name(...)` 使用处验证 `Name` 必须引用一个注解类（spec §15.2~§15.3）。
//!
//! 非目标（留给后续任务）：
//! - target/retention/meta-annotation 规则；
//! - 注解参数类型白名单与默认值/必填规则；
//! - 注解在表达式位置的语义（如 `@Suppress(...) expr`）；
//! - `@Extern/@Intrinsic/@NoGC/@Unsafe` 等内建注解的特殊行为（T1003 已覆盖最小门禁；更完整规则见 TODO）。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::span::Span;

use super::builtin_annotations::{BuiltinAnnotationFlags, BuiltinAnnotationKind, builtin_annotation_kind};
use super::TypeEnv;

#[derive(Debug, Error, Diagnostic)]
pub enum AnnotationError {
    #[error("注解类必须是 `class`：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_must_be_class))]
    AnnotationClassMustBeClass {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持继承/实现接口：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_supertypes_not_supported))]
    AnnotationClassSupertypesNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类暂不支持类型体（data-only 仅允许主构造参数）：{type_fqn}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_body_not_supported))]
    AnnotationClassBodyNotSupported {
        type_fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解类参数必须是 `val`：{type_fqn}.{param}")]
    #[diagnostic(code(scoop::typecheck::annotation_class_param_must_be_val))]
    AnnotationClassParamMustBeVal {
        type_fqn: String,
        param: String,
        #[label("这里需要写 `val`")]
        span: miette::SourceSpan,
    },

    #[error("未解析的注解类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unresolved_annotation_type))]
    UnresolvedAnnotationType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("注解 `{name}` 不是注解类（需要 `annotation class`）")]
    #[diagnostic(code(scoop::typecheck::annotation_type_is_not_annotation_class))]
    AnnotationTypeIsNotAnnotationClass {
        name: String,
        #[label("注解使用在这里")]
        use_span: miette::SourceSpan,
        #[label("该类型声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error("内建注解 `{annotation}` 只能用于 {allowed}，但这里出现在 {found} 上")]
    #[diagnostic(code(scoop::typecheck::builtin_annotation_invalid_target))]
    BuiltinAnnotationInvalidTarget {
        annotation: String,
        allowed: &'static str,
        found: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("内建注解 `{annotation}` 暂不支持参数")]
    #[diagnostic(code(scoop::typecheck::builtin_annotation_args_not_supported))]
    BuiltinAnnotationArgsNotSupported {
        annotation: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 暂仅支持 0 或 1 个字符串字面量参数")]
    #[diagnostic(code(scoop::typecheck::extern_annotation_args_invalid))]
    ExternAnnotationArgsInvalid {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("`@Extern` 函数必须省略函数体（外部实现）：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::extern_fun_must_have_no_body))]
    ExternFunMustHaveNoBody {
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic` 函数必须省略函数体（实现由编译器/运行时提供）：{fun_name}")]
    #[diagnostic(code(scoop::typecheck::intrinsic_fun_must_have_no_body))]
    IntrinsicFunMustHaveNoBody {
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },

    #[error("`@Intrinsic` 类型中的成员函数必须省略函数体：{type_fqn}.{fun_name}")]
    #[diagnostic(code(scoop::typecheck::intrinsic_member_fun_must_have_no_body))]
    IntrinsicMemberFunMustHaveNoBody {
        type_fqn: String,
        fun_name: String,
        #[label("这里不应有函数体")]
        span: miette::SourceSpan,
    },
}

/// 检查一个文件内的注解相关最小规则。
///
/// 说明：
/// - 该检查需要 `Index`/`TypeEnv` 来做“注解名 → 类型符号”的解析与分类；
/// - 它应当在 resolver bodies 之后、其它 typecheck pass 之前执行（尽早报错）。
pub fn check_file_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
) -> Result<(), AnnotationError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                check_annotation_uses(source, file, index, env, &ta.annotations)?;
                reject_builtin_annotations_on_target(source, &ta.annotations, "typealias")?;
            }
            ast::Item::Fun(fun) => {
                check_annotation_uses(source, file, index, env, &fun.annotations)?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
            }
            ast::Item::ExtensionProperty(p) => {
                check_annotation_uses(source, file, index, env, &p.annotations)?;
                reject_builtin_annotations_on_target(source, &p.annotations, "extension property")?;
            }
            ast::Item::Val(v) => {
                check_annotation_uses(source, file, index, env, &v.annotations)?;
                reject_builtin_annotations_on_target(source, &v.annotations, "val/var")?;
            }
            ast::Item::Type(ty) => {
                check_type_decl_annotations(source, file, index, env, ty, &pkg_prefix)?;
            }
            ast::Item::Object(obj) => {
                check_object_decl_annotations(source, file, index, env, obj, &pkg_prefix)?;
            }
        }
    }

    Ok(())
}

/// 检查类型声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_type_decl_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    decl: &ast::TypeDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    let local = source.slice(decl.name.span);
    let type_fqn = join_prefix(prefix, local);

    // 1) 注解使用：`@Foo` / `@Foo(...)`。
    check_annotation_uses(source, file, index, env, &decl.annotations)?;
    check_builtin_annotations_on_type_decl(source, decl, &type_fqn)?;

    // 2) 注解类自身的最小形态约束（data-only）。
    if decl.modifiers.contains(&ast::Modifier::Annotation) {
        check_annotation_class_decl_rules(source, decl, &type_fqn)?;
    }

    // 3) 递归检查类型体成员（包含 nested types）。
    let Some(body) = &decl.body else {
        return Ok(());
    };
    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                check_annotation_uses(source, file, index, env, &v.annotations)?;
                reject_builtin_annotations_on_target(source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                check_annotation_uses(source, file, index, env, &p.annotations)?;
                reject_builtin_annotations_on_target(source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                check_annotation_uses(source, file, index, env, &ctor.annotations)?;
                reject_builtin_annotations_on_target(source, &ctor.annotations, "constructor")?;
            }
            ast::TypeMember::Fun(fun) => {
                check_annotation_uses(source, file, index, env, &fun.annotations)?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(source, file, index, env, nested, &type_fqn)?;
            }
            ast::TypeMember::Object(obj) => {
                check_object_decl_annotations(source, file, index, env, obj, &type_fqn)?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

/// 检查 object 声明上的注解，并递归检查其类型体成员（含 nested type/object）。
fn check_object_decl_annotations(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    obj: &ast::ObjectDecl,
    prefix: &str,
) -> Result<(), AnnotationError> {
    // object 自身的注解使用。
    check_annotation_uses(source, file, index, env, &obj.annotations)?;
    check_builtin_annotations_on_object_decl(source, obj)?;

    let Some(body) = &obj.body else {
        return Ok(());
    };

    // 为递归处理 nested type/object 计算容器前缀（与 TypeEnv 的收集规则对齐）。
    let local_name = match &obj.name {
        Some(name) => source.slice(name.span).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => {
                // parser 会拒绝 `object { ... }`；这里作为健壮性兜底不深入。
                return Ok(());
            }
        },
    };
    let obj_fqn = join_prefix(prefix, &local_name);

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(v) => {
                check_annotation_uses(source, file, index, env, &v.annotations)?;
                reject_builtin_annotations_on_target(source, &v.annotations, "enum variant")?;
            }
            ast::TypeMember::Property(p) => {
                check_annotation_uses(source, file, index, env, &p.annotations)?;
                reject_builtin_annotations_on_target(source, &p.annotations, "property")?;
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                check_annotation_uses(source, file, index, env, &ctor.annotations)?;
                reject_builtin_annotations_on_target(source, &ctor.annotations, "constructor")?;
            }
            ast::TypeMember::Fun(fun) => {
                check_annotation_uses(source, file, index, env, &fun.annotations)?;
                check_builtin_annotations_on_fun_decl(source, fun)?;
            }
            ast::TypeMember::Type(nested) => {
                check_type_decl_annotations(source, file, index, env, nested, &obj_fqn)?;
            }
            ast::TypeMember::Object(nested) => {
                check_object_decl_annotations(source, file, index, env, nested, &obj_fqn)?;
            }
            ast::TypeMember::InitBlock(_b) => {}
        }
    }

    Ok(())
}

/// 注解类（`annotation class`）的最小形态约束（data-only）。
fn check_annotation_class_decl_rules(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    // spec §15.2：语法为 `annotation class ...`，当前阶段只支持 class。
    if decl.kind != ast::TypeKind::Class {
        return Err(AnnotationError::AnnotationClassMustBeClass {
            type_fqn: type_fqn.to_string(),
            span: decl.name.span.into(),
        });
    }

    // 当前阶段（T1002）：annotation class 作为 data-only 容器，不允许 implements/extends。
    if let Some(st) = decl.supertypes.first() {
        return Err(AnnotationError::AnnotationClassSupertypesNotSupported {
            type_fqn: type_fqn.to_string(),
            span: st.span.into(),
        });
    }

    // 当前阶段（T1002）：不解析/不支持注解类的类型体成员（方法/属性等）。
    if let Some(body) = &decl.body {
        return Err(AnnotationError::AnnotationClassBodyNotSupported {
            type_fqn: type_fqn.to_string(),
            span: body.span.into(),
        });
    }

    // spec §15.2：所有参数必须是 `val`（immutable）。
    if let Some(primary_ctor) = &decl.primary_ctor {
        for p in &primary_ctor.params {
            if p.kind != Some(ast::ValKind::Val) {
                let param = source.slice(p.name.span).to_string();
                return Err(AnnotationError::AnnotationClassParamMustBeVal {
                    type_fqn: type_fqn.to_string(),
                    param,
                    span: p.name.span.into(),
                });
            }
        }
    }

    Ok(())
}

/// 批量检查一组注解使用（`@Name(...)`）。
fn check_annotation_uses(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    annotations: &[ast::AnnotationUse],
) -> Result<(), AnnotationError> {
    for a in annotations {
        check_one_annotation_use(source, file, index, env, a)?;
    }
    Ok(())
}

/// 检查单个注解使用：解析注解名并确认其引用一个注解类。
fn check_one_annotation_use(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    env: &TypeEnv,
    ann: &ast::AnnotationUse,
) -> Result<(), AnnotationError> {
    let (name, name_span) = annotation_name_and_span(source, ann);

    // T1003：内建注解（`@Unsafe/@NoGC/@Extern/@Intrinsic`）由编译器识别，
    // 不要求存在对应的 `annotation class` 声明。
    if builtin_annotation_kind(source, ann).is_some() {
        return Ok(());
    }

    // 复用 Index 的“按 package/import 规则解析类型名”的逻辑来解析注解类型。
    let ty = ast::TypeRef::Path(ast::TypePath {
        span: ann.span,
        segments: ann.path.clone(),
        args: Vec::new(),
    });

    let Some(fqn) = index.type_ref_to_fqn_in_file(source, file, &ty) else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name,
            span: name_span.into(),
        });
    };

    let Some(sym) = env.type_symbol(&fqn) else {
        return Err(AnnotationError::UnresolvedAnnotationType {
            name,
            span: name_span.into(),
        });
    };

    if !sym.is_annotation_class {
        return Err(AnnotationError::AnnotationTypeIsNotAnnotationClass {
            name: fqn,
            use_span: name_span.into(),
            decl_span: sym.span.into(),
        });
    }

    Ok(())
}

fn reject_builtin_annotations_on_target(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
    found: &'static str,
) -> Result<(), AnnotationError> {
    for ann in annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        let (_, name_span) = annotation_name_and_span(source, ann);
        return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
            annotation: format!("@{}", kind.name()),
            allowed: kind.allowed_targets_hint(),
            found,
            span: name_span.into(),
        });
    }
    Ok(())
}

fn check_builtin_annotations_on_fun_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);

    // 1) `@Unsafe/@NoGC/@Intrinsic` 当前不支持参数；`@Extern` 允许最多一个字符串字面量参数。
    for ann in &fun.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        match kind {
            BuiltinAnnotationKind::Extern => {
                if ann.args.is_empty() {
                    continue;
                }
                if ann.args.len() != 1 {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: ann.span.into(),
                    });
                }
                let arg = &ann.args[0];
                if arg.name.is_some() {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: arg.span.into(),
                    });
                }
                if !matches!(arg.value.kind, ast::ExprKind::StringLit) {
                    return Err(AnnotationError::ExternAnnotationArgsInvalid {
                        span: arg.span.into(),
                    });
                }
            }
            BuiltinAnnotationKind::Unsafe
            | BuiltinAnnotationKind::NoGC
            | BuiltinAnnotationKind::Intrinsic => {
                if !ann.args.is_empty() {
                    let (_, name_span) = annotation_name_and_span(source, ann);
                    return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                        annotation: format!("@{}", kind.name()),
                        span: name_span.into(),
                    });
                }
            }
        }
    }

    // 2) `@Extern/@Intrinsic`：声明必须省略函数体（实现由外部/编译器提供）。
    if flags.is_extern {
        if let ast::FunBody::Block(b) = &fun.body {
            let fun_name = source.slice(fun.name.span).to_string();
            return Err(AnnotationError::ExternFunMustHaveNoBody {
                fun_name,
                span: b.span.into(),
            });
        }
    }
    if flags.is_intrinsic {
        if let ast::FunBody::Block(b) = &fun.body {
            let fun_name = source.slice(fun.name.span).to_string();
            return Err(AnnotationError::IntrinsicFunMustHaveNoBody {
                fun_name,
                span: b.span.into(),
            });
        }
    }

    Ok(())
}

fn check_builtin_annotations_on_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    type_fqn: &str,
) -> Result<(), AnnotationError> {
    let flags = BuiltinAnnotationFlags::from_annotations(source, &decl.annotations);

    // type 声明目前只允许 `@Intrinsic`；其它内建注解的 target 语义留到后续任务补齐。
    for ann in &decl.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind != BuiltinAnnotationKind::Intrinsic {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                annotation: format!("@{}", kind.name()),
                allowed: kind.allowed_targets_hint(),
                found: "type",
                span: name_span.into(),
            });
        }
        if !ann.args.is_empty() {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationArgsNotSupported {
                annotation: format!("@{}", kind.name()),
                span: name_span.into(),
            });
        }
    }

    // spec §15.7：intrinsic declarations have signatures but no bodies。
    // 当前阶段只对“成员函数是否带 body”做最小门禁（不涉及 codegen lowering）。
    if flags.is_intrinsic {
        let Some(body) = &decl.body else {
            return Ok(());
        };
        for m in &body.members {
            let ast::TypeMember::Fun(fun) = m else {
                continue;
            };
            if let ast::FunBody::Block(b) = &fun.body {
                let fun_name = source.slice(fun.name.span).to_string();
                return Err(AnnotationError::IntrinsicMemberFunMustHaveNoBody {
                    type_fqn: type_fqn.to_string(),
                    fun_name,
                    span: b.span.into(),
                });
            }
        }
    }

    Ok(())
}

fn check_builtin_annotations_on_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
) -> Result<(), AnnotationError> {
    for ann in &obj.annotations {
        let Some(kind) = builtin_annotation_kind(source, ann) else {
            continue;
        };
        if kind != BuiltinAnnotationKind::Extern {
            let (_, name_span) = annotation_name_and_span(source, ann);
            return Err(AnnotationError::BuiltinAnnotationInvalidTarget {
                annotation: format!("@{}", kind.name()),
                allowed: kind.allowed_targets_hint(),
                found: "object",
                span: name_span.into(),
            });
        }
        // `@Extern` 目前仅允许：无参或单字符串字面量（保留给后续 FFI/链接语义）。
        if ann.args.is_empty() {
            continue;
        }
        if ann.args.len() != 1 {
            return Err(AnnotationError::ExternAnnotationArgsInvalid {
                span: ann.span.into(),
            });
        }
        let arg = &ann.args[0];
        if arg.name.is_some() || !matches!(arg.value.kind, ast::ExprKind::StringLit) {
            return Err(AnnotationError::ExternAnnotationArgsInvalid {
                span: arg.span.into(),
            });
        }
    }

    Ok(())
}

/// 从 AST 的 `AnnotationUse` 构造用于诊断与解析的“名字 + 标注 span”。
fn annotation_name_and_span(source: &SourceFile, ann: &ast::AnnotationUse) -> (String, Span) {
    let name = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>()
        .join(".");
    let span = ann.path.first().map(|id| id.span).unwrap_or(ann.span);
    (name, span)
}

/// 计算当前文件的 package 前缀（`package a.b.c` → `"a.b.c"`）。
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

/// 拼接 FQN：`prefix` 为空时返回 `name`，否则返回 `"{prefix}.{name}"`。
fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}
