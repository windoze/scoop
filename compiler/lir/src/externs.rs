use super::*;

#[derive(Debug)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub calling_convention: CallingConvention,
    pub params: Vec<LirType>,
    pub return_type: LirType,
    pub kind: ExternFunctionKind,
}

#[derive(Debug)]
pub struct ExternFunctionDeclaration {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub calling_convention: CallingConvention,
    pub params: Vec<LirType>,
    pub return_type: LirType,
}

#[derive(Debug)]
pub struct CExternFunction {
    pub declaration: ExternFunctionDeclaration,
    pub bridge_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CType,
}

#[derive(Debug)]
pub struct ScoopExternFunction {
    pub declaration: ExternFunctionDeclaration,
    pub gc_effect: GcEffect,
}

/// ABI-refined identities into `Module::extern_functions`. The shared arena
/// remains the declaration store, while call destinations can only carry the
/// identity family admitted by their native transition protocol. Their private
/// constructors prevent downstream consumers from reclassifying a plain id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CExternFunctionRef(ExternFunctionId);

impl CExternFunctionRef {
    pub fn declaration(self) -> ExternFunctionId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScoopExternFunctionRef(ExternFunctionId);

impl ScoopExternFunctionRef {
    pub fn declaration(self) -> ExternFunctionId {
        self.0
    }
}

/// Declaration store and the sole producer of ABI-refined extern identities.
#[derive(Debug, Default)]
pub struct ExternFunctions {
    declarations: Arena<ExternFunction>,
}

impl ExternFunctions {
    pub fn alloc_c(&mut self, function: CExternFunction) -> CExternFunctionRef {
        let declaration = function.declaration;
        CExternFunctionRef(self.declarations.alloc(ExternFunction {
            source_name: declaration.source_name,
            native_symbol: declaration.native_symbol,
            library: declaration.library,
            calling_convention: declaration.calling_convention,
            params: declaration.params,
            return_type: declaration.return_type,
            kind: ExternFunctionKind::C {
                bridge_symbol: function.bridge_symbol,
                params: function.params,
                return_type: function.return_type,
            },
        }))
    }

    pub fn alloc_scoop(&mut self, function: ScoopExternFunction) -> ScoopExternFunctionRef {
        let declaration = function.declaration;
        ScoopExternFunctionRef(self.declarations.alloc(ExternFunction {
            source_name: declaration.source_name,
            native_symbol: declaration.native_symbol,
            library: declaration.library,
            calling_convention: declaration.calling_convention,
            params: declaration.params,
            return_type: declaration.return_type,
            kind: ExternFunctionKind::Scoop {
                gc_effect: function.gc_effect,
            },
        }))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ExternFunctionId, &ExternFunction)> {
        self.declarations.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

impl std::ops::Index<ExternFunctionId> for ExternFunctions {
    type Output = ExternFunction;

    fn index(&self, index: ExternFunctionId) -> &Self::Output {
        &self.declarations[index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug)]
pub enum ExternFunctionKind {
    C {
        bridge_symbol: String,
        params: Vec<CType>,
        return_type: CType,
    },
    Scoop {
        gc_effect: GcEffect,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Unit,
    Int,
    UInt,
    Boolean,
    Pointer,
    FunctionPointer {
        params: Vec<CType>,
        return_type: Box<CType>,
    },
    Struct(StructDefId),
}
