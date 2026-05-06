//! Materialized MIR 的 canonical callable body / summary 查询视图。
//!
//! 目标：
//! - 让 production/frontend 消费侧不再手动扫描 `MaterializedMir.file.items`；
//! - 保留 `InstanceKey -> root callable / callable family` 的稳定映射；
//! - 明确把 raw `MaterializedMir` 与“按 callable 身份组织的查询入口”分开。

use std::collections::{HashMap, HashSet};

use super::{FunDecl, InstanceKey, InstanceSummary, Item, MaterializedMir};

#[derive(Debug, Clone, Default)]
pub(crate) struct MaterializedCallableFamilies {
    by_instance: HashMap<InstanceKey, MaterializedCallableFamily>,
    owner_by_callable_fqn: HashMap<String, InstanceKey>,
}

#[derive(Debug, Clone)]
pub(crate) struct MaterializedCallableFamilyInput {
    pub(crate) instance: InstanceKey,
    pub(crate) root_fqn: String,
    pub(crate) callable_fqns: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MaterializedCallableFamily {
    pub(crate) root_fqn: String,
    pub(crate) callable_fqns: Vec<String>,
}

impl MaterializedCallableFamilies {
    pub(crate) fn from_inputs(inputs: Vec<MaterializedCallableFamilyInput>) -> Self {
        let mut families = Self {
            by_instance: HashMap::with_capacity(inputs.len()),
            owner_by_callable_fqn: HashMap::with_capacity(
                inputs.iter().map(|input| input.callable_fqns.len()).sum(),
            ),
        };
        for input in inputs {
            families.replace_family(input);
        }
        families
    }

    pub(crate) fn replace_family(&mut self, input: MaterializedCallableFamilyInput) {
        let MaterializedCallableFamilyInput {
            instance,
            root_fqn,
            callable_fqns,
        } = input;
        let callable_fqns = dedup_preserving_order(callable_fqns);

        if let Some(previous) = self.by_instance.remove(&instance) {
            for callable_fqn in previous.callable_fqns {
                let owned_by_instance = self
                    .owner_by_callable_fqn
                    .get(&callable_fqn)
                    .is_some_and(|owner| owner == &instance);
                if owned_by_instance {
                    self.owner_by_callable_fqn.remove(&callable_fqn);
                }
            }
        }

        // family 重写允许把 callable 身份迁移到新的实例；若某个 symbol 之前挂在别的 family
        // 上，需要同步把旧 family 中的记录移除，避免 release 构建里出现“一份 body 属于两个
        // family”的静默脏状态。
        for callable_fqn in &callable_fqns {
            let previous_owner = self.owner_by_callable_fqn.get(callable_fqn).cloned();
            if let Some(previous_owner) = previous_owner
                && previous_owner != instance
            {
                if let Some(previous_family) = self.by_instance.get_mut(&previous_owner) {
                    previous_family
                        .callable_fqns
                        .retain(|fqn| fqn != callable_fqn);
                }
                self.owner_by_callable_fqn.remove(callable_fqn);
            }
        }

        for callable_fqn in &callable_fqns {
            let previous = self
                .owner_by_callable_fqn
                .insert(callable_fqn.clone(), instance.clone());
            debug_assert!(
                previous.is_none() || previous.as_ref() == Some(&instance),
                "callable body `{callable_fqn}` 应只属于一个 materialized instance family"
            );
        }

        self.by_instance.insert(
            instance,
            MaterializedCallableFamily {
                root_fqn,
                callable_fqns,
            },
        );
    }

    pub(crate) fn len(&self) -> usize {
        self.by_instance.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_instance.is_empty()
    }

    pub(crate) fn family_entry(
        &self,
        key: &InstanceKey,
    ) -> Option<(&InstanceKey, &MaterializedCallableFamily)> {
        self.by_instance.get_key_value(key)
    }

    pub(crate) fn owner_of_callable(&self, fqn: &str) -> Option<&InstanceKey> {
        self.owner_by_callable_fqn.get(fqn)
    }
}

fn dedup_preserving_order(callable_fqns: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::with_capacity(callable_fqns.len());
    let mut deduped = Vec::with_capacity(callable_fqns.len());
    for callable_fqn in callable_fqns {
        if seen.insert(callable_fqn.clone()) {
            deduped.push(callable_fqn);
        }
    }
    deduped
}

/// `MaterializedMir` 上“按 callable/instance 身份组织”的只读查询视图。
#[derive(Debug)]
pub struct MaterializedCallableView<'a> {
    materialized: &'a MaterializedMir,
    families: &'a MaterializedCallableFamilies,
    funs_by_fqn: HashMap<&'a str, &'a FunDecl>,
}

