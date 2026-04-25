//! const/comptime 的表达式求值器（v0）。
//!
//! 约束：
//! - 直接建模的叶子能力仍以字面量、一元/二元运算、aggregate 与若干内建 const path 为主；
//! - block/`if`/assignment 这类需要局部作用域或控制流传播的节点，通过 `ConstEvalHost`
//!   回调到解释器执行；
//! - `when`、effects、`handle/perform`、lambda 等复杂语义仍保持不支持；
//! - 遇到不支持的语法节点必须返回结构化诊断（而非 panic）。

use std::ops::ControlFlow;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::syntax::char_literal::parse_char_literal;
use crate::syntax::float_literal::{FloatLiteralSuffix, parse_float_literal};
use crate::syntax::int_literal::parse_int_literal;
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_bytes};

use super::value::{
    ConstEnum, ConstFloat, ConstFloatTy, ConstInt, ConstIntTy, ConstStruct, ConstValue,
    mask_to_bits,
};

/// 求值上下文（v0）。
#[derive(Debug, Clone, Copy)]
pub struct ConstEvalCtx<'a> {
    pub source: &'a SourceFile,
    /// 在缺少类型信息时，Int 字面量默认采用的整数类型（位宽/符号位）。
    pub default_int_ty: ConstIntTy,
}

