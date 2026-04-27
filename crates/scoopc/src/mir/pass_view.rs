//! production/codegen 主路径消费的 canonical materialized MIR pass 视图。
//!
//! 这层负责：
//! - 把 raw `MaterializedMir` 与当前 pass 链对外暴露的 canonical callable body / summary
//!   / family 映射显式分层；
//! - 为后续 rewrite/inlining 提供稳定 side table，而不是把 pass 结果直接回写成“所有调用方
//!   都只能看到的唯一 raw materialization”；
//! - 让 production/build/codegen 可以先接到稳定的 pass 产物层，再逐步切掉对 HIR 兼容 body
//!   的隐式依赖。

use std::collections::{HashMap, HashSet};

use super::callables::MaterializedCallableFamily;
use super::{
    File, FunDecl, InstanceKey, InstanceSummary, Item, MaterializedCallableFamilies,
    MaterializedCallableFamilyInput, MaterializedMir, MaterializedMirSummaries,
};

/// `MaterializedMir` 上“当前 pass 后 canonical callable 产物”的稳定 side table。
///
/// 设计意图：
/// - raw `MaterializedMir.file` / `summaries` 继续保留 materialization 原始产物，便于 dump/调试；
/// - pass rewrite 通过这层覆盖 callable body / per-instance summary / family 映射；
/// - production 消费侧显式经由 `MaterializedMir::pass_view()` 读取 pass 后结果，而不是猜测
///   “是不是有人直接改过 raw MIR”。
#[derive(Debug, Clone)]
pub struct MaterializedMirPassArtifacts {
    callable_bodies_by_fqn: HashMap<String, FunDecl>,
    callable_families: MaterializedCallableFamilies,
    summaries: MaterializedMirSummaries,
    overridden_body_fqns: HashSet<String>,
    overridden_summary_instances: HashSet<InstanceKey>,
}

impl MaterializedMirPassArtifacts {
    pub(crate) fn from_raw_materialized(
        file: &File,
        summaries: &MaterializedMirSummaries,
        callable_families: &MaterializedCallableFamilies,
    ) -> Self {
        let callable_bodies_by_fqn = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) if fun.body.is_some() => Some((fun.fqn.clone(), fun.clone())),
                Item::Fun(_) | Item::Todo { .. } => None,
            })
            .collect::<HashMap<_, _>>();
        Self {
            callable_bodies_by_fqn,
            callable_families: callable_families.clone(),
            summaries: summaries.clone(),
            overridden_body_fqns: HashSet::new(),
            overridden_summary_instances: HashSet::new(),
        }
    }

    /// 覆盖某个 callable 的 canonical pass body。
    ///
    /// 说明：
    /// - 若 `fun.body.is_some()`，则用该 body 替换当前 pass 产物层中的 callable；
    /// - 若 `fun.body.is_none()`，则等价于把该 callable 从 pass 可见 body 集中移除；
    /// - `fun` 可以是 instance family 中的 callable，也可以是 pass 明确改写的
    ///   request-root / non-generic caller body；
    /// - 该操作不会修改 raw `MaterializedMir.file`。
    pub fn replace_callable_body(&mut self, fun: FunDecl) -> Option<FunDecl> {
        self.overridden_body_fqns.insert(fun.fqn.clone());
        if fun.body.is_some() {
            self.callable_bodies_by_fqn.insert(fun.fqn.clone(), fun)
        } else {
            self.callable_bodies_by_fqn.remove(&fun.fqn)
        }
    }

    /// 从当前 pass 可见 body 集中移除某个 callable。
    ///
    /// 说明：family 映射与 summary 不会自动删除，便于表达“callable 身份仍存在，但当前没有
    /// canonical body”的 declaration-only / helper-only 形态。
    pub fn remove_callable_body(&mut self, fqn: &str) -> Option<FunDecl> {
        self.overridden_body_fqns.insert(fqn.to_string());
        self.callable_bodies_by_fqn.remove(fqn)
    }

    /// 覆盖某个单态实例的 canonical pass summary。
    pub fn set_instance_summary(
        &mut self,
        key: InstanceKey,
        summary: InstanceSummary,
    ) -> Option<InstanceSummary> {
        self.overridden_summary_instances.insert(key.clone());
        self.summaries.insert(key, summary)
    }

    /// 覆盖某个实例 family 的 canonical root/callable 映射。
    ///
    /// 注意：
    /// - 该映射只更新 pass 产物层，不会改动 raw `MaterializedMir` 自带的 family side table；
    /// - 若 `callable_fqns` 包含当前没有 body 的 symbol，它仍会出现在 family 映射中，但
    ///   `callable(...)` / `callable_bodies()` 会返回 `None` / 跳过它。
    pub fn replace_callable_family(
        &mut self,
        instance: InstanceKey,
        root_fqn: String,
        callable_fqns: Vec<String>,
    ) {
        self.callable_families
            .replace_family(MaterializedCallableFamilyInput {
                instance,
                root_fqn,
                callable_fqns,
            });
    }

    fn callable_body(&self, fqn: &str) -> Option<&FunDecl> {
        self.callable_bodies_by_fqn.get(fqn)
    }

    fn body_is_overridden(&self, fqn: &str) -> bool {
        self.overridden_body_fqns.contains(fqn)
    }

    fn families(&self) -> &MaterializedCallableFamilies {
        &self.callable_families
    }

    fn summaries(&self) -> &MaterializedMirSummaries {
        &self.summaries
    }

    fn summary_is_overridden(&self, key: &InstanceKey) -> bool {
        self.overridden_summary_instances.contains(key)
    }
}

