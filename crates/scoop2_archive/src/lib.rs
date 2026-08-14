//! scoop2 分阶段编译：管线胶水 + 阶段 archive（PLAN.md M1，v0 过渡格式）。
//!
//! - [`pipeline`]：源码 → 解析 → typecheck 的可复用胶水（自 scoop2c 抽取，
//!   供 CLI 与测试共用）；
//! - [`v0`]：v0 阶段 archive 的写出 / 装配（显式 transitional：per-cone AST 文件
//!   + collection 级共享 `TypedHir`/`Interner` 段；M2 element 体系落地后退役）。
//!
//! 本 crate 是「每阶段只依赖上一阶段产出」纪律（PLAN.md C1/C8）的落地点：
//! `mir build` 只读取 HIR archive collection，不读源文件 / 不重新 parse。

pub mod pipeline;
pub mod v0;