impl<'a> ConstEvalCtx<'a> {
    /// 用默认配置创建一个求值上下文：
    /// - Int 默认为宿主机 word-sized signed（与当前后端 Int 映射一致）。
    pub fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            default_int_ty: ConstIntTy::host_word(true),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum ConstEvalError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Frontend(#[from] Box<dyn miette::Diagnostic + Send + Sync>),

    #[error("暂不支持的常量表达式：{kind}")]
    #[diagnostic(code(scoop::comptime::unsupported_expr))]
    UnsupportedExpr {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未定义的标识符：{name}")]
    #[diagnostic(code(scoop::comptime::unknown_ident))]
    UnknownIdent {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("常量表达式操作数类型不匹配：期望 {expected}，但得到 {found}")]
    #[diagnostic(code(scoop::comptime::operand_type_mismatch))]
    OperandTypeMismatch {
        expected: &'static str,
        found: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("整数除以 0")]
    #[diagnostic(code(scoop::comptime::int_div_by_zero))]
    IntDivByZero {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("非法的 shift 数量：{value}（必须为非负）")]
    #[diagnostic(code(scoop::comptime::invalid_shift_count))]
    InvalidShiftCount {
        value: i128,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("字符串字面量非法（不支持插值字符串或包含无效转义/UTF-8）")]
    #[diagnostic(code(scoop::comptime::invalid_string_literal))]
    InvalidStringLiteral {
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未知的成员：{name}")]
    #[diagnostic(code(scoop::comptime::unknown_member))]
    UnknownMember {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("tuple 元素索引越界：{index}（tuple 长度为 {len}）")]
    #[diagnostic(code(scoop::comptime::tuple_index_out_of_bounds))]
    TupleIndexOutOfBounds {
        index: usize,
        len: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的常量语句：{kind}")]
    #[diagnostic(code(scoop::comptime::unsupported_stmt))]
    UnsupportedStmt {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("{kind} 缺少 initializer")]
    #[diagnostic(code(scoop::comptime::missing_initializer))]
    MissingInitializer {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未找到可调用的 const fun：{name}")]
    #[diagnostic(code(scoop::comptime::unknown_const_fun))]
    UnknownConstFun {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("只能调用 const fun：{name}")]
    #[diagnostic(code(scoop::comptime::callee_not_const_fun))]
    CalleeNotConstFun {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 参数个数不匹配：{name} 期望 {expected} 个参数，但得到 {found} 个")]
    #[diagnostic(code(scoop::comptime::const_fun_arity_mismatch))]
    ConstFunArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 调用歧义：{name}（参数个数为 {arity}）")]
    #[diagnostic(code(scoop::comptime::const_fun_ambiguous))]
    ConstFunAmbiguous {
        name: String,
        arity: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("const fun 递归深度超限：{name}（limit={limit}）")]
    #[diagnostic(code(scoop::comptime::recursion_limit_exceeded))]
    RecursionLimitExceeded {
        name: String,
        limit: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的 const fun 签名：{reason}")]
    #[diagnostic(code(scoop::comptime::unsupported_const_fun_signature))]
    UnsupportedConstFunSignature {
        reason: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    // --- reflection intrinsics（T1204） ---
    #[error("反射 intrinsic `{name}` 调用不合法：{reason}")]
    #[diagnostic(code(scoop::comptime::reflection_bad_call))]
    ReflectionBadCall {
        name: String,
        reason: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("反射 intrinsic 的类型实参暂不支持：{found}")]
    #[diagnostic(code(scoop::comptime::reflection_type_arg_not_supported))]
    ReflectionTypeArgNotSupported {
        found: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未找到可反射的类型：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_unknown_type))]
    ReflectionUnknownType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型名歧义：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_ambiguous_type))]
    ReflectionAmbiguousType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持对该类型执行反射：{name}（期望 struct/class）")]
    #[diagnostic(code(scoop::comptime::reflection_unsupported_target))]
    ReflectionUnsupportedTarget {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("sizeOf<T>() 暂不支持该类型：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_sizeof_unsupported_type))]
    ReflectionSizeOfUnsupportedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("alignOf<T>() 暂不支持该类型：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_alignof_unsupported_type))]
    ReflectionAlignOfUnsupportedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("variantsOf<T>() 暂不支持该类型：{name}（期望 enum）")]
    #[diagnostic(code(scoop::comptime::reflection_variants_of_unsupported_target))]
    ReflectionVariantsOfUnsupportedTarget {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未找到可反射的函数：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_unknown_function))]
    ReflectionUnknownFunction {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("函数名歧义：{name}")]
    #[diagnostic(code(scoop::comptime::reflection_ambiguous_function))]
    ReflectionAmbiguousFunction {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("fieldsOf<T>() 发现重复字段：{field}")]
    #[diagnostic(code(scoop::comptime::reflection_duplicate_field))]
    ReflectionDuplicateField {
        field: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 常量求值时用于“外部交互”的宿主接口：
/// - 解释器可通过它解析局部/全局名字；
/// - 并把 `f(...)` 的调用委托给宿主（用于 `const fun`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvalControl {
    Return(ConstValue),
    Break,
    Continue,
}

pub(crate) type ConstExprFlow = ControlFlow<EvalControl, ConstValue>;

pub(crate) trait ConstEvalHost {
    fn resolve_ident(&mut self, name: &str) -> Option<ConstValue>;
    fn call_fun(
        &mut self,
        call_span: crate::span::Span,
        callee_name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError>;

    fn eval_block_expr(
        &mut self,
        _ctx: ConstEvalCtx<'_>,
        expr_span: crate::span::Span,
        _block: &ast::Block,
    ) -> Result<ConstExprFlow, ConstEvalError> {
        Err(ConstEvalError::UnsupportedExpr {
            kind: "block expression",
            span: expr_span.into(),
        })
    }

    fn assign_expr(
        &mut self,
        _ctx: ConstEvalCtx<'_>,
        expr_span: crate::span::Span,
        _lhs: &ast::Expr,
        _rhs: ConstValue,
    ) -> Result<ConstExprFlow, ConstEvalError> {
        Err(ConstEvalError::UnsupportedExpr {
            kind: "assignment",
            span: expr_span.into(),
        })
    }
}

struct NoHost;

impl ConstEvalHost for NoHost {
    fn resolve_ident(&mut self, _name: &str) -> Option<ConstValue> {
        None
    }

    fn call_fun(
        &mut self,
        call_span: crate::span::Span,
        _callee_name: &str,
        _type_args: Vec<ast::TypeRef>,
        _args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        Err(ConstEvalError::UnsupportedExpr {
            kind: "call expression",
            span: call_span.into(),
        })
    }
}

fn unexpected_control_flow_error(span: crate::span::Span, control: EvalControl) -> ConstEvalError {
    let kind = match control {
        EvalControl::Return(_) => "return expression",
        EvalControl::Break => "break expression",
        EvalControl::Continue => "continue expression",
    };
    ConstEvalError::UnsupportedExpr {
        kind,
        span: span.into(),
    }
}

/// 对 AST 表达式做最小常量求值（T1202a/T1202b）。
///
/// 说明：该函数不支持 `const fun` 调用；调用/局部变量由解释器（T1202c）通过
/// `eval_const_expr_with_host` 注入环境与调用能力。
pub fn eval_const_expr(
    ctx: ConstEvalCtx<'_>,
    expr: &ast::Expr,
) -> Result<ConstValue, ConstEvalError> {
    let mut host = NoHost;
    eval_const_expr_with_host(ctx, &mut host, expr)
}

macro_rules! eval_value {
    ($ctx:expr, $host:expr, $expr:expr) => {
        match eval_const_expr_flow_with_host($ctx, $host, $expr)? {
            ControlFlow::Continue(v) => v,
            ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
        }
    };
}

pub(crate) fn eval_const_expr_with_host(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    expr: &ast::Expr,
) -> Result<ConstValue, ConstEvalError> {
    match eval_const_expr_flow_with_host(ctx, host, expr)? {
        ControlFlow::Continue(v) => Ok(v),
        ControlFlow::Break(control) => Err(unexpected_control_flow_error(expr.span, control)),
    }
}

pub(crate) fn eval_const_expr_flow_with_host(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    expr: &ast::Expr,
) -> Result<ConstExprFlow, ConstEvalError> {
    match &expr.kind {
        ast::ExprKind::Missing => Err(ConstEvalError::UnsupportedExpr {
            kind: "missing",
            span: expr.span.into(),
        }),
        ast::ExprKind::IntLit => {
            let text = ctx.source.slice(expr.span);
            let raw = parse_int_literal(text);
            Ok(ControlFlow::Continue(ConstValue::Int(ConstInt::new(
                ctx.default_int_ty,
                raw,
            ))))
        }
        ast::ExprKind::FloatLit => {
            let text = ctx.source.slice(expr.span);
            let parsed = parse_float_literal(text);
            let value = match parsed.suffix {
                FloatLiteralSuffix::Float64 => ConstFloat::from_f64(parsed.value),
                FloatLiteralSuffix::Float32 => ConstFloat::from_f32(parsed.value as f32),
            };
            Ok(ControlFlow::Continue(ConstValue::Float(value)))
        }
        ast::ExprKind::CharLit => {
            let text = ctx.source.slice(expr.span);
            let value =
                parse_char_literal(text).map_err(|_err| ConstEvalError::UnsupportedExpr {
                    kind: "char literal",
                    span: expr.span.into(),
                })?;
            Ok(ControlFlow::Continue(ConstValue::Char(value)))
        }
        ast::ExprKind::StringLit => {
            let text = ctx.source.slice(expr.span);
            let bytes = match parse_string_literal_bytes(text) {
                Ok(bytes) => bytes,
                Err(StringLiteralParseError::Interpolated) => {
                    return Err(ConstEvalError::InvalidStringLiteral {
                        span: expr.span.into(),
                    });
                }
                Err(StringLiteralParseError::Invalid | StringLiteralParseError::InvalidUtf8) => {
                    return Err(ConstEvalError::InvalidStringLiteral {
                        span: expr.span.into(),
                    });
                }
            };
            let s =
                String::from_utf8(bytes).map_err(|_e| ConstEvalError::InvalidStringLiteral {
                    span: expr.span.into(),
                })?;
            Ok(ControlFlow::Continue(ConstValue::String(s)))
        }
        ast::ExprKind::UnitLit => Ok(ControlFlow::Continue(ConstValue::Unit)),

        // `true/false` 当前阶段仍用 Ident 承载（见 typecheck::infer_value_ident_type）。
        ast::ExprKind::Ident(id) => {
            let name = ctx.source.slice(id.span);
            match name {
                "true" => Ok(ControlFlow::Continue(ConstValue::Bool(true))),
                "false" => Ok(ControlFlow::Continue(ConstValue::Bool(false))),
                other => match host.resolve_ident(other) {
                    Some(v) => Ok(ControlFlow::Continue(v)),
                    None => Err(ConstEvalError::UnknownIdent {
                        name: name.to_string(),
                        span: id.span.into(),
                    }),
                },
            }
        }

        ast::ExprKind::Unary {
            op, expr: inner, ..
        } => {
            let v = eval_value!(ctx, host, inner);
            Ok(ControlFlow::Continue(eval_unary(expr.span, *op, v)?))
        }
        ast::ExprKind::Binary { lhs, op, rhs, .. } => {
            eval_binary(ctx, host, expr.span, *op, lhs, rhs)
        }

        // aggregates（T1202b）
        ast::ExprKind::TupleLit { elements } => {
            let mut out: Vec<ConstValue> = Vec::with_capacity(elements.len());
            for e in elements {
                out.push(eval_value!(ctx, host, e));
            }
            Ok(ControlFlow::Continue(ConstValue::Tuple(out)))
        }
        ast::ExprKind::ArrayLit { elements } => {
            // v0：编译期执行侧先把 array literal 视为“可迭代的常量序列”。
            // 目前 ConstValue 未区分 tuple/array（两者都用 Tuple 承载），
            // 主要用于 `comptime for` 的迭代对象（T1207）。
            let mut out: Vec<ConstValue> = Vec::with_capacity(elements.len());
            for e in elements {
                out.push(eval_value!(ctx, host, e));
            }
            Ok(ControlFlow::Continue(ConstValue::Tuple(out)))
        }
        ast::ExprKind::InterpolatedString { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "interpolated string",
            span: expr.span.into(),
        }),
        ast::ExprKind::Block(body) => host.eval_block_expr(ctx, expr.span, body),
        ast::ExprKind::DoBlock { body, .. } => host.eval_block_expr(ctx, expr.span, body),
        ast::ExprKind::UnsafeBlock { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "@Unsafe block",
            span: expr.span.into(),
        }),
        ast::ExprKind::SafeBlock { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "@Safe block",
            span: expr.span.into(),
        }),
        ast::ExprKind::Lambda(_) => Err(ConstEvalError::UnsupportedExpr {
            kind: "lambda",
            span: expr.span.into(),
        }),
        ast::ExprKind::ClassLit { ty } => {
            // T1019/T1218：class literal 视为“编译期可用的类型名常量”。
            //
            // 说明：
            // - 早期阶段 const eval 不接入完整 name resolution/type env；
            // - 因此这里输出的是“语法层面”的稳定名字（基于 TypeRef），用于注解参数与回归测试；
            // - 后续若接入 resolver/typecheck，可升级为 FQN / TypeMeta 等更强语义。
            let Some(name) = type_ref_name_for_class_literal(ctx.source, ty) else {
                return Err(ConstEvalError::UnsupportedExpr {
                    kind: "class literal",
                    span: expr.span.into(),
                });
            };
            Ok(ControlFlow::Continue(ConstValue::String(name)))
        }
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_v = eval_value!(ctx, host, cond);
            let ConstValue::Bool(cond_b) = cond_v else {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "Bool",
                    found: value_kind(&cond_v),
                    span: cond.span.into(),
                });
            };
            if cond_b {
                eval_const_expr_flow_with_host(ctx, host, then_branch)
            } else if let Some(else_branch) = else_branch {
                eval_const_expr_flow_with_host(ctx, host, else_branch)
            } else {
                Ok(ControlFlow::Continue(ConstValue::Unit))
            }
        }
        ast::ExprKind::When { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "when expression",
            span: expr.span.into(),
        }),
        ast::ExprKind::Handle { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "handle expression",
            span: expr.span.into(),
        }),
        ast::ExprKind::Async { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "async expression",
            span: expr.span.into(),
        }),
        ast::ExprKind::StructLit { ty, fields } => {
            let ty_name = type_path_name(ctx.source, ty);
            let mut out_fields = std::collections::BTreeMap::<String, ConstValue>::new();
            for f in fields {
                let name = f.name.text(ctx.source).to_string();
                let value = eval_value!(ctx, host, &f.value);
                out_fields.insert(name, value);
            }
            Ok(ControlFlow::Continue(ConstValue::Struct(ConstStruct {
                ty: ty_name,
                fields: out_fields,
            })))
        }

        ast::ExprKind::MemberAccess { receiver, member } => {
            // enum unit variant 值：`EnumName.Variant`
            //
            // 说明：
            // - 该表达式的 receiver 在语义上是“类型名/命名空间入口”，并非运行期值；
            // - const eval 早期阶段没有完整 name resolution/type env，因此这里用非常保守的启发式：
            //   当最后两段（type + variant）都长得像 “TypeLike”（首字母大写）时，将其视为 unit variant 构造。
            //
            // 额外约束：
            // - 若路径首段能被 host 解析为一个运行期值，则优先把它当作普通 member access（避免误判）。
            if let Some((ty, variant)) = try_parse_enum_unit_variant_path(ctx.source, host, expr) {
                return Ok(ControlFlow::Continue(ConstValue::Enum(ConstEnum {
                    ty: Some(ty),
                    variant,
                    payload: Vec::new(),
                })));
            }

            let recv = eval_value!(ctx, host, receiver);
            Ok(ControlFlow::Continue(eval_member_access(
                ctx, recv, member, expr.span,
            )?))
        }

        ast::ExprKind::SpliceField { receiver, field } => {
            // splice 字段访问：`receiver.[field]`（spec §6.4）
            //
            // v0：先允许 field 为以下两类编译期值：
            // - `String`：字段名
            // - `Struct` 且包含 `name: String`：为后续 FieldMeta 兼容预留
            let recv = eval_value!(ctx, host, receiver);
            let field_v = eval_value!(ctx, host, field);

            let field_name: String = match &field_v {
                ConstValue::String(s) => s.clone(),
                ConstValue::Struct(ConstStruct { fields, .. }) => match fields.get("name") {
                    Some(ConstValue::String(s)) => s.clone(),
                    _ => {
                        return Err(ConstEvalError::OperandTypeMismatch {
                            expected: "String（字段名）或 FieldMeta{name:String}",
                            found: value_kind(&field_v),
                            span: field.span.into(),
                        });
                    }
                },
                _ => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "String（字段名）或 FieldMeta{name:String}",
                        found: value_kind(&field_v),
                        span: field.span.into(),
                    });
                }
            };

            match recv {
                ConstValue::Struct(s) => {
                    let value = s.fields.get(&field_name).cloned().ok_or_else(|| {
                        ConstEvalError::UnknownMember {
                            name: field_name,
                            span: field.span.into(),
                        }
                    })?;
                    Ok(ControlFlow::Continue(value))
                }
                _ => Err(ConstEvalError::UnsupportedExpr {
                    kind: "splice field access（receiver 必须为 struct 常量）",
                    span: expr.span.into(),
                }),
            }
        }

        ast::ExprKind::Call { callee, args } => {
            // String 方法 intrinsics（T0123）：
            // 当 receiver 为编译期常量（ConstValue::String）时，在编译期执行并折叠。
            if let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind {
                let member_name = member_simple_name(ctx.source, member);
                match try_eval_string_method_intrinsic(
                    ctx,
                    host,
                    expr.span,
                    receiver,
                    member_name,
                    args,
                )? {
                    ControlFlow::Continue(Some(result)) => {
                        return Ok(ControlFlow::Continue(result));
                    }
                    ControlFlow::Continue(None) => {}
                    ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
                }
                match try_eval_float_method_intrinsic(
                    ctx,
                    host,
                    expr.span,
                    receiver,
                    member_name,
                    args,
                )? {
                    ControlFlow::Continue(Some(result)) => {
                        return Ok(ControlFlow::Continue(result));
                    }
                    ControlFlow::Continue(None) => {}
                    ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
                }
                match try_eval_char_method_intrinsic(
                    ctx,
                    host,
                    expr.span,
                    receiver,
                    member_name,
                    args,
                )? {
                    ControlFlow::Continue(Some(result)) => {
                        return Ok(ControlFlow::Continue(result));
                    }
                    ControlFlow::Continue(None) => {}
                    ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
                }
            }

            // 显式类型实参调用（T1204）：`nameOf<T>()` / `fieldsOf<T>()` / `sizeOf<T>()`。
            if let ast::ExprKind::TypeApply {
                callee: inner,
                args: type_args,
            } = &callee.kind
            {
                let ast::ExprKind::Ident(id) = &inner.kind else {
                    return Err(ConstEvalError::UnsupportedExpr {
                        kind: "generic call callee",
                        span: callee.span.into(),
                    });
                };
                let name = ctx.source.slice(id.span);

                let mut argv: Vec<ConstValue> = Vec::with_capacity(args.len());
                for a in args {
                    argv.push(eval_value!(ctx, host, a));
                }
                return Ok(ControlFlow::Continue(host.call_fun(
                    expr.span,
                    name,
                    type_args.clone(),
                    argv,
                )?));
            }

            // enum ctor（T1202b）：`Opt.Some(1)` / `Some(1)`
            //
            // 规则：
            // - 当 callee 看起来像“类型/variant 名”（大写开头）时，走 enum ctor；
            // - 否则当 callee 为普通 Ident（小写开头）时，委托给宿主做 `const fun` 调用。
            if let ast::ExprKind::Ident(id) = &callee.kind {
                let name = ctx.source.slice(id.span);
                if !looks_like_type_name(name) {
                    let mut argv: Vec<ConstValue> = Vec::with_capacity(args.len());
                    for a in args {
                        argv.push(eval_value!(ctx, host, a));
                    }
                    return Ok(ControlFlow::Continue(host.call_fun(
                        expr.span,
                        name,
                        Vec::new(),
                        argv,
                    )?));
                }
            }

            eval_enum_ctor_call(ctx, host, expr.span, callee, args)
        }
        ast::ExprKind::NamedArg { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "named arg",
            span: expr.span.into(),
        }),
        ast::ExprKind::NotNullAssert { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "not-null assert (!!)",
            span: expr.span.into(),
        }),
        ast::ExprKind::Assign { lhs, rhs, .. } => {
            let rhs_value = eval_value!(ctx, host, rhs);
            host.assign_expr(ctx, expr.span, lhs, rhs_value)
        }
        ast::ExprKind::TypeCheck { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "type check (is/!is)",
            span: expr.span.into(),
        }),
        ast::ExprKind::Cast { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "cast (as/as?)",
            span: expr.span.into(),
        }),
        ast::ExprKind::WithUpdate { .. } => Err(ConstEvalError::UnsupportedExpr {
            kind: "with-update expression",
            span: expr.span.into(),
        }),
        _ => Err(ConstEvalError::UnsupportedExpr {
            kind: "expression kind",
            span: expr.span.into(),
        }),
    }
}

