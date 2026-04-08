//! 表达式类型检查。
//!
//! 说明：历史上该模块曾是单个 10K+ 行的 `expr.rs`，定位与回归成本较高。
//! 当前按职责拆分为子模块：
//! - `entry`：文件级入口与遍历（收集 side tables、进入 fun/class body）
//! - `infer`：表达式类型推导（非调用/成员/运算符的主体逻辑）
//! - `call`：调用/构造/重载筛选与泛型实参推断
//! - `ops`：一元/二元运算符与 operator overloading
//! - `member`：成员访问/安全访问/Elvis/非空断言等
//! - `stmt`：语句层递归（block/if/when/return 等）
//! - `collect`：单文件 side table 收集（顶层签名、字段类型/可变性等）
//! - `util`：跨子模块复用的小工具
//! - `error`：`ExprTypeError`

mod call;
mod collect;
mod entry;
mod error;
mod infer;
mod member;
mod ops;
mod stmt;
mod util;

pub use entry::{
    check_file_exprs, check_file_exprs_with_monomorph_and_type_instantiation_keys,
    check_file_exprs_with_monomorph_keys, check_file_exprs_with_type_instantiation_keys,
};
pub use error::ExprTypeError;

use crate::ast;
use crate::span::Span;
use crate::ty::{EffectRow, TypeId};

use super::eff_row_subst::EffRowVarSubstPlan;

pub(super) use call::lower_type_ref_with_enum_subst;

pub(super) const ASYNC_EFFECT_FQN: &str = "scoop.core.Async";
pub(super) const TASK_FQN: &str = "scoop.core.Task";
pub(super) const PTR_FQN: &str = "scoop.unsafe.Ptr";
pub(super) const FUNPTR_FQN: &str = "scoop.unsafe.FunPtr";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgramBoundaryKind {
    None,
    /// 可执行入口：`fun main`（runtime entry point，spec §5.10）。
    Main,
    /// 库导出入口 / host entry point（T0629b：由 Cone.toml 或 driver 配置指定）。
    Export,
}

#[derive(Debug, Clone)]
struct EffParamSig {
    name: String,
    default: EffectRow,
}

