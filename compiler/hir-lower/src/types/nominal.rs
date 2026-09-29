//! Complete application keys over declaration storage during HIR lowering.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NominalTemplate {
    Imported(hir::SourceNominalId),
    Struct(hir::StructId),
    Class(hir::ClassId),
    Enum(hir::EnumId),
    Interface(hir::InterfaceId),
}

#[derive(Debug, Clone)]
pub(crate) struct NominalApplication {
    pub(crate) ty: TypeId,
    pub(crate) template: NominalTemplate,
    pub(crate) arguments: Vec<TypeId>,
}

impl Lowerer {
    pub(crate) fn nominal_application(&self, ty: TypeId) -> Option<NominalApplication> {
        if let Some((declaration, arguments)) = self.types[ty].imported_nominal_application() {
            return Some(NominalApplication {
                ty,
                template: NominalTemplate::Imported(declaration.owner()),
                arguments: arguments.to_vec(),
            });
        }
        let (template, arguments) = match self.types[ty] {
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (
                    NominalTemplate::Struct(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (
                    NominalTemplate::Class(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (
                    NominalTemplate::Enum(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Interface(application) => {
                let application = &self.interface_applications[application];
                (
                    NominalTemplate::Interface(application.template),
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
}
