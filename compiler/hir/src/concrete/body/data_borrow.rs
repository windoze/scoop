use super::*;

/// The concrete signature selects both the physical source and callback ABI.
#[derive(Debug, Clone)]
pub struct DataBorrow {
    pub source: BorrowDataSource,
    pub function_type: FunctionTypeId,
    pub object: Box<Expr>,
    pub block: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowDataSource {
    String,
    Array(ClassId),
}
