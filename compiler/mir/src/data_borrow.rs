/// Exact backing object used by a scoped data borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowDataSource {
    Array(crate::ClassId),
    String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBorrowOperationKind {
    PushPinFrame,
    PopPinFrame,
    DataPointer(BorrowDataSource),
    Length(BorrowDataSource),
}

impl DataBorrowOperationKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::PushPinFrame => "PushPinFrame",
            Self::PopPinFrame => "PopPinFrame",
            Self::DataPointer(_) => "BorrowDataPointer",
            Self::Length(_) => "BorrowDataLength",
        }
    }
}

/// All operations are NoGC. The ordinary closure call and its cleanup are
/// separate control-flow operations; the frame resides on the caller stack.
#[derive(Debug, Clone)]
pub struct DataBorrowOperation<E> {
    pub kind: DataBorrowOperationKind,
    pub operand: Box<E>,
}
