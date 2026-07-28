//! MIR 阶段诊断：`scoop::mir::*` 错误码与错误类型。
//!
//! 实现 [`scoop2_base::diag::Diagnostic`]（实际是产出 `Diagnostic` 数据，而非实现
//! trait——本仓库的诊断是纯数据）。错误码形如 `scoop::mir::<name>`，供 fixture
//! 的 `EXPECT-ERROR-CODE: scoop::mir::*` 匹配。

use scoop2_base::diag::{Diagnostic, DiagnosticSink};
use scoop2_base::Span;

use crate::mir::{BasicBlockId, LocalId};

// ---------------------------------------------------------------------------
// 错误码常量
// ---------------------------------------------------------------------------
//
// 设计原则：**不使用笼统的「unsupported」码**。每个码必须反映具体语义：
// - 非法程序 → 具体拒绝码（如 break_outside_loop、splice_field_removed）；
// - 合法但 lowering 未覆盖 → 不允许存在（必须实现 lowering，不得报错）；
// - prelude / 编译环境缺失 → internal_error（环境错误，非用户程序错误）。

/// `break` 出现在循环体外（spec：break/continue 仅限循环内）。非法程序拒绝。
pub const BREAK_OUTSIDE_LOOP: &str = "scoop::mir::break_outside_loop";
/// `continue` 出现在循环体外。非法程序拒绝。
pub const CONTINUE_OUTSIDE_LOOP: &str = "scoop::mir::continue_outside_loop";
/// splice field `p.["x"]` / `p.[FieldMeta{...}]`：comptime 反射特性，已从语言移除，
/// MIR 阶段明确拒绝（引导用户改用具体字段访问 `p.x`）。
pub const SPLICE_FIELD_REMOVED: &str = "scoop::mir::splice_field_removed";
/// lowering 时引用了未解析的值（typecheck/resolve 失败的延续；正常不应到达，
/// 到达表示 HIR 不完整——completeness 闸门应已拦截）。
pub const LOWER_UNRESOLVED: &str = "scoop::mir::lower_unresolved";
/// prelude / 编译环境必需符号缺失（如 `set` / `iterator` / `hasNext` / `next` 未注册）。
/// 这是编译环境错误，非用户程序错误。
pub const PRELUDE_SYMBOL_MISSING: &str = "scoop::mir::prelude_symbol_missing";
/// 单态化错误（无法解析类型实参 / 缺泛型模板）。
pub const MONOMORPH_ERROR: &str = "scoop::mir::monomorph_error";
/// 单态化：实例化请求找不到对应泛型模板。
pub const MONOMORPH_NO_TEMPLATE: &str = "scoop::mir::monomorph_no_template";
/// 验证：CFG 结构错误（悬空块 / cleanup 目标非法 / 不可达 etc.）。
pub const VERIFY_CFG: &str = "scoop::mir::verify_cfg";
/// 验证：direct-style 语义错误（resume_target 缺失 etc.）。
pub const VERIFY_DIRECT_STYLE: &str = "scoop::mir::verify_direct_style";
/// 验证：production 语义完整性（callee 不可解析 / member resolved 为空 etc.）。
pub const VERIFY_SEMANTIC: &str = "scoop::mir::verify_semantic";

// ---------------------------------------------------------------------------
// 错误类型（lowering / monomorph 用 Result 携带）
// ---------------------------------------------------------------------------

/// MIR lowering 错误（携带诊断码 + span + 消息，可直接 push 进 DiagnosticSink）。
#[derive(Clone, Debug)]
pub struct MirLowerError {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

impl MirLowerError {
    /// 用指定错误码构造（具体语义码，非笼统 unsupported）。
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            message: message.into(),
        }
    }

    pub fn unresolved(span: Span, name: impl Into<String>) -> Self {
        Self {
            code: LOWER_UNRESOLVED,
            span,
            message: format!("MIR lowering 遇到未解析的引用：{}", name.into()),
        }
    }

    /// 转为 base Diagnostic（用于 push 进 sink）。
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code, self.message.clone())
            .with_primary(self.span, "MIR lowering 在此失败")
    }
}

impl std::fmt::Display for MirLowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", "error", self.code, self.message)
    }
}

impl std::error::Error for MirLowerError {}

/// 单态化错误。
#[derive(Clone, Debug)]
pub struct MonomorphError {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

impl MonomorphError {
    pub fn no_template(span: Span, fqn: impl Into<String>) -> Self {
        Self {
            code: MONOMORPH_NO_TEMPLATE,
            span,
            message: format!(
                "单态化失败：找不到泛型模板 `{}`",
                fqn.into()
            ),
        }
    }

    pub fn error(span: Span, msg: impl Into<String>) -> Self {
        Self {
            code: MONOMORPH_ERROR,
            span,
            message: msg.into(),
        }
    }

    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code, self.message.clone())
            .with_primary(self.span, "单态化在此失败")
    }
}

impl std::fmt::Display for MonomorphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", "error", self.code, self.message)
    }
}

impl std::error::Error for MonomorphError {}

/// CFG 验证错误（携带块 / local 上下文）。
#[derive(Clone, Debug)]
pub struct VerifyError {
    pub code: &'static str,
    pub block: Option<BasicBlockId>,
    pub local: Option<LocalId>,
    pub message: String,
}

impl VerifyError {
    pub fn cfg(block: Option<BasicBlockId>, msg: impl Into<String>) -> Self {
        Self {
            code: VERIFY_CFG,
            block,
            local: None,
            message: msg.into(),
        }
    }

    pub fn direct_style(block: Option<BasicBlockId>, msg: impl Into<String>) -> Self {
        Self {
            code: VERIFY_DIRECT_STYLE,
            block,
            local: None,
            message: msg.into(),
        }
    }

    pub fn semantic(msg: impl Into<String>) -> Self {
        Self {
            code: VERIFY_SEMANTIC,
            block: None,
            local: None,
            message: msg.into(),
        }
    }
}

/// 把一组 MIR 错误的诊断 push 进 sink。
pub fn report_errors(sink: &mut DiagnosticSink, errors: &[MirLowerError]) {
    for e in errors {
        sink.push(e.to_diagnostic());
    }
}
