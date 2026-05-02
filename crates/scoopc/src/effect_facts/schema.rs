/// `StepSchema` 的稳定 identity。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepSchemaId(u32);

impl StepSchemaId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// continuation schema 的稳定 identity。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContinuationSchemaId(u32);

impl ContinuationSchemaId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// `Step_F` 的 schema 外壳；具体字段在 P4-T02 落地。
#[derive(Debug, Clone, Default)]
pub struct StepSchema {}

/// continuation schema 外壳；具体字段在 P4-T02 落地。
#[derive(Debug, Clone, Default)]
pub struct ContinuationSchema {}
