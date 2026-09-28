use super::*;

impl Lowerer {
    pub(super) fn collect_selected_interface_members(
        &mut self,
        receiver: TypeId,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        let (implementations, arguments) = match self.types[receiver].clone() {
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (
                    self.classes[application.template]
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (
                    self.structs[application.template]
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (
                    self.enums[application.template]
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            ty => {
                let kind = match ty {
                    Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                    Type::Boolean => hir::IntrinsicTypeKind::Boolean,
                    Type::String => hir::IntrinsicTypeKind::String,
                    _ => return,
                };
                let Some(&(owner, _)) = self.intrinsic_type_owners.get(&kind) else {
                    return;
                };
                let implementations = match owner {
                    crate::IntrinsicTypeOwner::Struct(owner) => {
                        self.structs[owner].interface_implementations.clone()
                    }
                    crate::IntrinsicTypeOwner::Class(owner) => {
                        self.classes[owner].interface_implementations.clone()
                    }
                };
                (implementations, Vec::new())
            }
        };
        for method in implementations
            .into_iter()
            .flat_map(|implementation| implementation.methods)
        {
            let application = match method.target {
                hir::InterfaceImplementationTarget::Method(application)
                | hir::InterfaceImplementationTarget::Abstract(application) => application,
                hir::InterfaceImplementationTarget::Imported(_)
                | hir::InterfaceImplementationTarget::ImportedTemplate(_)
                | hir::InterfaceImplementationTarget::ImportedAbstractTemplate(_)
                | hir::InterfaceImplementationTarget::ImportedAbstract(_) => continue,
            };
            let application = self.method_applications[application].clone();
            let hir::MethodOwnerApplication::Interface(owner) = application.owner else {
                continue;
            };
            let owner = self.interface_applications[owner].canonical_type;
            let owner = self.instantiate_ty(owner, &arguments);
            let Type::Interface(owner) = self.types[owner] else {
                unreachable!("a selected interface member retains its actual application");
            };
            let candidate = crate::CallableCandidate::method(
                application.function,
                hir::MethodOwnerApplication::Interface(owner),
            );
            if !out.iter().any(|(existing, _, _)| {
                existing.function == candidate.function && existing.owner == candidate.owner
            }) {
                out.push((candidate, 0, 0));
            }
        }
    }
}