impl<'a> MaterializedCallableView<'a> {
    pub(crate) fn new(
        materialized: &'a MaterializedMir,
        families: &'a MaterializedCallableFamilies,
    ) -> Self {
        let funs_by_fqn = materialized
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) if fun.body.is_some() => Some((fun.fqn.as_str(), fun)),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        Self {
            materialized,
            families,
            funs_by_fqn,
        }
    }

    /// 当前视图中可查询的实例数量。
    pub fn len(&self) -> usize {
        self.materialized.instance_keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.materialized.instance_keys.is_empty()
    }

    /// 直接按 callable FQN 查询当前 materialized body。
    pub fn callable(&self, fqn: &str) -> Option<&'a FunDecl> {
        self.funs_by_fqn.get(fqn).copied()
    }

    /// 查询某个 materialized callable body 属于哪个 `InstanceKey` family。
    pub fn owner_of_callable(&self, fqn: &str) -> Option<&'a InstanceKey> {
        self.families.owner_of_callable(fqn)
    }

    /// 按 `InstanceKey` 读取 canonical family 视图。
    pub fn instance(&'a self, key: &InstanceKey) -> Option<MaterializedCallableFamilyView<'a>> {
        let (key, family) = self.families.family_entry(key)?;
        Some(MaterializedCallableFamilyView {
            view: self,
            key,
            family,
        })
    }

    /// 以稳定的 `instance_keys` 顺序遍历所有实例 family。
    pub fn instances(&'a self) -> impl Iterator<Item = MaterializedCallableFamilyView<'a>> + 'a {
        self.materialized
            .instance_keys
            .iter()
            .filter_map(move |key| self.instance(key))
    }
}

/// 某个 `InstanceKey` 在 materialized MIR 中对应的 callable family 视图。
#[derive(Debug, Clone, Copy)]
pub struct MaterializedCallableFamilyView<'a> {
    view: &'a MaterializedCallableView<'a>,
    key: &'a InstanceKey,
    family: &'a MaterializedCallableFamily,
}

impl<'a> MaterializedCallableFamilyView<'a> {
    pub fn key(&self) -> &'a InstanceKey {
        self.key
    }

    /// 当前实例 family 的根 callable symbol。
    pub fn root_fqn(&self) -> &'a str {
        self.family.root_fqn.as_str()
    }

    /// 当前实例的根 callable body；对 declaration-only instance 返回 `None`。
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

    /// 当前实例的 canonical summary。
    pub fn summary(&self) -> &'a InstanceSummary {
        self.view
            .materialized
            .summaries
            .get(self.key)
            .expect("every materialized callable family should have a summary")
    }

    /// 当前实例 family 中记录的 callable FQN 集合。
    pub fn callable_fqns(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.family.callable_fqns.iter().map(String::as_str)
    }

    /// 当前实例 family 中仍存在于 `MaterializedMir.file` 的 callable body。
    pub fn callable_bodies(&self) -> impl Iterator<Item = &'a FunDecl> + 'a {
        let view = self.view;
        self.family
            .callable_fqns
            .iter()
            .filter_map(move |fqn| view.callable(fqn))
    }
}

#[cfg(test)]
mod tests {
    use super::super::ResultProvenance;
    use crate::mir::materialize_for_dump;
    use crate::session::Session;
    use crate::source::SourceFile;
    use std::collections::BTreeSet;

    #[test]
    fn callable_view_keeps_overloaded_generic_roots_distinct() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_callable_view_overload_identity.scoop",
            r#"
package fixtures.mircallable

fun <T> pick(x: T): T {
    return x
}

fun <T> pick(x: T, y: T): T {
    return y
}

fun entry(): Int {
    val a = pick(1)
    val b = pick(1, 2)
    return a + b
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let view = materialized.callable_view();
        let families = view
            .instances()
            .filter(|family| family.key().template.fqn == "fixtures.mircallable.pick")
            .collect::<Vec<_>>();

        assert_eq!(
            families.len(),
            2,
            "同名 generic overload 的 materialized callable view 应保留两个独立实例"
        );

        let root_fqns = families
            .iter()
            .map(|family| family.root_fqn().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(root_fqns.len(), 2);
        assert!(
            root_fqns
                .iter()
                .all(|fqn| fqn.starts_with("fixtures.mircallable.pick::<Int>")),
            "callable view 应保留 `template_fqn::<args>` 前缀，避免消费侧回退到 ad-hoc root symbol 推导: {root_fqns:#?}"
        );

        let mut unary = None;
        let mut binary = None;
        for family in families {
            let owner = view
                .owner_of_callable(family.root_fqn())
                .expect("view 应能从 root callable 反查所属实例");
            assert_eq!(owner, family.key());

            let root = family
                .root_body()
                .expect("有 body 的实例应能在 callable view 里直接读到根 body");
            assert_eq!(
                view.callable(family.root_fqn())
                    .expect("root callable 应可直接按 FQN 查询")
                    .fqn,
                root.fqn
            );
            assert_eq!(
                family.callable_bodies().next().map(|fun| fun.fqn.as_str()),
                Some(family.root_fqn()),
                "family callable 顺序应先给出 root body"
            );

            match root.params.len() {
                1 => unary = Some(family.summary().result_provenance.clone()),
                2 => binary = Some(family.summary().result_provenance.clone()),
                arity => panic!("unexpected overload arity: {arity}"),
            }
        }

        assert_eq!(unary, Some(ResultProvenance::Param(0)));
        assert_eq!(binary, Some(ResultProvenance::Param(1)));
    }
}
