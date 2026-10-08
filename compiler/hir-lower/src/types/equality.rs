//! Identity-based access to the ordinary core equality protocol.

use super::*;

impl Lowerer {
    pub(crate) fn equality_parameter_types(&mut self, receiver: TypeId) -> Vec<TypeId> {
        self.type_interfaces(receiver)
            .into_iter()
            .filter_map(|interface| {
                if !self.is_equality_interface(interface) {
                    return None;
                }
                let Type::Interface(application) = self.types[interface] else {
                    unreachable!()
                };
                Some(self.interface_applications[application].arguments[0])
            })
            .collect()
    }

    pub(crate) fn equality_interface_identity(&self) -> Option<hir::SourceNominalId> {
        match &self.core {
            crate::CoreLoweringAuthority::Defined => self.equality_core.map(|core| {
                self.nominal_identity(crate::Owner::Interface(core.interface))
                    .declaration_id()
            }),
            crate::CoreLoweringAuthority::Imported(core) => Some(
                hir::SourceNominalId::GenericTemplate(core.equality().interface().persistent()),
            ),
        }
    }

    pub(crate) fn is_equality_interface(&self, ty: TypeId) -> bool {
        matches!(self.types[ty], Type::Interface(application)
            if Some(self.interface_applications[application].template) == self.equality_interface_identity())
    }

    pub(crate) fn equality_interface(&mut self, argument: TypeId) -> Option<TypeId> {
        let owner = self.equality_interface_identity()?;
        match &self.core {
            crate::CoreLoweringAuthority::Defined => {
                let id = self
                    .source_interface_id(owner)
                    .expect("the defining core owns Equality");
                Some(self.source_interface_type(id, vec![argument]))
            }
            crate::CoreLoweringAuthority::Imported(_) => Some(
                self.imported_nominal_application(owner, vec![argument])
                    .expect("the imported core protocol retains its Equality declaration"),
            ),
        }
    }
}
