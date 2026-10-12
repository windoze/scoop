use super::*;

impl Lowerer {
    pub(super) fn integer_interface_implementations(
        &mut self,
        owner: Owner,
        span: ast::Span,
    ) -> Vec<hir::InterfaceMethodImplementation> {
        let Owner::Enum(id) = owner else {
            return Vec::new();
        };
        let Some(kind) = self.enums[id].representation.integer_kind() else {
            return Vec::new();
        };
        let kind = hir::IntrinsicTypeKind::Integer(kind);
        let interfaces = self.intrinsic_type_interfaces(kind);
        if self.imported_intrinsic_types.contains_key(&kind) {
            for interface in interfaces {
                if let Err(error) = self.require_imported_bound_interface(interface) {
                    self.error(span, error.diagnostic("integer parent interface"));
                    return Vec::new();
                }
            }
            return self.imported_intrinsic_types[&kind]
                .interface_implementations
                .iter()
                .flat_map(|conformance| conformance.methods.iter().cloned())
                .collect();
        }
        match self.intrinsic_type_owners.get(&kind) {
            Some((crate::IntrinsicTypeOwner::Struct(id), _)) => self.structs[*id]
                .interface_implementations
                .iter()
                .flat_map(|conformance| conformance.methods.iter().cloned())
                .collect(),
            _ => Vec::new(),
        }
    }
}
