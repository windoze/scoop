//! Concrete HIR conformance for ordinary and structural boxed values.

use super::*;

impl Lowerer {
    pub(crate) fn value_interfaces(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Vec<mir::InterfaceId> {
        let declared = match self.value_struct_source(module, payload) {
            Some(hir_id) => module.structs[hir_id].interfaces.clone(),
            None => match payload {
                mir::Type::Enum(mir_id, _) => {
                    let hir_id = self.enums.hir_ids[mir_id];
                    module.enums[hir_id].interfaces.clone()
                }
                mir::Type::Tuple(_) => self
                    .tuple_conformance(module, payload)
                    .map(|implementation| {
                        module.interfaces[implementation.interface].canonical_type
                    })
                    .into_iter()
                    .collect(),
                _ => return Vec::new(),
            },
        };
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        declared
            .into_iter()
            .map(|ty| {
                let lowered = types.lower(
                    ty,
                    &mut self.source_exact_types,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let mir::Type::Interface(interface) = lowered else {
                    unreachable!()
                };
                interface
            })
            .collect()
    }

    pub(crate) fn value_interface_implementations<'a>(
        &self,
        module: &'a hir::Module,
        payload: &mir::Type,
    ) -> &'a [hir::InterfaceImplementation] {
        match self.value_struct_source(module, payload) {
            Some(source) => &module.structs[source].interface_implementations,
            None => match payload {
                mir::Type::Enum(id, _) => {
                    &module.enums[self.enums.hir_ids[id]].interface_implementations
                }
                mir::Type::Tuple(_) => self
                    .tuple_conformance(module, payload)
                    .map(std::slice::from_ref)
                    .unwrap_or_default(),
                _ => unreachable!("only value types receive boxed interface adapters"),
            },
        }
    }

    fn tuple_conformance<'a>(
        &self,
        module: &'a hir::Module,
        payload: &mir::Type,
    ) -> Option<&'a hir::InterfaceImplementation> {
        let exact = self.source_exact_types.get(payload)?.identity_record().id();
        let owner = module.exact_type_identities.type_for_identity(exact)?;
        module.tuple_interface_implementations.get(&owner)
    }

    /// Exact source declaration for a MIR struct-like payload. Primitive
    /// representations use the typed relation emitted by HIR; ordinary
    /// struct instances use the mandatory MIR→HIR provenance map.
    pub(crate) fn value_struct_source(
        &self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Option<hir::StructId> {
        match payload {
            mir::Type::Unit | mir::Type::Integer(_) | mir::Type::Boolean => {
                let exact = self
                    .source_exact_types
                    .get(payload)
                    .expect("boxed payloads retain their HIR exact type")
                    .identity_record()
                    .id();
                Some(
                    module
                        .structs
                        .iter()
                        .find_map(|(id, declaration)| {
                            (module.exact_type_identities[declaration.canonical_type].id() == exact)
                                .then_some(id)
                        })
                        .expect("primitive HIR types retain their actual struct declarations"),
                )
            }
            mir::Type::Struct(mir_id) => Some(self.structs.hir_ids[mir_id]),
            _ => None,
        }
    }
}
