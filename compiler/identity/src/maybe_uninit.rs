/// Typed storage operations shared by IRs. The operand is evaluated exactly once.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MaybeUninitOperation<Value> {
    Uninit,
    Initialized(Value),
    AssumeInit(Value),
}

impl<Value> MaybeUninitOperation<Value> {
    pub fn try_map<'a, Other, E>(
        &'a self,
        mut map: impl FnMut(&'a Value) -> Result<Other, E>,
    ) -> Result<MaybeUninitOperation<Other>, E> {
        Ok(match self {
            Self::Uninit => MaybeUninitOperation::Uninit,
            Self::Initialized(value) => MaybeUninitOperation::Initialized(map(value)?),
            Self::AssumeInit(value) => MaybeUninitOperation::AssumeInit(map(value)?),
        })
    }

    pub fn try_into_map<Other, E>(
        self,
        mut map: impl FnMut(Value) -> Result<Other, E>,
    ) -> Result<MaybeUninitOperation<Other>, E> {
        Ok(match self {
            Self::Uninit => MaybeUninitOperation::Uninit,
            Self::Initialized(value) => MaybeUninitOperation::Initialized(map(value)?),
            Self::AssumeInit(value) => MaybeUninitOperation::AssumeInit(map(value)?),
        })
    }
    pub fn operand(&self) -> Option<&Value> {
        match self {
            Self::Uninit => None,
            Self::Initialized(value) | Self::AssumeInit(value) => Some(value),
        }
    }

    pub fn operand_mut(&mut self) -> Option<&mut Value> {
        match self {
            Self::Uninit => None,
            Self::Initialized(value) | Self::AssumeInit(value) => Some(value),
        }
    }

    pub fn map<'a, Other>(
        &'a self,
        mut map: impl FnMut(&'a Value) -> Other,
    ) -> MaybeUninitOperation<Other> {
        match self {
            Self::Uninit => MaybeUninitOperation::Uninit,
            Self::Initialized(value) => MaybeUninitOperation::Initialized(map(value)),
            Self::AssumeInit(value) => MaybeUninitOperation::AssumeInit(map(value)),
        }
    }
}

mod wire;
