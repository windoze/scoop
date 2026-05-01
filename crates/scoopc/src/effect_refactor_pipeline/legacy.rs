use crate::session::EffectPipelineMode;

use super::StageKind;

/// legacy 主线的阶段入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StageEntry {
    stage: StageKind,
}

impl StageEntry {
    pub(crate) const fn new(stage: StageKind) -> Self {
        Self { stage }
    }

    pub(crate) const fn mode(self) -> EffectPipelineMode {
        EffectPipelineMode::Legacy
    }

    pub(crate) const fn stage(self) -> StageKind {
        self.stage
    }

    pub(crate) fn delegate_to_legacy<T>(self, legacy_op: impl FnOnce() -> T) -> T {
        let _ = self;
        legacy_op()
    }
}