/// String 方法 intrinsics（T0123）。
///
/// 当 receiver 为编译期常量 `ConstValue::String` 时，在编译期执行内建实现并返回结果。
/// 若方法名不匹配或 receiver 非 String，返回 `Ok(None)` 表示"未匹配，由后续路径处理"。
fn try_eval_string_method_intrinsic(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    call_span: crate::span::Span,
    receiver_expr: &ast::Expr,
    method_name: &str,
    args: &[ast::Expr],
) -> Result<ControlFlow<EvalControl, Option<ConstValue>>, ConstEvalError> {
    // 已知的 String 方法名集合——若不匹配则提前返回 None（不影响其它路径）。
    let is_known_string_method = matches!(
        method_name,
        "trimIndent"
            | "byteLength"
            | "getByte"
            | "length"
            | "substring"
            | "indexOf"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "split"
            | "isEmpty"
            | "trim"
            | "trimStart"
            | "trimEnd"
            | "replace"
            | "charAt"
            | "repeat"
            | "compareTo"
            | "concat"
            | "toString"
            | "hash"
    );
    if !is_known_string_method {
        return Ok(ControlFlow::Continue(None));
    }

    // 求值 receiver：若非 String 则不处理。
    let recv = match eval_const_expr_flow_with_host(ctx, host, receiver_expr)? {
        ControlFlow::Continue(v) => v,
        ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
    };
    let ConstValue::String(s) = recv else {
        return Ok(ControlFlow::Continue(None));
    };

    let int_ty = ctx.default_int_ty;
    let mk_int = |v: i64| ConstValue::Int(ConstInt::new(int_ty, v as u128));

    // 求值参数。
    let mut argv: Vec<ConstValue> = Vec::with_capacity(args.len());
    for a in args {
        let value = match eval_const_expr_flow_with_host(ctx, host, a)? {
            ControlFlow::Continue(v) => v,
            ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
        };
        argv.push(value);
    }

    let check_arity = |expected: usize| -> Result<(), ConstEvalError> {
        if argv.len() != expected {
            return Err(ConstEvalError::ConstFunArityMismatch {
                name: method_name.to_string(),
                expected,
                found: argv.len(),
                span: call_span.into(),
            });
        }
        Ok(())
    };

    let arg_string = |idx: usize| -> Result<String, ConstEvalError> {
        match &argv[idx] {
            ConstValue::String(s) => Ok(s.clone()),
            other => Err(ConstEvalError::OperandTypeMismatch {
                expected: "String",
                found: value_kind(other),
                span: call_span.into(),
            }),
        }
    };

    let arg_int = |idx: usize| -> Result<i64, ConstEvalError> {
        match &argv[idx] {
            ConstValue::Int(i) => Ok(i.as_i128() as i64),
            other => Err(ConstEvalError::OperandTypeMismatch {
                expected: "Int",
                found: value_kind(other),
                span: call_span.into(),
            }),
        }
    };

    let result = match method_name {
        // --- 0-arity methods ---
        "trimIndent" => {
            check_arity(0)?;
            ConstValue::String(string_trim_indent_kotlin_like(&s))
        }
        "byteLength" | "length" => {
            check_arity(0)?;
            mk_int(s.len() as i64)
        }
        "isEmpty" => {
            check_arity(0)?;
            ConstValue::Bool(s.is_empty())
        }
        "trim" => {
            check_arity(0)?;
            ConstValue::String(string_trim_ascii_ws(&s))
        }
        "trimStart" => {
            check_arity(0)?;
            ConstValue::String(string_trim_start_ascii_ws(&s))
        }
        "trimEnd" => {
            check_arity(0)?;
            ConstValue::String(string_trim_end_ascii_ws(&s))
        }
        "toString" => {
            check_arity(0)?;
            ConstValue::String(s)
        }
        "hash" => {
            // FNV-1a（与 runtime/c scoop_string_hash 一致）。
            check_arity(0)?;
            let bytes = s.as_bytes();
            if bytes.is_empty() {
                mk_int(0)
            } else {
                let mut h: u64 = 14695981039346656037;
                for &b in bytes {
                    h ^= u64::from(b);
                    h = h.wrapping_mul(1099511628211);
                }
                mk_int(h as i64)
            }
        }

        // --- 1-arity methods ---
        "getByte" => {
            check_arity(1)?;
            let index = arg_int(0)?;
            let bytes = s.as_bytes();
            if index < 0 || (index as usize) >= bytes.len() {
                mk_int(0)
            } else {
                mk_int(i64::from(bytes[index as usize]))
            }
        }
        "charAt" => {
            // charAt(index)：与 runtime 一致，按字节索引返回字节值（-1 if OOB）。
            check_arity(1)?;
            let index = arg_int(0)?;
            let bytes = s.as_bytes();
            if index < 0 || (index as usize) >= bytes.len() {
                mk_int(-1)
            } else {
                mk_int(i64::from(bytes[index as usize]))
            }
        }
        "indexOf" => {
            check_arity(1)?;
            let sub = arg_string(0)?;
            let result = string_index_of(&s, &sub);
            mk_int(result)
        }
        "contains" => {
            check_arity(1)?;
            let sub = arg_string(0)?;
            ConstValue::Bool(string_index_of(&s, &sub) >= 0)
        }
        "startsWith" => {
            check_arity(1)?;
            let prefix = arg_string(0)?;
            ConstValue::Bool(s.as_bytes().starts_with(prefix.as_bytes()))
        }
        "endsWith" => {
            check_arity(1)?;
            let suffix = arg_string(0)?;
            ConstValue::Bool(s.as_bytes().ends_with(suffix.as_bytes()))
        }
        "split" => {
            check_arity(1)?;
            let delim = arg_string(0)?;
            let parts = string_split(&s, &delim);
            ConstValue::Tuple(parts.into_iter().map(ConstValue::String).collect())
        }
        "concat" => {
            check_arity(1)?;
            let other = arg_string(0)?;
            ConstValue::String(s + &other)
        }
        "compareTo" => {
            check_arity(1)?;
            let other = arg_string(0)?;
            let cmp = string_compare_to(&s, &other);
            mk_int(cmp)
        }
        "repeat" => {
            check_arity(1)?;
            let n = arg_int(0)?;
            if n <= 0 || s.is_empty() {
                ConstValue::String(String::new())
            } else {
                ConstValue::String(s.repeat(n as usize))
            }
        }

        // --- 2-arity methods ---
        "substring" => {
            check_arity(2)?;
            let from = arg_int(0)?;
            let to = arg_int(1)?;
            let bytes = s.as_bytes();
            let len = bytes.len() as i64;
            // 与 sysroot/string.scoop 语义一致：clamp 到 [0, len]。
            let start = from.max(0).min(len) as usize;
            let end = to.max(start as i64).min(len) as usize;
            let slice = &bytes[start..end];
            // safety: 子串仍是合法 UTF-8（假设输入合法）。
            ConstValue::String(String::from_utf8_lossy(slice).into_owned())
        }
        "replace" => {
            check_arity(2)?;
            let old = arg_string(0)?;
            let new_s = arg_string(1)?;
            if old.is_empty() {
                ConstValue::String(s)
            } else {
                ConstValue::String(s.replace(&old, &new_s))
            }
        }

        _ => return Ok(ControlFlow::Continue(None)),
    };

    Ok(ControlFlow::Continue(Some(result)))
}

