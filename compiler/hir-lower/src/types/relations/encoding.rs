use super::*;

impl Lowerer {
    pub(crate) fn encoding_method_applies(&mut self, candidate: &crate::CallableCandidate) -> bool {
        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
            return true;
        };
        let ty = match owner {
            hir::MethodOwnerApplication::Class(id) => self.class_applications[id].canonical_type,
            hir::MethodOwnerApplication::Enum(id) => self.enum_applications[id].canonical_type,
            _ => return true,
        };
        let kind = self.types[ty].clone();
        let Some((encoding, _)) = self.element_encoding_for_type(&kind) else {
            return true;
        };
        let is_encoding = encoding.implementation.methods.iter().any(|method| {
            matches!(method.target, hir::InterfaceImplementationTarget::Method(application)
                if self.method_applications[application].function == candidate.function)
        });
        !is_encoding || self.element_encoding_parent(&kind).is_some()
    }

    pub(crate) fn element_encoding_for_type(
        &self,
        ty: &Type,
    ) -> Option<(&hir::ElementEncoding, &[TypeId])> {
        match *ty {
            Type::Class(application) => {
                let application = &self.class_applications[application];
                self.class_definition(application.template)
                    .element_encoding
                    .as_ref()
                    .map(|encoding| (encoding, application.arguments.as_slice()))
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                self.enum_definition(application.template)
                    .element_encoding
                    .as_ref()
                    .map(|encoding| (encoding, application.arguments.as_slice()))
            }
            _ => None,
        }
    }

    pub(crate) fn element_encoding_parent(&mut self, ty: &Type) -> Option<TypeId> {
        if let Type::Tuple(elements) = ty {
            if elements.is_empty() {
                return None;
            }
            let interface = self.core_coding_type("Encodable")?;
            return elements
                .iter()
                .all(|element| self.is_subtype(*element, interface))
                .then_some(interface);
        }
        let (encoding, arguments) = self.element_encoding_for_type(ty)?;
        let (element, interface, arguments) = (
            encoding.element,
            encoding.implementation.interface,
            arguments.to_vec(),
        );
        let element = self.instantiate_ty(element, &arguments);
        let interface = self.instantiate_ty(interface, &arguments);
        self.is_subtype(element, interface).then_some(interface)
    }
}