/// 当前 pass 产物上的 canonical callable body / summary 查询面。
///
/// 与 raw `MaterializedCallableView` 的区别：
/// - 这里查询的是 pass 产物层中的 canonical callable body / summary / family 映射；
/// - raw materialized MIR 继续可通过 `MaterializedMir::callable_view()` 直接观察，不会被
///   pass rewrite 隐式覆盖。
#[derive(Debug)]
pub struct MaterializedPassCallableView<'a> {
    instance_keys: &'a [InstanceKey],
    pass_artifacts: &'a MaterializedMirPassArtifacts,
}

impl<'a> MaterializedPassCallableView<'a> {
    fn new(materialized: &'a MaterializedMir) -> Self {
        Self {
            instance_keys: &materialized.instance_keys,
            pass_artifacts: materialized.pass_artifacts(),
        }
    }

    /// 当前视图中可查询的实例数量。
    pub fn len(&self) -> usize {
        self.pass_artifacts.families().len()
    }

    pub fn is_empty(&self) -> bool {
        self.pass_artifacts.families().is_empty()
    }

    /// 直接按 callable FQN 查询当前 pass 后的 canonical body。
    pub fn callable(&self, fqn: &str) -> Option<&'a FunDecl> {
        self.pass_artifacts.callable_body(fqn)
    }

    /// 该 callable body 是否由 MIR pass 显式覆盖或移除。
    pub fn body_is_overridden(&self, fqn: &str) -> bool {
        self.pass_artifacts.body_is_overridden(fqn)
    }

    /// 查询某个 callable 目前归属哪个实例 family。
    pub fn owner_of_callable(&self, fqn: &str) -> Option<&'a InstanceKey> {
        self.pass_artifacts.families().owner_of_callable(fqn)
    }

    /// 按 `InstanceKey` 读取当前 pass 产物层中的 canonical family。
    pub fn instance(&'a self, key: &InstanceKey) -> Option<MaterializedPassCallableFamilyView<'a>> {
        let (key, family) = self.pass_artifacts.families().family_entry(key)?;
        Some(MaterializedPassCallableFamilyView {
            view: self,
            key,
            family,
        })
    }

    /// 以稳定的 `instance_keys` 顺序遍历所有实例 family。
    pub fn instances(
        &'a self,
    ) -> impl Iterator<Item = MaterializedPassCallableFamilyView<'a>> + 'a {
        self.instance_keys
            .iter()
            .filter_map(move |key| self.instance(key))
    }
}

/// 某个 `InstanceKey` 在当前 pass 产物层中对应的 callable family。
#[derive(Debug, Clone, Copy)]
pub struct MaterializedPassCallableFamilyView<'a> {
    view: &'a MaterializedPassCallableView<'a>,
    key: &'a InstanceKey,
    family: &'a MaterializedCallableFamily,
}