/// Float builtin 方法 intrinsics（T0148d-3）。
///
/// 目标：让 `const val` / `comptime if` / `const fun` 能在编译期直接执行
/// `Float.toInt()/toString()/hash()/abs()/isNaN()/isInfinite()`，避免掉到模糊的
/// “generic call callee / expression kind” 路径。
fn try_eval_float_method_intrinsic(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    call_span: crate::span::Span,
    receiver_expr: &ast::Expr,
    method_name: &str,
    args: &[ast::Expr],
) -> Result<ControlFlow<EvalControl, Option<ConstValue>>, ConstEvalError> {
    let is_known_float_method = matches!(
        method_name,
        "toInt" | "toString" | "hash" | "abs" | "isNaN" | "isInfinite"
    );
    if !is_known_float_method {
        return Ok(ControlFlow::Continue(None));
    }

    let recv = match eval_const_expr_flow_with_host(ctx, host, receiver_expr)? {
        ControlFlow::Continue(v) => v,
        ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
    };
    let ConstValue::Float(value) = recv else {
        return Ok(ControlFlow::Continue(None));
    };

    if !args.is_empty() {
        return Err(ConstEvalError::ConstFunArityMismatch {
            name: method_name.to_string(),
            expected: 0,
            found: args.len(),
            span: call_span.into(),
        });
    }

    let mk_int = |v: i64| ConstValue::Int(ConstInt::new(ctx.default_int_ty, v as u128));

    let result = match method_name {
        "toInt" => mk_int(const_float_to_i64(value)),
        "toString" => ConstValue::String(format_const_float_to_runtime_text(value)),
        "hash" => mk_int(const_float_hash_i64(value)),
        "abs" => match value.ty() {
            ConstFloatTy::Float64 => ConstValue::Float(ConstFloat::from_f64(value.as_f64().abs())),
            ConstFloatTy::Float32 => ConstValue::Float(ConstFloat::from_f32(value.as_f32().abs())),
        },
        "isNaN" => ConstValue::Bool(match value.ty() {
            ConstFloatTy::Float64 => value.as_f64().is_nan(),
            ConstFloatTy::Float32 => value.as_f32().is_nan(),
        }),
        "isInfinite" => ConstValue::Bool(match value.ty() {
            ConstFloatTy::Float64 => value.as_f64().is_infinite(),
            ConstFloatTy::Float32 => value.as_f32().is_infinite(),
        }),
        _ => unreachable!("filtered by is_known_float_method"),
    };

    Ok(ControlFlow::Continue(Some(result)))
}

/// Char 方法 intrinsics（T0146b）。
///
/// 当前只补齐 `toInt()` 的编译期求值；运行期/sysroot API 留给 T0146c。
fn try_eval_char_method_intrinsic(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    call_span: crate::span::Span,
    receiver_expr: &ast::Expr,
    method_name: &str,
    args: &[ast::Expr],
) -> Result<ControlFlow<EvalControl, Option<ConstValue>>, ConstEvalError> {
    if method_name != "toInt" {
        return Ok(ControlFlow::Continue(None));
    }

    let recv = match eval_const_expr_flow_with_host(ctx, host, receiver_expr)? {
        ControlFlow::Continue(v) => v,
        ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
    };
    let ConstValue::Char(ch) = recv else {
        return Ok(ControlFlow::Continue(None));
    };

    if !args.is_empty() {
        return Err(ConstEvalError::ConstFunArityMismatch {
            name: method_name.to_string(),
            expected: 0,
            found: args.len(),
            span: call_span.into(),
        });
    }

    Ok(ControlFlow::Continue(Some(ConstValue::Int(ConstInt::new(
        ctx.default_int_ty,
        ch as u32 as u128,
    )))))
}

fn format_const_float_to_runtime_text(value: ConstFloat) -> String {
    match value {
        ConstFloat::Float64(bits) => {
            normalize_const_float_text(f64::from_bits(bits).to_string(), f64::from_bits(bits))
        }
        ConstFloat::Float32(bits) => {
            let raw = f32::from_bits(bits);
            normalize_const_float_text(raw.to_string(), f64::from(raw))
        }
    }
}

