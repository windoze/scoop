use scoop_lir::ValidatedLirTargetSelection;
use scoop_toolchain::ResolvedTargetProfile;

#[derive(Debug)]
pub(super) enum SelectedTarget {
    Frontend(ValidatedLirTargetSelection),
    Production(Box<ResolvedTargetProfile>),
}

impl SelectedTarget {
    pub(super) fn selection(&self) -> ValidatedLirTargetSelection {
        match self {
            Self::Frontend(target) => *target,
            Self::Production(target) => target.lir_target_selection(),
        }
    }

    pub(super) fn production(&self) -> &ResolvedTargetProfile {
        match self {
            Self::Production(target) => target,
            Self::Frontend(_) => unreachable!("documentation returns before machine production"),
        }
    }
}
