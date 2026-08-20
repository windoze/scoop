use crate::AtomicValueKind;

mod wire;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicIntrinsic {
    family: AtomicValueKind,
    method: AtomicMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicMethod {
    Load,
    Store,
    Exchange,
    CompareAndSet,
    CompareAndExchange,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
}

impl AtomicIntrinsic {
    pub fn all() -> impl Iterator<Item = Self> {
        AtomicValueKind::ALL.iter().copied().flat_map(|family| {
            AtomicMethod::ALL
                .iter()
                .copied()
                .filter_map(move |method| Self::new(family, method))
        })
    }

    pub const fn new(family: AtomicValueKind, method: AtomicMethod) -> Option<Self> {
        let supported = match method {
            AtomicMethod::FetchAdd | AtomicMethod::FetchSub => {
                matches!(family, AtomicValueKind::Int | AtomicValueKind::Long)
            }
            AtomicMethod::FetchAnd | AtomicMethod::FetchOr | AtomicMethod::FetchXor => {
                !matches!(family, AtomicValueKind::Reference)
            }
            _ => true,
        };
        if supported {
            Some(Self { family, method })
        } else {
            None
        }
    }

    pub const fn family(self) -> AtomicValueKind {
        self.family
    }
    pub const fn method(self) -> AtomicMethod {
        self.method
    }

    pub fn name(self) -> String {
        format!(
            "{}_{}",
            self.family.registry_key(),
            self.method.registry_key()
        )
    }
}

impl AtomicMethod {
    pub const ALL: &[Self] = &[
        Self::Load,
        Self::Store,
        Self::Exchange,
        Self::CompareAndSet,
        Self::CompareAndExchange,
        Self::FetchAdd,
        Self::FetchSub,
        Self::FetchAnd,
        Self::FetchOr,
        Self::FetchXor,
    ];

    pub const fn value_parameter_count(self) -> usize {
        match self {
            Self::Load => 0,
            Self::CompareAndSet | Self::CompareAndExchange => 2,
            _ => 1,
        }
    }

    pub const fn order_parameter_count(self) -> usize {
        match self {
            Self::CompareAndSet | Self::CompareAndExchange => 2,
            _ => 1,
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Store => "store",
            Self::Exchange => "exchange",
            Self::CompareAndSet => "compareAndSet",
            Self::CompareAndExchange => "compareAndExchange",
            Self::FetchAdd => "fetchAdd",
            Self::FetchSub => "fetchSub",
            Self::FetchAnd => "fetchAnd",
            Self::FetchOr => "fetchOr",
            Self::FetchXor => "fetchXor",
        }
    }

    const fn registry_key(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Store => "store",
            Self::Exchange => "exchange",
            Self::CompareAndSet => "compare_and_set",
            Self::CompareAndExchange => "compare_and_exchange",
            Self::FetchAdd => "fetch_add",
            Self::FetchSub => "fetch_sub",
            Self::FetchAnd => "fetch_and",
            Self::FetchOr => "fetch_or",
            Self::FetchXor => "fetch_xor",
        }
    }
}
