//! production/codegen 主路径消费的 canonical materialized MIR pass 视图。
//!
//! 这层负责：
//! - 把 raw `MaterializedMir` 与 production 入口真正要消费的查询面分开；
//! - 暴露 callable body / per-instance summary / 后续 pass rewrite 的稳定挂点；
//! - 避免 LLVM build / single-file entry 继续隐式退回到只看 HIR 兼容 body。

use super::{
    FunDecl, InstanceKey, InstanceSummary, MaterializedCallableFamilyView,
    MaterializedCallableView, MaterializedMir,
};

/// production/codegen 主路径上的 canonical materialized MIR pass 视图。
///
/// 当前阶段先承接：
/// - canonical callable body 查询；
/// - per-instance summary 查询；
/// - root callable FQN 到 materialized instance family 的稳定映射。
///
/// 后续 MIR rewrite / inlining / 其它 pass 产物会继续沿这层扩展，而不是重新把查询面埋回
/// `MaterializedMir.file.items` 或 HIR lowering。
#[derive(Debug)]
pub struct MaterializedMirPassView<'a> {
    materialized: &'a MaterializedMir,
    callables: MaterializedCallableView<'a>,
}

impl<'a> MaterializedMirPassView<'a> {
    pub(crate) fn new(materialized: &'a MaterializedMir) -> Self {
        Self {
            materialized,
            callables: materialized.callable_view(),
        }
    }

    /// 返回底层 raw materialized MIR；主要供调试/测试直接观察原始产物。
    pub fn materialized(&self) -> &'a MaterializedMir {
        self.materialized
    }

    /// 返回 canonical callable body / summary 查询面。
    pub fn callables(&self) -> &MaterializedCallableView<'a> {
        &self.callables
    }

    /// 直接按 `InstanceKey` 查询当前实例 family。
    pub fn instance(&'a self, key: &InstanceKey) -> Option<MaterializedCallableFamilyView<'a>> {
        self.callables.instance(key)
    }

    /// 读取某个 materialized root callable 对应的 canonical family。
    ///
    /// 仅当 `fqn` 恰好是 family root symbol 时返回结果；若它只是 family 内部 helper body，
    /// 则返回 `None`。
    pub fn root_family_for_fqn(&'a self, fqn: &str) -> Option<MaterializedCallableFamilyView<'a>> {
        let owner = self.callables.owner_of_callable(fqn)?;
        let family = self.callables.instance(owner)?;
        (family.root_fqn() == fqn).then_some(family)
    }

    /// 读取某个 materialized root callable 的 canonical body。
    pub fn root_body(&'a self, fqn: &str) -> Option<&'a FunDecl> {
        self.root_family_for_fqn(fqn)
            .and_then(|family| family.root_body())
    }

    /// 读取某个 materialized root callable 的 canonical summary。
    pub fn root_summary(&'a self, fqn: &str) -> Option<&'a InstanceSummary> {
        self.root_family_for_fqn(fqn).map(|family| family.summary())
    }
}