fn normalize_const_float_text(mut text: String, value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn const_float_to_i64(value: ConstFloat) -> i64 {
    let value = match value.ty() {
        ConstFloatTy::Float64 => value.as_f64(),
        ConstFloatTy::Float32 => f64::from(value.as_f32()),
    };

    if value.is_nan() {
        return 0;
    }
    if value >= i64::MAX as f64 {
        return i64::MAX;
    }
    if value <= i64::MIN as f64 {
        return i64::MIN;
    }
    value as i64
}

fn const_float_hash_i64(value: ConstFloat) -> i64 {
    let mut bits = match value {
        ConstFloat::Float64(bits) => bits,
        ConstFloat::Float32(bits) => u64::from(bits),
    };

    bits ^= bits >> 30;
    bits = bits.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    bits ^= bits >> 27;
    bits = bits.wrapping_mul(0x94d0_49bb_1331_11eb);
    bits ^= bits >> 31;
    bits as i64
}

/// 字节级 indexOf（与 sysroot/string.scoop 语义一致）。
fn string_index_of(haystack: &str, needle: &str) -> i64 {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return 0;
    }
    if n.len() > h.len() {
        return -1;
    }
    let limit = h.len() - n.len();
    for i in 0..=limit {
        if &h[i..i + n.len()] == n {
            return i as i64;
        }
    }
    -1
}

/// 字节级 split（与 sysroot/string.scoop 语义一致）。
fn string_split(s: &str, delim: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    if delim.is_empty() {
        return vec![s.to_string()];
    }
    let h = s.as_bytes();
    let d = delim.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i + d.len() <= h.len() {
        if &h[i..i + d.len()] == d {
            parts.push(String::from_utf8_lossy(&h[start..i]).into_owned());
            start = i + d.len();
            i = start;
        } else {
            i += 1;
        }
    }
    parts.push(String::from_utf8_lossy(&h[start..]).into_owned());
    parts
}

/// 字节级 compareTo（与 runtime/c scoop_string_compare_to 一致）。
fn string_compare_to(a: &str, b: &str) -> i64 {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let min_len = ab.len().min(bb.len());
    for i in 0..min_len {
        let diff = (ab[i] as i64) - (bb[i] as i64);
        if diff != 0 {
            return diff;
        }
    }
    (ab.len() as i64) - (bb.len() as i64)
}

/// 按 ASCII 空白字符 trim（与 sysroot/string.scoop 一致：空格/Tab/CR/LF/VT/FF）。
fn string_trim_ascii_ws(s: &str) -> String {
    string_trim_end_ascii_ws(&string_trim_start_ascii_ws(s))
}

fn string_trim_start_ascii_ws(s: &str) -> String {
    let b = s.as_bytes();
    let mut start = 0;
    while start < b.len() && is_ascii_ws(b[start]) {
        start += 1;
    }
    String::from_utf8_lossy(&b[start..]).into_owned()
}

fn string_trim_end_ascii_ws(s: &str) -> String {
    let b = s.as_bytes();
    let mut end = b.len();
    while end > 0 && is_ascii_ws(b[end - 1]) {
        end -= 1;
    }
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// 与 sysroot/string.scoop 的 trimStart/trimEnd 一致：space(32) + [9..13]。
fn is_ascii_ws(b: u8) -> bool {
    b == 32 || (9..=13).contains(&b)
}

fn eval_member_access(
    ctx: ConstEvalCtx<'_>,
    receiver: ConstValue,
    member: &ast::MemberIdent,
    whole_expr_span: crate::span::Span,
) -> Result<ConstValue, ConstEvalError> {
    let member_name = member_simple_name(ctx.source, member);

    match receiver {
        ConstValue::Tuple(elements) => {
            let Some(index) = parse_tuple_member_index(member_name) else {
                return Err(ConstEvalError::UnknownMember {
                    name: member_name.to_string(),
                    span: member.span.into(),
                });
            };
            let Some(v) = elements.get(index) else {
                return Err(ConstEvalError::TupleIndexOutOfBounds {
                    index,
                    len: elements.len(),
                    span: member.span.into(),
                });
            };
            Ok(v.clone())
        }
        ConstValue::Struct(s) => {
            s.fields
                .get(member_name)
                .cloned()
                .ok_or_else(|| ConstEvalError::UnknownMember {
                    name: member_name.to_string(),
                    span: member.span.into(),
                })
        }
        ConstValue::Enum(e) => {
            // 早期阶段把 enum payload 当作“位置字段”，并沿用 tuple 的 `_0/_1/...` 访问语法。
            let Some(index) = parse_tuple_member_index(member_name) else {
                return Err(ConstEvalError::UnknownMember {
                    name: member_name.to_string(),
                    span: member.span.into(),
                });
            };
            let Some(v) = e.payload.get(index) else {
                return Err(ConstEvalError::TupleIndexOutOfBounds {
                    index,
                    len: e.payload.len(),
                    span: member.span.into(),
                });
            };
            Ok(v.clone())
        }
        other => Err(ConstEvalError::UnsupportedExpr {
            kind: match other {
                ConstValue::Unit
                | ConstValue::Bool(_)
                | ConstValue::Char(_)
                | ConstValue::Int(_)
                | ConstValue::Float(_)
                | ConstValue::String(_) => "member access（非 aggregate）",
                ConstValue::Tuple(_) | ConstValue::Struct(_) | ConstValue::Enum(_) => {
                    unreachable!()
                }
            },
            span: whole_expr_span.into(),
        }),
    }
}

fn eval_enum_ctor_call(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    call_span: crate::span::Span,
    callee: &ast::Expr,
    args: &[ast::Expr],
) -> Result<ConstExprFlow, ConstEvalError> {
    let (ty, variant) = match &callee.kind {
        // `Some(1)`：缺少 expected type 时无法静态消歧，先允许 ty 为空。
        ast::ExprKind::Ident(id) => {
            let variant = ctx.source.slice(id.span);
            if !looks_like_type_name(variant) {
                return Err(ConstEvalError::UnsupportedExpr {
                    kind: "call expression",
                    span: call_span.into(),
                });
            }
            (None, variant.to_string())
        }
        // `Option.Some(1)`：显式指定 enum 名称（或命名空间入口）。
        ast::ExprKind::MemberAccess { receiver, member } => {
            let ast::ExprKind::Ident(receiver_id) = &receiver.kind else {
                return Err(ConstEvalError::UnsupportedExpr {
                    kind: "call expression",
                    span: call_span.into(),
                });
            };
            let enum_name = ctx.source.slice(receiver_id.span);
            let variant = member_simple_name(ctx.source, member);
            if !looks_like_type_name(enum_name) || !looks_like_type_name(variant) {
                return Err(ConstEvalError::UnsupportedExpr {
                    kind: "call expression",
                    span: call_span.into(),
                });
            }
            (Some(enum_name.to_string()), variant.to_string())
        }
        _ => {
            return Err(ConstEvalError::UnsupportedExpr {
                kind: "call expression",
                span: call_span.into(),
            });
        }
    };

    let mut payload: Vec<ConstValue> = Vec::with_capacity(args.len());
    for a in args {
        let value = match eval_const_expr_flow_with_host(ctx, host, a)? {
            ControlFlow::Continue(v) => v,
            ControlFlow::Break(signal) => return Ok(ControlFlow::Break(signal)),
        };
        payload.push(value);
    }

    Ok(ControlFlow::Continue(ConstValue::Enum(ConstEnum {
        ty,
        variant,
        payload,
    })))
}

fn type_path_name(source: &SourceFile, ty: &ast::TypePath) -> String {
    ty.segments
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn looks_like_type_name(name: &str) -> bool {
    // Kotlin 风格：类型/enum variant 使用大写开头，函数/变量通常小写开头。
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

/// 把一个 `TypeRef` 格式化为稳定字符串（用于 class literal：`TypeName::class`）。
///
/// 说明：
/// - 这里输出的是“语法层面”的名字（基于 AST），并不保证是全限定名；
/// - 仅覆盖当前阶段注解参数/fixtures 需要的子集。
fn type_ref_name_for_class_literal(source: &SourceFile, ty: &ast::TypeRef) -> Option<String> {
    match ty {
        ast::TypeRef::Path(p) => {
            let mut out = p
                .segments
                .iter()
                .map(|id| id.text(source))
                .collect::<Vec<_>>()
                .join(".");
            if !p.args.is_empty() {
                let inner = p
                    .args
                    .iter()
                    .filter_map(|a| type_ref_name_for_class_literal(source, a))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push('<');
                out.push_str(&inner);
                out.push('>');
            }
            Some(out)
        }
        ast::TypeRef::Nullable { inner, .. } => {
            type_ref_name_for_class_literal(source, inner).map(|s| format!("{s}?"))
        }
        ast::TypeRef::Tuple(t) if t.elements.is_empty() => Some("Unit".to_string()),
        // v0：不支持把这些类型表达成“可稳定序列化的 class literal 名字”。
        ast::TypeRef::Tuple(_)
        | ast::TypeRef::Star { .. }
        | ast::TypeRef::EffectRowArg { .. }
        | ast::TypeRef::Function(_) => None,
    }
}

/// 尝试把 `A.B` / `pkg.Enum.Variant` 识别为 enum unit variant 常量。
///
/// 返回 `(ty_path, variant)`，其中 `ty_path` 为去掉最后一段后的 `.` 连接字符串。
fn try_parse_enum_unit_variant_path(
    source: &SourceFile,
    host: &mut impl ConstEvalHost,
    expr: &ast::Expr,
) -> Option<(String, String)> {
    let mut segs: Vec<&str> = Vec::new();
    collect_simple_member_access_path(source, expr, &mut segs)?;
    if segs.len() < 2 {
        return None;
    }

    // 若首段是已绑定的值（局部/参数/const val），优先按“普通 member access”处理。
    if host.resolve_ident(segs[0]).is_some() {
        return None;
    }

    let variant = *segs.last()?;
    let type_name = segs.get(segs.len().saturating_sub(2)).copied()?;
    if !looks_like_type_name(type_name) || !looks_like_type_name(variant) {
        return None;
    }

    let ty = segs[..segs.len() - 1].join(".");
    Some((ty, variant.to_string()))
}

/// 收集一个“纯路径形式”的 member access：`a.b.c` → `["a","b","c"]`。
///
/// 仅接受由 `Ident` 与 `MemberAccess` 组成的链；遇到其它表达式形态返回 None。
fn collect_simple_member_access_path<'a>(
    source: &'a SourceFile,
    expr: &'a ast::Expr,
    out: &mut Vec<&'a str>,
) -> Option<()> {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            out.push(source.slice(id.span));
            Some(())
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            collect_simple_member_access_path(source, receiver, out)?;
            out.push(source.slice(member.span));
            Some(())
        }
        _ => None,
    }
}

