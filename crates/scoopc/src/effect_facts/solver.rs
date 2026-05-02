use super::MaterializedEffectFacts;

/// P4-T01 先固定 solver 边界；真正的 `resolved_outward_cases` 求解在后续任务补齐。
#[derive(Debug, Default)]
pub struct MaterializedEffectFactsSolver;

impl MaterializedEffectFactsSolver {
    pub fn solve(&self, facts: MaterializedEffectFacts) -> MaterializedEffectFacts {
        facts
    }
}
