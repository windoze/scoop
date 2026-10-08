//! Atomic value kinds and complete operation orderings shared by the IRs.

mod wire;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicValueKind {
    Int,
    Long,
    Boolean,
    Reference,
}

impl AtomicValueKind {
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Int => "AtomicInt",
            Self::Long => "AtomicLong",
            Self::Boolean => "AtomicBoolean",
            Self::Reference => "AtomicRef",
        }
    }

    pub const fn registry_key(self) -> &'static str {
        match self {
            Self::Int => "atomic_int",
            Self::Long => "atomic_long",
            Self::Boolean => "atomic_boolean",
            Self::Reference => "atomic_ref",
        }
    }

    pub const fn bytes(self) -> u32 {
        match self {
            Self::Int => 4,
            Self::Long | Self::Reference => 8,
            Self::Boolean => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicMemoryOrder {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicLoadOrder {
    Relaxed,
    Acquire,
    SeqCst,
}

impl AtomicLoadOrder {
    pub const fn from_memory_order(order: AtomicMemoryOrder) -> Option<Self> {
        match order {
            AtomicMemoryOrder::Relaxed => Some(Self::Relaxed),
            AtomicMemoryOrder::Acquire => Some(Self::Acquire),
            AtomicMemoryOrder::SeqCst => Some(Self::SeqCst),
            AtomicMemoryOrder::Release | AtomicMemoryOrder::AcqRel => None,
        }
    }

    pub const fn memory_order(self) -> AtomicMemoryOrder {
        match self {
            Self::Relaxed => AtomicMemoryOrder::Relaxed,
            Self::Acquire => AtomicMemoryOrder::Acquire,
            Self::SeqCst => AtomicMemoryOrder::SeqCst,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicStoreOrder {
    Relaxed,
    Release,
    SeqCst,
}

impl AtomicStoreOrder {
    pub const fn from_memory_order(order: AtomicMemoryOrder) -> Option<Self> {
        match order {
            AtomicMemoryOrder::Relaxed => Some(Self::Relaxed),
            AtomicMemoryOrder::Release => Some(Self::Release),
            AtomicMemoryOrder::SeqCst => Some(Self::SeqCst),
            AtomicMemoryOrder::Acquire | AtomicMemoryOrder::AcqRel => None,
        }
    }

    pub const fn memory_order(self) -> AtomicMemoryOrder {
        match self {
            Self::Relaxed => AtomicMemoryOrder::Relaxed,
            Self::Release => AtomicMemoryOrder::Release,
            Self::SeqCst => AtomicMemoryOrder::SeqCst,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeOrder {
    Relaxed,
    AcquireRelaxed,
    Acquire,
    Release,
    AcqRelRelaxed,
    AcqRelAcquire,
    SeqCstRelaxed,
    SeqCstAcquire,
    SeqCst,
}

impl AtomicCompareExchangeOrder {
    pub const fn from_memory_orders(
        success: AtomicMemoryOrder,
        failure: AtomicMemoryOrder,
    ) -> Option<Self> {
        use AtomicMemoryOrder::*;
        match (success, failure) {
            (Relaxed, Relaxed) => Some(Self::Relaxed),
            (Acquire, Relaxed) => Some(Self::AcquireRelaxed),
            (Acquire, Acquire) => Some(Self::Acquire),
            (Release, Relaxed) => Some(Self::Release),
            (AcqRel, Relaxed) => Some(Self::AcqRelRelaxed),
            (AcqRel, Acquire) => Some(Self::AcqRelAcquire),
            (SeqCst, Relaxed) => Some(Self::SeqCstRelaxed),
            (SeqCst, Acquire) => Some(Self::SeqCstAcquire),
            (SeqCst, SeqCst) => Some(Self::SeqCst),
            _ => None,
        }
    }

    pub const fn memory_orders(self) -> (AtomicMemoryOrder, AtomicMemoryOrder) {
        use AtomicMemoryOrder::*;
        match self {
            Self::Relaxed => (Relaxed, Relaxed),
            Self::AcquireRelaxed => (Acquire, Relaxed),
            Self::Acquire => (Acquire, Acquire),
            Self::Release => (Release, Relaxed),
            Self::AcqRelRelaxed => (AcqRel, Relaxed),
            Self::AcqRelAcquire => (AcqRel, Acquire),
            Self::SeqCstRelaxed => (SeqCst, Relaxed),
            Self::SeqCstAcquire => (SeqCst, Acquire),
            Self::SeqCst => (SeqCst, SeqCst),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicRmwOperation {
    Exchange,
    Add,
    Subtract,
    And,
    Or,
    Xor,
}

impl AtomicRmwOperation {
    pub const fn accepts(self, kind: AtomicValueKind) -> bool {
        matches!(
            (self, kind),
            (Self::Exchange, _)
                | (_, AtomicValueKind::Int | AtomicValueKind::Long)
                | (Self::And | Self::Or | Self::Xor, AtomicValueKind::Boolean)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicCompareExchangeResult {
    ObservedValue,
    Success,
}