fn member_simple_name<'a>(source: &'a SourceFile, member: &ast::MemberIdent) -> &'a str {
    let raw = source.slice(member.span);
    raw.rsplit('.').next().unwrap_or(raw)
}

fn parse_tuple_member_index(text: &str) -> Option<usize> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
}

fn is_trim_indent_ws(b: u8) -> bool {
    // Kotlin 风格：缩进只考虑空格/Tab（raw string 的常见场景）。
    b == b' ' || b == b'\t'
}

fn is_trim_blank_ws(b: u8) -> bool {
    // “空白行”判断：把 CR 也视为可忽略空白，以兼容 CRLF 输入。
    is_trim_indent_ws(b) || b == b'\r'
}

fn is_blank_line(bytes: &[u8], start: usize, end: usize) -> bool {
    bytes[start..end].iter().copied().all(is_trim_blank_ws)
}

/// `trimIndent()`：去掉所有行的公共缩进，并剥离首尾空白行（spec §8.4）。
///
/// 该实现与 `runtime/c/scoop_runtime.c:scoop_string_trim_indent` 保持一致：
/// - 按 `\n` 分割行，并对每行剥离末尾 `\r`（兼容 CRLF）；
/// - 缩进仅识别 ASCII 空格/Tab；
/// - 空白行判定把 `\r` 也视为可忽略空白；
/// - 空白行在输出中会被规范化为真正的空行（不保留空格）。
fn string_trim_indent_kotlin_like(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }

    let bytes = s.as_bytes();

    // 1) 记录每一行的 [start, end)（end 不含 '\n'；若行尾是 '\r' 则剥离）。
    let mut starts: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();

    let mut cur_start = 0usize;
    for (i, b) in bytes.iter().copied().enumerate() {
        if b != b'\n' {
            continue;
        }

        let mut end = i;
        if end > cur_start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        starts.push(cur_start);
        ends.push(end);
        cur_start = i + 1;
    }

    // last line
    let mut end = bytes.len();
    if end > cur_start && bytes[end - 1] == b'\r' {
        end -= 1;
    }
    starts.push(cur_start);
    ends.push(end);

    // 2) 剥离首尾空白行。
    let mut first = 0usize;
    while first < starts.len() && is_blank_line(bytes, starts[first], ends[first]) {
        first += 1;
    }
    if first == starts.len() {
        return String::new();
    }

    let mut last = starts.len() - 1;
    while last > first && is_blank_line(bytes, starts[last], ends[last]) {
        last -= 1;
    }

    // 3) 计算最小公共缩进（仅在非空白行上统计）。
    let mut min_indent = usize::MAX;
    for li in first..=last {
        let s0 = starts[li];
        let e0 = ends[li];
        if is_blank_line(bytes, s0, e0) {
            continue;
        }

        let mut indent = 0usize;
        while s0 + indent < e0 && is_trim_indent_ws(bytes[s0 + indent]) {
            indent += 1;
        }
        min_indent = min_indent.min(indent);
    }

    if min_indent == usize::MAX {
        min_indent = 0;
    }

    // 4) 输出：对每行 drop `min_indent`，并把剩余空白行规范化为真正的空行。
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    for li in first..=last {
        let s0 = starts[li];
        let e0 = ends[li];

        let mut drop = 0usize;
        while drop < min_indent && s0 + drop < e0 && is_trim_indent_ws(bytes[s0 + drop]) {
            drop += 1;
        }
        let ts = s0 + drop;

        if !is_blank_line(bytes, ts, e0) {
            out.extend_from_slice(&bytes[ts..e0]);
        }

        if li != last {
            out.push(b'\n');
        }
    }

    // safety: 输入为合法 UTF-8；trimIndent 仅删除/插入 ASCII 字节，不会破坏 UTF-8。
    String::from_utf8(out).expect("trimIndent result should be valid UTF-8")
}

/// 求值一元运算（`!`/`-`/`~`）。
fn eval_unary(
    span: crate::span::Span,
    op: ast::UnaryOp,
    v: ConstValue,
) -> Result<ConstValue, ConstEvalError> {
    match op {
        ast::UnaryOp::Not => match v {
            ConstValue::Bool(b) => Ok(ConstValue::Bool(!b)),
            _ => Err(ConstEvalError::OperandTypeMismatch {
                expected: "Bool",
                found: value_kind(&v),
                span: span.into(),
            }),
        },
        ast::UnaryOp::Neg => match v {
            ConstValue::Int(i) => {
                // two's complement negation: (-x) == (!x + 1) mod 2^bits
                let mask = mask_for(i.ty.bits);
                let raw = (!i.raw_bits).wrapping_add(1) & mask;
                Ok(ConstValue::Int(ConstInt::new(i.ty, raw)))
            }
            ConstValue::Float(f) => match f.ty() {
                ConstFloatTy::Float64 => Ok(ConstValue::Float(ConstFloat::from_f64(-f.as_f64()))),
                ConstFloatTy::Float32 => Ok(ConstValue::Float(ConstFloat::from_f32(-f.as_f32()))),
            },
            _ => Err(ConstEvalError::OperandTypeMismatch {
                expected: "整数或 Float",
                found: value_kind(&v),
                span: span.into(),
            }),
        },
        ast::UnaryOp::BitNot => match v {
            ConstValue::Int(i) => {
                let mask = mask_for(i.ty.bits);
                let raw = (!i.raw_bits) & mask;
                Ok(ConstValue::Int(ConstInt::new(i.ty, raw)))
            }
            _ => Err(ConstEvalError::OperandTypeMismatch {
                expected: "整数",
                found: value_kind(&v),
                span: span.into(),
            }),
        },
    }
}

/// 求值二元运算（入口）：负责处理 short-circuit（`&&`/`||`）。
fn eval_binary(
    ctx: ConstEvalCtx<'_>,
    host: &mut impl ConstEvalHost,
    span: crate::span::Span,
    op: ast::BinaryOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
) -> Result<ConstExprFlow, ConstEvalError> {
    match op {
        ast::BinaryOp::LogAnd => {
            let l = eval_value!(ctx, host, lhs);
            let ConstValue::Bool(lb) = l else {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "Bool",
                    found: value_kind(&l),
                    span: span.into(),
                });
            };
            if !lb {
                return Ok(ControlFlow::Continue(ConstValue::Bool(false)));
            }
            let r = eval_value!(ctx, host, rhs);
            let ConstValue::Bool(rb) = r else {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "Bool",
                    found: value_kind(&r),
                    span: span.into(),
                });
            };
            Ok(ControlFlow::Continue(ConstValue::Bool(rb)))
        }
        ast::BinaryOp::LogOr => {
            let l = eval_value!(ctx, host, lhs);
            let ConstValue::Bool(lb) = l else {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "Bool",
                    found: value_kind(&l),
                    span: span.into(),
                });
            };
            if lb {
                return Ok(ControlFlow::Continue(ConstValue::Bool(true)));
            }
            let r = eval_value!(ctx, host, rhs);
            let ConstValue::Bool(rb) = r else {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "Bool",
                    found: value_kind(&r),
                    span: span.into(),
                });
            };
            Ok(ControlFlow::Continue(ConstValue::Bool(rb)))
        }
        _ => {
            let l = eval_value!(ctx, host, lhs);
            let r = eval_value!(ctx, host, rhs);
            Ok(ControlFlow::Continue(eval_binary_eager(
                ctx.source, span, op, lhs, rhs, l, r,
            )?))
        }
    }
}

