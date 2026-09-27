use super::*;

impl Lowerer {
    pub(super) fn collect_selected_interface_defaults(
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
            _ => return,
        };
        for method in implementations
            .into_iter()
            .flat_map(|implementation| implementation.methods)
        {
            let hir::InterfaceImplementationTarget::Method(application) = method.target else {
                continue;
            };
            let application = self.method_applications[application].clone();
            let hir::MethodOwnerApplication::Interface(owner) = application.owner else {
                continue;
            };
            let owner = self.interface_applications[owner].canonical_type;
            let owner = self.instantiate_ty(owner, &arguments);
            let Type::Interface(owner) = self.types[owner] else {
                unreachable!("a local default retains its actual interface application");
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
