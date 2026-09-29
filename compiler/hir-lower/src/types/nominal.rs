//! Complete application keys over declaration storage during HIR lowering.

use super::*;
use crate::Owner;

#[derive(Debug, Clone)]
pub(crate) struct NominalApplication {
    pub(crate) ty: TypeId,
    pub(crate) template: hir::SourceNominalId,
    pub(crate) arguments: Vec<TypeId>,
}

impl Lowerer {
    pub(crate) fn nominal_application(&self, ty: TypeId) -> Option<NominalApplication> {
        if let Some((declaration, arguments)) = self.types[ty].imported_nominal_application() {
            return Some(NominalApplication {
                ty,
                template: declaration.owner(),
                arguments: arguments.to_vec(),
            });
        }
        let (template, arguments) = match self.types[ty] {
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (
                    self.nominal_identity(Owner::Struct(application.template))
                        .declaration_id(),
                    application.arguments.clone(),
                )
            }
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (
                    self.nominal_identity(Owner::Class(application.template))
                        .declaration_id(),
                    application.arguments.clone(),
                )
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (
                    self.nominal_identity(Owner::Enum(application.template))
                        .declaration_id(),
                    application.arguments.clone(),
                )
            }
            Type::Interface(application) => {
                let application = &self.interface_applications[application];
                (
                    self.nominal_identity(Owner::Interface(application.template))
                        .declaration_id(),
                    application.arguments.clone(),
                )
            }
            _ => return None,
        };
        Some(NominalApplication {
            ty,
            template,
            arguments,
        })
    }
    pub(crate) fn nominal_template_name(&self, template: hir::SourceNominalId) -> &str {
        if let Some(owner) = self.nominal_owners.get(&template) {
            return match *owner {
                Owner::Struct(id) => &self.structs[id].name,
                Owner::Class(id) => &self.classes[id].name,
                Owner::Enum(id) => &self.enums[id].name,
                Owner::Interface(id) => &self.interfaces[id].name,
                Owner::Object(id) => &self.objects[id].name,
            };
        }
        self.dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(template))
            .expect("a resolved nominal retains its original declaration")
            .name()
    }
}