impl<'a> MaterializedPassCallableFamilyView<'a> {
    pub fn key(&self) -> &'a InstanceKey {
        self.key
    }

    /// 当前实例 family 的 canonical root callable symbol。
    pub fn root_fqn(&self) -> &'a str {
        self.family.root_fqn.as_str()
    }

    /// 当前实例的 canonical root callable body；对 declaration-only/root body 已被移除的
    /// instance 返回 `None`。
    pub fn root_body(&self) -> Option<&'a FunDecl> {
        if !self
            .family
            .callable_fqns
            .iter()
            .any(|fqn| fqn == self.root_fqn())
        {
            return None;
        }
        self.view.callable(self.root_fqn())
    }

    /// 当前实例的 canonical pass summary。
    pub fn summary(&self) -> &'a InstanceSummary {
        self.view
            .pass_artifacts
            .summaries()
            .get(self.key)
            .expect("every pass-visible callable family should have a summary")
    }

    /// 该实例 summary 是否由 MIR pass 显式覆盖。
    ///
    /// raw materialization 初始化的 summary 主要服务 MIR side table；LLVM production 的
    /// effect/suspend cache 只把显式 pass override 当作 canonical rewrite 结果消费，避免把
    /// 仍需 HIR/effect 分析补充的初始 summary 提前当成完整后端事实。
    pub fn summary_is_overridden(&self) -> bool {
        self.view.pass_artifacts.summary_is_overridden(self.key)
    }

    /// 当前实例 family 中记录的 callable FQN 集合。
    pub fn callable_fqns(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.family.callable_fqns.iter().map(String::as_str)
    }

    /// 当前实例 family 中仍存在 canonical pass body 的 callable 集合。
    pub fn callable_bodies(&self) -> impl Iterator<Item = &'a FunDecl> + 'a {
        let view = self.view;
        self.family
            .callable_fqns
            .iter()
            .filter_map(move |fqn| view.callable(fqn))
    }
}

/// production/codegen 主路径上的 canonical materialized MIR pass 视图。
///
/// 当前阶段先承接：
/// - raw `MaterializedMir` 与 pass 产物层之间的显式分层；
/// - canonical callable body / summary / family 映射查询；
/// - 后续 MIR rewrite / inlining 的稳定挂点。
#[derive(Debug)]
pub struct MaterializedMirPassView<'a> {
    materialized: &'a MaterializedMir,
    callables: MaterializedPassCallableView<'a>,
}

impl<'a> MaterializedMirPassView<'a> {
    pub(crate) fn new(materialized: &'a MaterializedMir) -> Self {
        Self {
            materialized,
            callables: MaterializedPassCallableView::new(materialized),
        }
    }

    /// 返回底层 raw materialized MIR；主要供调试/测试直接观察原始产物。
    pub fn materialized(&self) -> &'a MaterializedMir {
        self.materialized
    }

    /// 返回当前 pass 后的 canonical callable body / summary 查询面。
    pub fn callables(&self) -> &MaterializedPassCallableView<'a> {
        &self.callables
    }

    /// 当前视图中可查询的实例数量。
    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    /// 直接按 callable FQN 查询当前 pass 后的 canonical body。
    pub fn callable(&self, fqn: &str) -> Option<&'a FunDecl> {
        self.callables.callable(fqn)
    }

    /// 该 callable body 是否由 MIR pass 显式覆盖或移除。
    pub fn callable_body_is_overridden(&self, fqn: &str) -> bool {
        self.callables.body_is_overridden(fqn)
    }

    /// 查询某个 callable 当前归属哪个实例 family。
    pub fn owner_of_callable(&self, fqn: &str) -> Option<&'a InstanceKey> {
        self.callables.owner_of_callable(fqn)
    }

    /// 直接按 `InstanceKey` 查询当前实例 family。
    pub fn instance(&'a self, key: &InstanceKey) -> Option<MaterializedPassCallableFamilyView<'a>> {
        self.callables.instance(key)
    }

    /// 以稳定的 `instance_keys` 顺序遍历所有实例 family。
    pub fn instances(
        &'a self,
    ) -> impl Iterator<Item = MaterializedPassCallableFamilyView<'a>> + 'a {
        self.callables.instances()
    }

    /// 读取某个 materialized root callable 对应的 canonical family。
    ///
    /// 仅当 `fqn` 恰好是当前 pass 产物层记录的 family root symbol 时返回结果；若它只是 family
    /// 内部 helper body，则返回 `None`。
    pub fn root_family_for_fqn(
        &'a self,
        fqn: &str,
    ) -> Option<MaterializedPassCallableFamilyView<'a>> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::{Statement, StatementKind};
    use crate::mir::materialize_for_dump;
    use crate::session::Session;
    use crate::source::SourceFile;

    fn pass_view_fixture_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/mir_pass_view_fixture.scoop",
            r#"
package fixtures.mirpass

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    return wrap<Int>(1)
}
"#,
        )
    }

    fn pass_view_cross_family_rehome_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/mir_pass_view_cross_family_rehome.scoop",
            r#"
