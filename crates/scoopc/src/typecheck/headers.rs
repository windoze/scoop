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
            ast::Item::Val(v) => check_top_level_val_header(source, v)?,
            ast::Item::Type(ty) => check_type_decl_headers(source, ty)?,
        }
    }
    Ok(())
}

fn check_fun_header(source: &SourceFile, fun: &ast::FunDecl) -> Result<(), TypeHeaderError> {
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
            ast::TypeMember::Fun(f) => check_fun_header(source, f)?,
            ast::TypeMember::Type(nested) => check_type_decl_headers(source, nested)?,
        }
    }

    Ok(())
}
