use std::collections::HashMap;

use crate::mir::MaterializedMir;

use super::{BodyEffectFacts, CallableEffectFacts, MaterializedEffectFacts, MirSnapshotBinding};

/// 从 canonical materialized MIR snapshot 生成 P4 facts 容器外壳。
#[derive(Debug)]
pub struct MaterializedEffectFactsBuilder<'a> {
    materialized: &'a MaterializedMir,
}

impl<'a> MaterializedEffectFactsBuilder<'a> {
    pub fn from_materialized_snapshot(materialized: &'a MaterializedMir) -> Self {
        Self { materialized }
    }

    pub fn build(self) -> MaterializedEffectFacts {
        let pass_view = self.materialized.pass_view();
        let mut callable_facts = HashMap::with_capacity(pass_view.len());
        let mut bodies = HashMap::with_capacity(pass_view.len());

        for family in pass_view.instances() {
            let key = family.key().clone();
            callable_facts.insert(key.clone(), CallableEffectFacts::default());
            bodies.insert(key, BodyEffectFacts::default());
        }

        MaterializedEffectFacts::new(
            MirSnapshotBinding::from_pass_view(&pass_view),
            callable_facts,
            bodies,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::MaterializedEffectFactsBuilder;
    use crate::effect_facts::CanonicalMirQuerySurface;
    use crate::mir::{Item, materialize_for_dump};
    use crate::session::Session;
    use crate::source::SourceFile;

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_builder_fixture.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        )
    }

    #[test]
    fn materialized_effect_facts_builder_uses_canonical_pass_view_snapshot() {
        let session = Session::new().unwrap();
        let mut materialized = materialize_for_dump(&session, &sample_source()).unwrap();
        let removed_fqn = materialized
            .pass_view()
            .instances()
            .next()
            .expect("fixture 应该产生至少一个 instance")
            .root_fqn()
            .to_string();

        assert!(
            materialized
                .file
                .items
                .iter()
                .any(|item| { matches!(item, Item::Fun(fun) if fun.fqn == removed_fqn) })
        );

        materialized
            .pass_artifacts_mut()
            .remove_callable_body(&removed_fqn);

        let facts =
            MaterializedEffectFactsBuilder::from_materialized_snapshot(&materialized).build();

        assert_eq!(
            facts.snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(facts.callable_facts().len(), materialized.pass_view().len());
        assert!(
            !facts
                .snapshot_binding()
                .canonical_body_fqns()
                .iter()
                .any(|fqn| fqn == &removed_fqn)
        );
        assert!(
            materialized
                .file
                .items
                .iter()
                .any(|item| { matches!(item, Item::Fun(fun) if fun.fqn == removed_fqn) })
        );
    }
}
