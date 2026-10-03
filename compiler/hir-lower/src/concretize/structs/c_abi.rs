//! C boundary projection from the actual core declaration.

use super::*;

impl Concretizer<'_> {
    pub(super) fn struct_c_abi(&self, source: export::StructId) -> concrete::StructCAbi {
        let export::CoreProtocols::Defined(protocols) = &self.source.core_protocols else {
            return concrete::StructCAbi::SourceRepresentation;
        };
        if source != protocols.ffi.pinned_ptr && source != protocols.ffi.gc_handle {
            return concrete::StructCAbi::SourceRepresentation;
        }
        let field = export::StructFieldRef::checked(&self.source.structs, source, 0)
            .expect("the validated scalar C projection has exactly one source field");
        concrete::StructCAbi::UInt64Field {
            field: self.source.field_identities[field].id(),
        }
    }
}