/// 求值二元运算（eager）：假设两侧都需要被求值。
fn eval_binary_eager(
    source: &SourceFile,
    span: crate::span::Span,
    op: ast::BinaryOp,
    lhs_expr: &ast::Expr,
    rhs_expr: &ast::Expr,
    lhs: ConstValue,
    rhs: ConstValue,
) -> Result<ConstValue, ConstEvalError> {
    match op {
        // String `+`（concatenation）：两个编译期 String 常量拼接。
        ast::BinaryOp::Add
            if matches!((&lhs, &rhs), (ConstValue::String(_), ConstValue::String(_))) =>
        {
            let (ConstValue::String(a), ConstValue::String(b)) = (lhs, rhs) else {
                unreachable!()
            };
            Ok(ConstValue::String(a + &b))
        }

        // arithmetic / bitwise
        ast::BinaryOp::Add
        | ast::BinaryOp::Sub
        | ast::BinaryOp::Mul
        | ast::BinaryOp::Div
        | ast::BinaryOp::Rem
        | ast::BinaryOp::BitAnd
        | ast::BinaryOp::BitXor
        | ast::BinaryOp::BitOr
        | ast::BinaryOp::Shl
        | ast::BinaryOp::Shr
        | ast::BinaryOp::Lt
        | ast::BinaryOp::Le
        | ast::BinaryOp::Gt
        | ast::BinaryOp::Ge => {
            if matches!(
                (&lhs, &rhs),
                (ConstValue::Float(_), _) | (_, ConstValue::Float(_))
            ) {
                let (lf, rf) = match (&lhs, &rhs) {
                    (ConstValue::Float(lf), ConstValue::Float(rf)) => (*lf, *rf),
                    _ => {
                        return Err(ConstEvalError::OperandTypeMismatch {
                            expected: "相同的 Float 类型",
                            found: binary_found_kind(&lhs, &rhs),
                            span: span.into(),
                        });
                    }
                };
                let Some((lf, rf, _target_ty)) =
                    coerce_float_operands_for_binary(source, lhs_expr, lf, rhs_expr, rf)
                else {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "相同的 Float 类型（或 Float32 + 无后缀 Float 字面量）",
                        found: binary_found_kind(&lhs, &rhs),
                        span: span.into(),
                    });
                };
                return eval_float_binary(span, op, lf, rf);
            }

            if matches!(
                op,
                ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge
            ) && let (ConstValue::Char(a), ConstValue::Char(b)) = (&lhs, &rhs)
            {
                let ok = match op {
                    ast::BinaryOp::Lt => a < b,
                    ast::BinaryOp::Le => a <= b,
                    ast::BinaryOp::Gt => a > b,
                    ast::BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                };
                return Ok(ConstValue::Bool(ok));
            }
            let (li, ri) = match (lhs, rhs) {
                (ConstValue::Int(li), ConstValue::Int(ri)) => (li, ri),
                (l, r) => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "整数或 Char",
                        found: binary_found_kind(&l, &r),
                        span: span.into(),
                    });
                }
            };
            if li.ty != ri.ty {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "相同的整数类型",
                    found: "不同位宽/符号位的整数",
                    span: span.into(),
                });
            }
            eval_int_binary(span, op, li, ri)
        }

        ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
            if matches!(
                (&lhs, &rhs),
                (ConstValue::Float(_), _) | (_, ConstValue::Float(_))
            ) {
                let (lf, rf) = match (&lhs, &rhs) {
                    (ConstValue::Float(lf), ConstValue::Float(rf)) => (*lf, *rf),
                    _ => {
                        return Err(ConstEvalError::OperandTypeMismatch {
                            expected: "相同的 Float 类型",
                            found: binary_found_kind(&lhs, &rhs),
                            span: span.into(),
                        });
                    }
                };
                let Some((lf, rf, _target_ty)) =
                    coerce_float_operands_for_binary(source, lhs_expr, lf, rhs_expr, rf)
                else {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "相同的 Float 类型（或 Float32 + 无后缀 Float 字面量）",
                        found: binary_found_kind(&lhs, &rhs),
                        span: span.into(),
                    });
                };
                return eval_float_equality(op, lf, rf);
            }

            match (lhs, rhs) {
                (ConstValue::Bool(a), ConstValue::Bool(b)) => match op {
                    ast::BinaryOp::Eq => Ok(ConstValue::Bool(a == b)),
                    ast::BinaryOp::Ne => Ok(ConstValue::Bool(a != b)),
                    _ => unreachable!(),
                },
                (ConstValue::Char(a), ConstValue::Char(b)) => match op {
                    ast::BinaryOp::Eq => Ok(ConstValue::Bool(a == b)),
                    ast::BinaryOp::Ne => Ok(ConstValue::Bool(a != b)),
                    _ => unreachable!(),
                },
                (ConstValue::Int(a), ConstValue::Int(b)) => {
                    if a.ty != b.ty {
                        return Err(ConstEvalError::OperandTypeMismatch {
                            expected: "相同的整数类型",
                            found: "不同位宽/符号位的整数",
                            span: span.into(),
                        });
                    }
                    match op {
                        ast::BinaryOp::Eq => Ok(ConstValue::Bool(a.raw_bits == b.raw_bits)),
                        ast::BinaryOp::Ne => Ok(ConstValue::Bool(a.raw_bits != b.raw_bits)),
                        _ => unreachable!(),
                    }
                }
                (ConstValue::String(a), ConstValue::String(b)) => match op {
                    ast::BinaryOp::Eq => Ok(ConstValue::Bool(a == b)),
                    ast::BinaryOp::Ne => Ok(ConstValue::Bool(a != b)),
                    _ => unreachable!(),
                },
                (l, r) => Err(ConstEvalError::OperandTypeMismatch {
                    expected: "相同的 Bool/Char/整数/Float/String",
                    found: binary_found_kind(&l, &r),
                    span: span.into(),
                }),
            }
        }

        ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => unreachable!("short-circuit 已在上层处理"),
        ast::BinaryOp::RangeInclusive => Err(ConstEvalError::UnsupportedExpr {
            kind: "range (..)",
            span: span.into(),
        }),
        ast::BinaryOp::Elvis => Err(ConstEvalError::UnsupportedExpr {
            kind: "elvis (?:)",
            span: span.into(),
        }),
    }
}

