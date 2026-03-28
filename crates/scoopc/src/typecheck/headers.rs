//! 顶层声明头检查（T0404）。
//!
//! 目标：
//! - 不进入函数体/initializer（表达式类型检查留给后续任务）
//! - 先把“类型环境 + 错误诊断”的骨架跑通，尽早对明显的签名问题报错

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;

/// 顶层声明头检查错误（不含表达式类型检查）。
#[derive(Debug, Error, Diagnostic)]
pub enum TypeHeaderError {
    /// 某个“声明位置”缺少类型注解（例如函数参数、顶层 val/var、属性等）。
    #[error("{kind} 缺少类型注解：{name}")]
    #[diagnostic(code(scoop::typecheck::missing_type_annotation))]
    MissingTypeAnnotation {
        kind: &'static str,
        name: String,
        #[label("这里需要写 `: Type`")]
        span: miette::SourceSpan,
    },

    /// 当前阶段不支持对 `val (a, b) = ...` 这类 pattern 绑定做类型检查。
    #[error("暂不支持的模式绑定（pattern binding）")]
    #[diagnostic(code(scoop::typecheck::unsupported_pattern_binding))]
    UnsupportedPatternBinding {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// `const fun` 的语法限制：当前阶段要求其 effect row 只能为 Pure（或缺省）。
    ///
    /// 说明：
    /// - spec §6.2：const 计算应为纯计算；
    /// - 早期阶段先做“声明级”的最小门禁，避免后续解释器入口难以界定；
    /// - 更细粒度的规则（例如禁止在 body 内 perform/raise 等）可在 comptime 解释器任务中逐步补齐。
    #[error("const fun 不允许声明非 Pure 的 effect row")]
    #[diagnostic(code(scoop::typecheck::const_fun_effects_not_allowed))]
    ConstFunEffectsNotAllowed {
        #[label("const fun 必须为 Pure（或不写 effect row）")]
        span: miette::SourceSpan,
    },

    /// `const fun` 不允许声明 effect row 参数（`<eff E = ...>`）。
    #[error("const fun 不允许声明 effect row 参数")]
    #[diagnostic(code(scoop::typecheck::const_fun_eff_param_not_allowed))]
    ConstFunEffParamNotAllowed {
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 检查一个文件内的“声明头”是否满足当前 typecheck 阶段的最小约束。
///
/// 说明：
/// - 该检查不依赖表达式 AST 是否完整；
/// - 该检查不做任何“类型匹配/推断”，仅保证签名信息足够进入后续 typecheck pass。
pub fn check_file_headers(source: &SourceFile, file: &ast::File) -> Result<(), TypeHeaderError> {
    for item in &file.items {
        match item {
            ast::Item::TypeAlias(_ta) => {
                // typealias 的 rhs 类型合法性由 `check_file_type_refs`（T0403）负责；
                // 此处仅做“签名形态”的检查。
            }
            ast::Item::Fun(fun) => check_fun_header(source, fun)?,
            ast::Item::ExtensionProperty(p) => check_extension_property_header(source, p)?,
            ast::Item::Val(v) => check_top_level_val_header(source, v)?,
            ast::Item::Type(ty) => check_type_decl_headers(source, ty)?,
            ast::Item::Object(obj) => check_object_decl_headers(source, obj)?,
        }
    }
    Ok(())
}

fn check_fun_header(source: &SourceFile, fun: &ast::FunDecl) -> Result<(), TypeHeaderError> {
    let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
    if is_const_fun {
        if let Some(eff_param) = &fun.eff_param {
            return Err(TypeHeaderError::ConstFunEffParamNotAllowed {
                span: eff_param.span.into(),
            });
        }

        if let Some(effects) = &fun.effects {
            // 允许：
            // - 缺省（None）
            // - 显式 Pure（`/ Pure` 或 `/ Pure!`）：`terms.is_empty()`
            //
            // 不允许：
            // - 任何非空 effect row（例如 `/ Raise<E>` / `/ IO+State!`）
            if !effects.terms.is_empty() {
                return Err(TypeHeaderError::ConstFunEffectsNotAllowed {
                    span: effects.span.into(),
                });
            }
        }
    }

    for p in &fun.params {
        if p.ty.is_none() {
            let name = source.slice(p.name.span).to_string();
            return Err(TypeHeaderError::MissingTypeAnnotation {
                kind: "参数",
                name,
                span: p.name.span.into(),
            });
        }
    }
    Ok(())
}

fn check_extension_property_header(
    source: &SourceFile,
    p: &ast::ExtensionPropertyDecl,
) -> Result<(), TypeHeaderError> {
    if p.ty.is_none() {
        let name = source.slice(p.name.span).to_string();
        return Err(TypeHeaderError::MissingTypeAnnotation {
            kind: "扩展属性",
            name,
            span: p.name.span.into(),
        });
    }
    Ok(())
}

fn check_top_level_val_header(source: &SourceFile, v: &ast::ValDecl) -> Result<(), TypeHeaderError> {
    match &v.binding {
        ast::ValBinding::Name(name) => {
            if v.ty.is_none() {
                let name_str = source.slice(name.span).to_string();
                return Err(TypeHeaderError::MissingTypeAnnotation {
                    kind: "顶层变量",
                    name: name_str,
                    span: name.span.into(),
                });
            }
        }
        ast::ValBinding::Pattern(pat) => {
            return Err(TypeHeaderError::UnsupportedPatternBinding {
                span: pat.span.into(),
            });
        }
    }

    Ok(())
}

fn check_type_decl_headers(source: &SourceFile, ty: &ast::TypeDecl) -> Result<(), TypeHeaderError> {
    // 主构造头参数类型：class/struct 等语法位置强依赖类型注解（当前阶段不做推断）。
    if let Some(primary_ctor) = &ty.primary_ctor {
        for p in &primary_ctor.params {
            if p.ty.is_none() {
                let name = source.slice(p.name.span).to_string();
                return Err(TypeHeaderError::MissingTypeAnnotation {
                    kind: "构造参数",
                    name,
                    span: p.name.span.into(),
                });
            }
        }
    }

    // 类型体成员签名：property/fun/nested type。
    let Some(body) = &ty.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(_v) => {
                // enum variant 字段的 `: Type` 约束由 parser 保证；
                // 更完整的 enum 语义检查留给 rich enum 任务（T0425+）。
            }
            ast::TypeMember::Property(p) => {
                if p.ty.is_none() {
                    let name = source.slice(p.name.span).to_string();
                    return Err(TypeHeaderError::MissingTypeAnnotation {
                        kind: "属性",
                        name,
                        span: p.name.span.into(),
                    });
                }
            }
            ast::TypeMember::InitBlock(_b) => {
                // init block 不引入新的“签名层”类型需求（它属于初始化执行体），
                // 后续由更完整的 class 初始化语义任务处理（T0313+）。
            }
            ast::TypeMember::SecondaryCtor(ctor) => {
                // 次构造器参数类型：同函数参数一样，当前阶段要求显式 `: Type`。
                for p in &ctor.params {
                    if p.ty.is_none() {
                        let name = source.slice(p.name.span).to_string();
                        return Err(TypeHeaderError::MissingTypeAnnotation {
                            kind: "构造参数",
                            name,
                            span: p.name.span.into(),
                        });
                    }
                }
            }
            ast::TypeMember::Fun(f) => check_fun_header(source, f)?,
            ast::TypeMember::Type(nested) => check_type_decl_headers(source, nested)?,
            ast::TypeMember::Object(obj) => check_object_decl_headers(source, obj)?,
        }
    }

    Ok(())
}

fn check_object_decl_headers(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
) -> Result<(), TypeHeaderError> {
    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(_v) => {}
            ast::TypeMember::Property(p) => {
                if p.ty.is_none() {
                    let name = source.slice(p.name.span).to_string();
                    return Err(TypeHeaderError::MissingTypeAnnotation {
                        kind: "属性",
                        name,
                        span: p.name.span.into(),
                    });
                }
            }
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                for p in &ctor.params {
                    if p.ty.is_none() {
                        let name = source.slice(p.name.span).to_string();
                        return Err(TypeHeaderError::MissingTypeAnnotation {
                            kind: "构造参数",
                            name,
                            span: p.name.span.into(),
                        });
                    }
                }
            }
            ast::TypeMember::Fun(f) => check_fun_header(source, f)?,
            ast::TypeMember::Type(nested) => check_type_decl_headers(source, nested)?,
            ast::TypeMember::Object(nested) => check_object_decl_headers(source, nested)?,
        }
    }

    Ok(())
}
