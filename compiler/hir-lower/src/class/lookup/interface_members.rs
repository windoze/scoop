use super::*;

impl Lowerer {
    pub(crate) fn collect_selected_interface_members(
        &mut self,
        receiver: TypeId,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        let (implementations, arguments) = match self.types[receiver].clone() {
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (
                    self.class_definition(application.template)
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (
                    self.struct_definition(application.template)
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (
                    self.enum_definition(application.template)
                        .interface_implementations
                        .clone(),
                    application.arguments.clone(),
                )
            }
            ty => {
                let kind = match ty {
                    Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                    Type::Unit => hir::IntrinsicTypeKind::Unit,
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
        let applicable = implementations
            .into_iter()
            .filter(|implementation| {
                let Some(mut condition) = implementation.condition.clone() else {
                    return true;
                };
                condition.arguments = condition
                    .arguments
                    .into_iter()
                    .map(|argument| self.instantiate_ty(argument, &arguments))
                    .collect();
                self.conditional_application_holds(&condition)
            })
            .flat_map(|implementation| implementation.methods)
            .collect::<Vec<_>>();
        let mut selected = Vec::<crate::CallableCandidate>::new();
        for method in applicable {
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
            if selected.iter().any(|existing| {
                existing.function == candidate.function && existing.owner == candidate.owner
            }) {
                continue;
            }
            let candidate_owner = self.interface_applications[owner].canonical_type;
            let mut shadowed = false;
            selected.retain(|existing| {
                if self.source_function_name(existing.function)
                    != self.source_function_name(candidate.function)
                    || self.function_accessor_kind(existing.function)
                        != self.function_accessor_kind(candidate.function)
                    || !self.same_applied_method_signature(existing, &candidate)
                {
                    return true;
                }
                let crate::CallableCandidateOwner::Method(existing_owner) = existing.owner else {
                    unreachable!("selected interface defaults are methods")
                };
                let existing_owner = self.method_owner_type(existing_owner);
                if self.is_subtype(existing_owner, candidate_owner) {
                    shadowed = true;
                    true
                } else {
                    !self.is_subtype(candidate_owner, existing_owner)
                }
            });
            if !shadowed {
                selected.push(candidate);
            }
        }
        out.extend(selected.into_iter().map(|candidate| (candidate, 0, 0)));
    }
}