/// 求值“两个整数”的二元运算（算术/位运算/比较等）。
fn eval_int_binary(
    span: crate::span::Span,
    op: ast::BinaryOp,
    lhs: ConstInt,
    rhs: ConstInt,
) -> Result<ConstValue, ConstEvalError> {
    let ty = lhs.ty;
    let mask = mask_for(ty.bits);

    let mk_int = |raw: u128| ConstValue::Int(ConstInt::new(ty, raw));

    let out = match op {
        ast::BinaryOp::Add => mk_int(lhs.raw_bits.wrapping_add(rhs.raw_bits) & mask),
        ast::BinaryOp::Sub => mk_int(lhs.raw_bits.wrapping_sub(rhs.raw_bits) & mask),
        ast::BinaryOp::Mul => mk_int(lhs.raw_bits.wrapping_mul(rhs.raw_bits) & mask),
        ast::BinaryOp::BitAnd => mk_int((lhs.raw_bits & rhs.raw_bits) & mask),
        ast::BinaryOp::BitXor => mk_int((lhs.raw_bits ^ rhs.raw_bits) & mask),
        ast::BinaryOp::BitOr => mk_int((lhs.raw_bits | rhs.raw_bits) & mask),
        ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
            let shift = shift_amount(span, rhs)?;
            let bits = ty.bits;
            if bits == 0 {
                return Ok(mk_int(0));
            }
            let shift = (shift % u128::from(bits)) as u32;
            let raw = match op {
                ast::BinaryOp::Shl => (lhs.raw_bits << shift) & mask,
                ast::BinaryOp::Shr => {
                    if ty.signed {
                        let v = lhs.as_i128();
                        mask_to_bits((v >> shift) as u128, bits)
                    } else {
                        (lhs.raw_bits >> shift) & mask
                    }
                }
                _ => unreachable!(),
            };
            mk_int(raw)
        }
        ast::BinaryOp::Div | ast::BinaryOp::Rem => {
            if rhs.raw_bits == 0 {
                return Err(ConstEvalError::IntDivByZero { span: span.into() });
            }
            if ty.signed {
                let a = lhs.as_i128();
                let b = rhs.as_i128();
                if op == ast::BinaryOp::Div {
                    let q = a
                        .checked_div(b)
                        .ok_or(ConstEvalError::IntDivByZero { span: span.into() })?;
                    mk_int(q as u128)
                } else {
                    let r = a
                        .checked_rem(b)
                        .ok_or(ConstEvalError::IntDivByZero { span: span.into() })?;
                    mk_int(r as u128)
                }
            } else if op == ast::BinaryOp::Div {
                mk_int((lhs.raw_bits / rhs.raw_bits) & mask)
            } else {
                mk_int((lhs.raw_bits % rhs.raw_bits) & mask)
            }
        }
        ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
            let ok = if ty.signed {
                let a = lhs.as_i128();
                let b = rhs.as_i128();
                match op {
                    ast::BinaryOp::Lt => a < b,
                    ast::BinaryOp::Le => a <= b,
                    ast::BinaryOp::Gt => a > b,
                    ast::BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                }
            } else {
                let a = lhs.as_u128();
                let b = rhs.as_u128();
                match op {
                    ast::BinaryOp::Lt => a < b,
                    ast::BinaryOp::Le => a <= b,
                    ast::BinaryOp::Gt => a > b,
                    ast::BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                }
            };
            ConstValue::Bool(ok)
        }
        _ => {
            return Err(ConstEvalError::UnsupportedExpr {
                kind: "binary op",
                span: span.into(),
            });
        }
    };

    Ok(out)
}

fn is_unsuffixed_float_literal(source: &SourceFile, expr: &ast::Expr) -> bool {
    matches!(expr.kind, ast::ExprKind::FloatLit)
        && matches!(
            parse_float_literal(source.slice(expr.span)).suffix,
            FloatLiteralSuffix::Float64
        )
}

fn coerce_float_operands_for_binary(
    source: &SourceFile,
    lhs_expr: &ast::Expr,
    lhs: ConstFloat,
    rhs_expr: &ast::Expr,
    rhs: ConstFloat,
) -> Option<(ConstFloat, ConstFloat, ConstFloatTy)> {
    let target_ty = match (lhs.ty(), rhs.ty()) {
        (lhs_ty, rhs_ty) if lhs_ty == rhs_ty => lhs_ty,
        (ConstFloatTy::Float64, ConstFloatTy::Float32)
            if is_unsuffixed_float_literal(source, lhs_expr) =>
        {
            ConstFloatTy::Float32
        }
        (ConstFloatTy::Float32, ConstFloatTy::Float64)
            if is_unsuffixed_float_literal(source, rhs_expr) =>
        {
            ConstFloatTy::Float32
        }
        _ => return None,
    };

    Some((lhs.cast(target_ty), rhs.cast(target_ty), target_ty))
}

fn eval_float_binary(
    span: crate::span::Span,
    op: ast::BinaryOp,
    lhs: ConstFloat,
    rhs: ConstFloat,
) -> Result<ConstValue, ConstEvalError> {
    debug_assert_eq!(lhs.ty(), rhs.ty());

    match lhs.ty() {
        ConstFloatTy::Float64 => {
            let a = lhs.as_f64();
            let b = rhs.as_f64();
            match op {
                ast::BinaryOp::Add => Ok(ConstValue::Float(ConstFloat::from_f64(a + b))),
                ast::BinaryOp::Sub => Ok(ConstValue::Float(ConstFloat::from_f64(a - b))),
                ast::BinaryOp::Mul => Ok(ConstValue::Float(ConstFloat::from_f64(a * b))),
                ast::BinaryOp::Div => Ok(ConstValue::Float(ConstFloat::from_f64(a / b))),
                ast::BinaryOp::Rem => Ok(ConstValue::Float(ConstFloat::from_f64(a % b))),
                ast::BinaryOp::Lt => Ok(ConstValue::Bool(a < b)),
                ast::BinaryOp::Le => Ok(ConstValue::Bool(a <= b)),
                ast::BinaryOp::Gt => Ok(ConstValue::Bool(a > b)),
                ast::BinaryOp::Ge => Ok(ConstValue::Bool(a >= b)),
                _ => Err(ConstEvalError::UnsupportedExpr {
                    kind: "float binary op",
                    span: span.into(),
                }),
            }
        }
        ConstFloatTy::Float32 => {
            let a = lhs.as_f32();
            let b = rhs.as_f32();
            match op {
                ast::BinaryOp::Add => Ok(ConstValue::Float(ConstFloat::from_f32(a + b))),
                ast::BinaryOp::Sub => Ok(ConstValue::Float(ConstFloat::from_f32(a - b))),
                ast::BinaryOp::Mul => Ok(ConstValue::Float(ConstFloat::from_f32(a * b))),
                ast::BinaryOp::Div => Ok(ConstValue::Float(ConstFloat::from_f32(a / b))),
                ast::BinaryOp::Rem => Ok(ConstValue::Float(ConstFloat::from_f32(a % b))),
                ast::BinaryOp::Lt => Ok(ConstValue::Bool(a < b)),
                ast::BinaryOp::Le => Ok(ConstValue::Bool(a <= b)),
                ast::BinaryOp::Gt => Ok(ConstValue::Bool(a > b)),
                ast::BinaryOp::Ge => Ok(ConstValue::Bool(a >= b)),
                _ => Err(ConstEvalError::UnsupportedExpr {
                    kind: "float binary op",
                    span: span.into(),
                }),
            }
        }
    }
}

fn eval_float_equality(
    op: ast::BinaryOp,
    lhs: ConstFloat,
    rhs: ConstFloat,
) -> Result<ConstValue, ConstEvalError> {
    debug_assert_eq!(lhs.ty(), rhs.ty());

    let result = match lhs.ty() {
        ConstFloatTy::Float64 => {
            let a = lhs.as_f64();
            let b = rhs.as_f64();
            match op {
                ast::BinaryOp::Eq => a == b,
                ast::BinaryOp::Ne => a != b,
                _ => unreachable!(),
            }
        }
        ConstFloatTy::Float32 => {
            let a = lhs.as_f32();
            let b = rhs.as_f32();
            match op {
                ast::BinaryOp::Eq => a == b,
                ast::BinaryOp::Ne => a != b,
                _ => unreachable!(),
            }
        }
    };

    Ok(ConstValue::Bool(result))
}

/// 计算 shift amount（拒绝负数；并返回非负的 u128 值）。
fn shift_amount(span: crate::span::Span, rhs: ConstInt) -> Result<u128, ConstEvalError> {
    let value = rhs.as_i128();
    if value < 0 {
        return Err(ConstEvalError::InvalidShiftCount {
            value,
            span: span.into(),
        });
    }
    Ok(value as u128)
}

/// 返回给定位宽对应的 mask（低 bits 位为 1）。
fn mask_for(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return u128::MAX;
    }
    (1u128 << bits) - 1
}

/// 将 const value 映射为“用户可读的类型类别”字符串（用于诊断）。
pub(crate) fn value_kind(v: &ConstValue) -> &'static str {
    match v {
        ConstValue::Unit => "Unit",
        ConstValue::Bool(_) => "Bool",
        ConstValue::Char(_) => "Char",
        ConstValue::Int(_) => "整数",
        ConstValue::Float(f) => f.ty().name(),
        ConstValue::String(_) => "String",
        ConstValue::Tuple(_) => "Tuple",
        ConstValue::Struct(_) => "Struct",
        ConstValue::Enum(_) => "Enum",
    }
}

/// 将二元操作数映射为“组合类别”字符串（用于诊断）。
fn binary_found_kind(lhs: &ConstValue, rhs: &ConstValue) -> &'static str {
    match (lhs, rhs) {
        (ConstValue::Bool(_), ConstValue::Bool(_)) => "Bool",
        (ConstValue::Char(_), ConstValue::Char(_)) => "Char",
        (ConstValue::Int(_), ConstValue::Int(_)) => "整数",
        (ConstValue::Float(a), ConstValue::Float(b)) if a.ty() == b.ty() => a.ty().name(),
        (ConstValue::Float(_), ConstValue::Float(_)) => "不同精度的 Float",
        (ConstValue::String(_), ConstValue::String(_)) => "String",
        _ => "不匹配的类型组合",
    }
}