package fixtures.mirpass

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    val text = wrap<String>("hello")
    val number = wrap<Int>(1)
    return number
}
"#,
        )
    }

    #[test]
    fn pass_view_keeps_rewritten_body_and_summary_separate_from_raw_materialized_mir() {
        let sess = Session::new().unwrap();
        let mut materialized = materialize_for_dump(&sess, &pass_view_fixture_source()).unwrap();

        let (key, root_fqn, raw_stmt_len, raw_summary, rewritten_root, rewritten_summary) = {
            let raw_view = materialized.callable_view();
            let family = raw_view
                .instances()
                .find(|family| family.key().template.fqn == "fixtures.mirpass.wrap")
                .expect("应能在 raw materialized MIR 中找到 `wrap::<Int>` family");
            let raw_root = family
                .root_body()
                .expect("wrap::<Int> 应在 raw materialized MIR 中保留 body")
                .clone();
            let mut rewritten_root = raw_root.clone();
            let body = rewritten_root
                .body
                .as_mut()
                .expect("rewritten root 应保留 body");
            let start = body.start.as_u32() as usize;
            let raw_stmt_len = body.blocks[start].stmts.len();
            body.blocks[start].stmts.push(Statement {
                span: rewritten_root.span,
                kind: StatementKind::Nop,
            });

            let mut rewritten_summary = family.summary().clone();
            rewritten_summary.size_cost += 7;

            (
                family.key().clone(),
                family.root_fqn().to_string(),
                raw_stmt_len,
                family.summary().clone(),
                rewritten_root,
                rewritten_summary,
            )
        };

        {
            let pass_artifacts = materialized.pass_artifacts_mut();
            pass_artifacts.replace_callable_body(rewritten_root);
            pass_artifacts.set_instance_summary(key.clone(), rewritten_summary.clone());
        }

        let pass_view = materialized.pass_view();
        let pass_family = pass_view
            .instance(&key)
            .expect("pass view 应继续保留 `wrap::<Int>` family");
        let pass_root = pass_family
            .root_body()
            .expect("pass view 应能读取 rewritten root body");
        let pass_body = pass_root.body.as_ref().expect("rewritten root 应保留 body");
        let pass_stmt_len = pass_body.blocks[pass_body.start.as_u32() as usize]
            .stmts
            .len();
        assert_eq!(
            pass_stmt_len,
            raw_stmt_len + 1,
            "pass view 应观察到 rewritten body，而不是继续读 raw materialized body"
        );
        assert_eq!(
            pass_view.root_summary(&root_fqn),
            Some(&rewritten_summary),
            "pass view 应返回更新后的 canonical summary"
        );

        let raw_view = materialized.callable_view();
        let raw_family = raw_view
            .instance(&key)
            .expect("raw callable view 应仍保留原始 family");
        let raw_root = raw_family
            .root_body()
            .expect("raw callable view 应继续保留原始 body");
        let raw_body = raw_root.body.as_ref().expect("raw root 应保留 body");
        assert_eq!(
            raw_body.blocks[raw_body.start.as_u32() as usize]
                .stmts
                .len(),
            raw_stmt_len,
            "raw materialized MIR 不应被 pass rewrite 隐式覆盖"
        );
        assert_eq!(
            raw_family.summary(),
            &raw_summary,
            "raw summary 不应被 pass summary override 隐式覆盖"
        );
    }

    #[test]
    fn pass_view_can_override_family_mapping_without_mutating_raw_materialization() {
        let sess = Session::new().unwrap();
        let mut materialized = materialize_for_dump(&sess, &pass_view_fixture_source()).unwrap();

        let (key, root_fqn, raw_callable_fqns) = {
            let raw_view = materialized.callable_view();
            let family = raw_view
                .instances()
                .find(|family| family.key().template.fqn == "fixtures.mirpass.wrap")
                .expect("应能在 raw materialized MIR 中找到 `wrap::<Int>` family");
            (
                family.key().clone(),
                family.root_fqn().to_string(),
                family
                    .callable_fqns()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
        };
        let synthetic_helper = format!("{root_fqn}#pass-inline-helper");

        materialized.pass_artifacts_mut().replace_callable_family(
            key.clone(),
            root_fqn.clone(),
            raw_callable_fqns
                .iter()
                .cloned()
                .chain(std::iter::once(synthetic_helper.clone()))
                .collect(),
        );

        let pass_view = materialized.pass_view();
        let pass_family = pass_view
            .instance(&key)
            .expect("pass view 应继续保留该实例 family");
        let pass_callable_fqns = pass_family.callable_fqns().collect::<BTreeSet<_>>();
        assert!(
            pass_callable_fqns.contains(synthetic_helper.as_str()),
            "pass view 应能暴露仅存在于 pass family side table 的 helper symbol"
        );
        assert_eq!(
            pass_view.owner_of_callable(&synthetic_helper),
            Some(&key),
            "pass view 应按重写后的 family 映射识别 helper 所属实例"
        );
        assert!(
            pass_view.callable(&synthetic_helper).is_none(),
            "family 映射可以先暴露 callable 身份，而无需强制 raw materialized MIR 同时提供 body"
        );

        let raw_view = materialized.callable_view();
        assert!(
            raw_view.owner_of_callable(&synthetic_helper).is_none(),
            "raw materialized callable view 不应被 pass family 映射覆盖"
        );
        let raw_callable_fqns_after = raw_view
            .instance(&key)
            .expect("raw callable view 应继续保留原始 family")
            .callable_fqns()
            .collect::<BTreeSet<_>>();
        let raw_callable_fqns_before = raw_callable_fqns
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            raw_callable_fqns_after, raw_callable_fqns_before,
            "raw family mapping 不应因 pass side table 重写而变化"
        );
    }

    #[test]
    fn pass_view_rehomes_callable_across_families_without_leaving_duplicate_membership() {
        let sess = Session::new().unwrap();
        let mut materialized =
            materialize_for_dump(&sess, &pass_view_cross_family_rehome_source()).unwrap();

        let (source_key, source_root, target_key, target_root, target_fqns) = {
            let raw_view = materialized.callable_view();
            let wrap_families = raw_view
                .instances()
                .filter(|family| family.key().template.fqn == "fixtures.mirpass.wrap")
                .collect::<Vec<_>>();
            assert_eq!(
                wrap_families.len(),
                2,
                "fixture 应 materialize 出两个 `wrap` family"
            );

            let source_family = wrap_families
                .iter()
                .find(|family| family.root_fqn().contains("String"))
                .expect("应能找到 `wrap::<String>` family");
            let target_family = wrap_families
                .iter()
                .find(|family| family.root_fqn().contains("Int"))
                .expect("应能找到 `wrap::<Int>` family");
            (
                source_family.key().clone(),
                source_family.root_fqn().to_string(),
                target_family.key().clone(),
                target_family.root_fqn().to_string(),
                target_family
                    .callable_fqns()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
        };

        materialized.pass_artifacts_mut().replace_callable_family(
            target_key.clone(),
            target_root.clone(),
            target_fqns
                .into_iter()
                .chain(std::iter::once(source_root.clone()))
                .collect(),
        );

        let pass_view = materialized.pass_view();
        assert_eq!(
            pass_view.owner_of_callable(&source_root),
            Some(&target_key),
            "迁移后的 callable 应只归属目标 family"
        );

        let source_family = pass_view
            .instance(&source_key)
            .expect("pass view 应继续保留源 family 身份");
        assert!(
            !source_family.callable_fqns().any(|fqn| fqn == source_root),
            "源 family 不应继续保留已迁走的 callable 记录"
        );
        assert!(
            source_family.root_body().is_none(),
            "源 family 的 root 在迁出后应退化为无 body 的 family 身份"
        );

        let target_family = pass_view
            .instance(&target_key)
            .expect("pass view 应继续保留目标 family");
        assert!(
            target_family.callable_fqns().any(|fqn| fqn == source_root),
            "目标 family 应接管迁入的 callable 身份"
        );

        let duplicate_memberships = pass_view
            .instances()
            .filter(|family| family.callable_fqns().any(|fqn| fqn == source_root))
            .count();
        assert_eq!(
            duplicate_memberships, 1,
            "同一个 callable 在 pass family 重写后不应同时残留在两个 family"
        );

        let raw_view = materialized.callable_view();
        let raw_source_family = raw_view
            .instance(&source_key)
            .expect("raw callable view 应继续保留原始源 family");
        assert!(
            raw_source_family
                .callable_fqns()
                .any(|fqn| fqn == source_root),
            "raw materialized family 不应被 pass family 重写影响"
        );
    }
}