#[derive(Debug, Clone)]
struct FunSigOwned {
    /// 声明处 name 的 span：用于把“某个具体 overload”与 AST 节点对应起来，
    /// 以便在后续 pass 中回写（例如返回类型推断，T0507）。
    decl_span: Span,
    /// 该签名所属的声明文件（用于在正确的 source/package/import 上下文中 lower row/type）。
    decl_file: std::path::PathBuf,
    /// 是否为扩展函数（`fun Receiver.name(...)`）。
    ///
    /// 说明：
    /// - 在 typecheck 阶段我们把扩展函数“降糖”为普通顶层函数：receiver 作为第一个参数（spec §7.4）；
    /// - 该标记仅用于限制语法层的可调用性：扩展函数不能以 `f(args...)` 形式直接调用，
    ///   只能通过 `receiver.f(args...)` / `receiver?.f(args...)` 调用（当前阶段最小子集）。
    is_extension: bool,
    /// 是否为 `inline` 函数（spec §7.2/§7.3；TODO T0444）。
    ///
    /// 说明：当前阶段不做任何 inlining 优化，该标记仅用于：
    /// - lambda non-local return 的静态门禁（只有 inline lambda 实参允许 `return`）
    is_inline: bool,
    /// 是否为 `const fun`（spec §6.2）。
    ///
    /// 用途：
    /// - TODO T1211：在 `const fun` 语境中禁止调用非 const（但允许 const/intrinsic）；
    /// - 该标记会在“当前文件内”从 AST modifiers 收集；在“跨文件调用”路径中从 `Index` 的 `FunSig.is_const`
    ///   读取（resolver 已把该信息写入索引）。
    is_const: bool,
    /// 是否为 `@Unsafe` 函数（spec §15.9.1）。
    ///
    /// 说明：当前阶段（T1003）仅用于调用门禁：非 unsafe context 禁止调用 `@Unsafe`。
    is_unsafe: bool,
    /// 是否为 `@NoGC` 函数（spec §15.8）。
    ///
    /// 说明：当前阶段不实现 “可能分配” 分析；但 `@Extern` 会隐含 `@NoGC`（在收集阶段折叠）。
    #[allow(dead_code)]
    is_nogc: bool,
    /// 是否为 `@Extern` 函数（spec §15.8.3）。
    ///
    /// 说明：当前阶段（T1003）仅用于调用门禁：非 unsafe context 禁止调用 `@Extern`。
    is_extern: bool,
    /// 是否为 `@Intrinsic` 函数（spec §15.7）。
    ///
    /// 说明：当前阶段仅记录该标记，供后续 lowering/codegen 使用。
    #[allow(dead_code)]
    is_intrinsic: bool,
    /// 形参名列表（与 `params` 对齐）。
    ///
    /// 用途：
    /// - T0453：命名实参（`name = expr`）的重排与匹配；
    /// - 未来：默认参数/重载决议可复用该信息。
    ///
    /// 说明：
    /// - 对于扩展函数，`params[0]` 是 receiver 的类型占位；该位置的 `param_names[0]`
    ///   仅用于对齐，当前不会参与命名实参匹配（因为 receiver 不可被命名传入）。
    param_names: Vec<String>,
    /// 形参是否带默认值（与 `params` 对齐）。
    ///
    /// 用途：
    /// - T0454：构造调用重载决议已经支持默认参数；
    /// - T0512：把默认参数纳入函数调用的 overload resolution（先只做“候选可用性/映射”，
    ///   默认值表达式的补齐语义留给后续任务 T1305）。
    ///
    /// 说明：
    /// - 对于扩展函数，`params[0]` 是 receiver，占位为 `false`；
    /// - 当前阶段这里只需要“是否存在默认值”，不复制默认值表达式本体。
    param_has_defaults: Vec<bool>,
    /// 形参是否为 `vararg`（与 `params` 对齐）。
    ///
    /// 说明：
    /// - 当前阶段仅支持“至多一个 vararg，且必须为最后一个形参”（Kotlin-like；见 TODO T1308）；
    /// - 对于扩展函数，receiver 占位参数恒为 `false`。
    param_is_vararg: Vec<bool>,
    /// 函数级 type params（按声明顺序）。
    ///
    /// 用途（T0505）：
    /// - 让调用点可以识别“哪些 TypeId 是该函数的类型参数”
    /// - 在参数检查前做最小泛型实参推断，并对签名做 substitution（实例化）
    type_params: Vec<TypeId>,
    /// effect row 参数（`<eff E = Pure>`）（spec §3.4 / §14.7.3）。
    ///
    /// 说明：
    /// - 当前阶段仅支持单一 `eff` 参数（parser 已强制最多一个）；
    /// - 若调用点无法从 lambda 实参推断该 row，则回退到 `default`。
    eff_param: Option<EffParamSig>,
    /// 形参类型若为函数类型，且其 effects row 引用函数级 `eff` 变量，则记录其“base row”：
    /// 把 `E` 从 row 表达式中移除后剩余的常量项（已按声明处上下文 lowering）。
    ///
    /// 例：
    /// - `(...)->T / E`            => `Some(Pure)`（base 为空）
    /// - `(...)->T / (E + IO)`     => `Some(IO)`
    /// - `(...)->T / (IO + State)` => `None`（不引用 `E`）
    ///
    /// 对齐约定：该数组与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_fn_effect_eff_base: Vec<Option<EffectRow>>,
    /// 形参类型若为 `Type<eff Row>` 这类“use-site effect row 实参引用 `eff` 变量”的名义类型，
    /// 同样记录其 base row（把 `E` 移除后剩余的常量项）。
    ///
    /// 用途（T0624）：
    /// - 推断 `E` 时，除了从 lambda body 的 required effects 外，也需要从类型实参里提取约束：
    ///   `Disposable<eff Async>` 作为实参会让 `E` 至少包含 `Async`。
    /// - 在推断出 `E` 之后，还需要把签名里以默认值 lowering 的 `Type<eff E>` 回填为
    ///   `Type<eff E_arg>`，否则 call arg 的 assignable 检查会错误地用默认值对比。
    ///
    /// 对齐约定：该数组与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_nominal_eff_eff_base: Vec<Option<EffectRow>>,
    /// `E + ...` 的嵌套替换 plan：用于把签名类型中（包括 tuple/Option/多层 function type 等）的
    /// `E + base` 统一实例化为调用点的 `E_arg + base`（T0628b）。
    ///
    /// 对齐约定：与 `params` 对齐（扩展函数包含 receiver 的占位参数）。
    param_eff_row_var_subst: Vec<EffRowVarSubstPlan>,
    /// 返回类型中的 `E + ...` 嵌套替换 plan（T0628b）。
    return_eff_row_var_subst: EffRowVarSubstPlan,
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// 函数声明处的 effect row 标注：`/ Pure` / `/ E` / `/ (E1 + E2)`（spec §5.8）。
    effects: Option<ast::EffectRowExpr>,
}
