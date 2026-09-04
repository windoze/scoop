use super::*;

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub access: DeclarationAccess,
    pub ty: TypeId,
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

/// A typed, GC-free initializer accepted for local global storage.
#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPtr,
    NullFunPtr,
    Struct {
        application: StructApplicationId,
        fields: Vec<ConstantValue>,
    },
}
