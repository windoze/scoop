use crate::session::EffectPipelineMode;

use super::StageKind;

/// refactor 主线的阶段入口。
///
/// P0 只负责把入口形状固定下来；真正的新实现会在后续任务逐步填到这些 stage entry 后面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageEntry {
    stage: StageKind,
}

impl StageEntry {
    pub(crate) const fn new(stage: StageKind) -> Self {
        Self { stage }
    }

    pub(crate) const fn mode(self) -> EffectPipelineMode {
        EffectPipelineMode::Refactor
    }

    pub(crate) const fn stage(self) -> StageKind {
        self.stage
    }

    pub(crate) fn delegate_to_legacy<T>(self, legacy_op: impl FnOnce() -> T) -> T {
        let _ = self;
        legacy_op()
    }
}
