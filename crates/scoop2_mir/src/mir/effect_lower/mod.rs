//! Effect lowering pass：把 Perform/Handle/Resume 消除为状态机 + Step tagged union。
//!
//! 在 materialize + devirtualize + inline 之后运行。遍历每个函数体，把 direct-style
//! 的 effect 结构（Perform/Handle/Resume 终结符）转换为：
//!
//! - **EffectStep 函数**：含有未本地捕获的 Perform 的函数。ABI 变为
//!   `step(frame, resume_payload?) -> Step`。函数体变为状态机：用 frame 中的
//!   `state` 字段选择重入点，Perform 变为保存 live locals + 返回 Step case。
//! - **Plain 函数**：无未捕获 Perform 的函数。保持普通 ABI。
//!
//! Handle 的 body/arm/finally 变为状态机中的状态组。Perform 在 Handle body 内
//! 且匹配某 arm 时，跳到 arm 而非返回 Step（本地捕获）。不匹配的 Perform
//! 返回 Step 向上传播。
//!
//! Resume（`k.resume(v)`）变为：检查 resumed 标志 → panic（若已 resumed）→
//! 重入 step 函数（通过 continuation 的 resume_fn 函数指针）。
//!
//! 完成后，IR 中不再有 Perform/Handle/Resume 终结符。effect_row 保留在
//! FunDecl 中作诊断元数据。

use crate::mir::{Body, Module, Rvalue, StatementKind, TerminatorKind};

pub mod analyze;
pub mod lower;

pub use lower::lower_effects;
