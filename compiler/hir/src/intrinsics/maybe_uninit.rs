/// Operations on one exact MaybeUninit application; no initialization state is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaybeUninitIntrinsic {
    Uninit,
    Initialized,
    AssumeInit,
}

impl MaybeUninitIntrinsic {
    pub const ALL: [Self; 3] = [Self::Uninit, Self::Initialized, Self::AssumeInit];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninit => "maybe_uninit_zero",
            Self::Initialized => "maybe_uninit_initialized",
            Self::AssumeInit => "maybe_uninit_assume_init",
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Uninit => "uninit",
            Self::Initialized => "initialized",
            Self::AssumeInit => "assumeInit",
        }
    }

    pub const fn wire_tag(self) -> u64 {
        match self {
            Self::Uninit => 1,
            Self::Initialized => 2,
            Self::AssumeInit => 3,
        }
    }
}
