//! Scoop 下一代 MIR：从 typed HIR（`scoop2_hir`）生成 backend-agnostic 的
//! early-MIR（显式 CFG + ANF operand + direct-style effect 终结符），再做单态化。
//!
//! 设计要点（参考主线 `scoopc_mir`，但完全独立、不共享类型）：
//!
//! - **消费 typed HIR**：输入是 `ast::File` + `scoop2_hir::TypedHir`（含 `expr_types`
//!   与语义事实侧表 `call_resolutions` / `member_refs` / `assign_places` /
//!   `effect_sites` / `value_refs`）。lowering 遍历 AST 节点，类型靠
//!   `hir.expr_type(file_id, node.id)` 查。
//! - **direct-style effect**：`Perform` / `Handle` 是终结符；`Perform` 携带
//!   `resume_target`，使 CFG 保持直接风格（不做 CPS 变换）。
//! - **类型用 `scoop2_hir::ty::TypeId`**：MIR 不自带类型系统，复用 HIR 的
//!   `TypeStore` 句柄。
//! - **无优化 pass**：本 crate 只做 lowering + 单态化 + 验证；优化交给后端。
//! - **零 placeholder**：禁止 `todo!` / `unimplemented!` / `Todo` 变体；无法 lower
//!   的情形在 lowering 时报 `scoop::mir::*` 诊断，不进 IR。
//!
//! 模块布局：
//! - [`mir`]：IR 数据结构与 dump；
//! - [`mir::lower`]：HIR → MIR lowering；
//! - [`mir::materialize`]：generic → monomorphic 单态化；
//! - [`mir::verify`]：CFG 结构 / direct-style / 语义完整性验证；
//! - [`diagnostics`]：`scoop::mir::*` 诊断码与错误类型。

#![forbid(unsafe_code)]

pub use scoop2_base as base;
pub use scoop2_hir as hir;

pub mod diagnostics;
pub use scoop2_hir::ty;

pub mod mir;

pub use mir::{Module, dump, lower, materialize, verify};
