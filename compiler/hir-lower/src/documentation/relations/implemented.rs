use super::*;

impl Lowerer {
    pub(super) fn documentation_implemented_slots(&self, function: hir::FunctionId) -> Vec<Symbol> {
        let Some(owner) = self.function_owner.get(&function) else {
            return Vec::new();
        };
        let implementations = match *owner {
            Owner::Class(id) => &self.classes[id].interface_implementations,
            Owner::Struct(id) => &self.structs[id].interface_implementations,
            Owner::Enum(id) => &self.enums[id].interface_implementations,
            Owner::Object(id) => {
                &self.classes[self.objects[id].backing_class].interface_implementations
            }
            Owner::Interface(_) => return Vec::new(),
        };
        implementations
            .iter()
            .flat_map(|implementation| &implementation.methods)
            .filter_map(|implementation| {
                let application = match implementation.target {
                    hir::InterfaceImplementationTarget::Method(id)
                    | hir::InterfaceImplementationTarget::Abstract(id) => id,
                    _ => return None,
                };
                if self.method_applications[application].function != function {
                    return None;
                }
                Some(match implementation.member {
                    hir::InterfaceMethodReference::Local(id) => {
                        Symbol::Current(self.documentation_function_target(
                            self.interface_method_entities[id].function,
                        ))
                    }
                    hir::InterfaceMethodReference::Imported { owner, slot } => {
                        let owner = self
                            .imported_nominal_owner(owner)
                            .expect("imported interface owner");
                        let candidate = self
                            .dependencies
                            .as_ref()
                            .unwrap()
                            .callable_for_slot(owner, slot)
                            .expect("checked imported slot")
                            .expect("imported slot declaration");
                        Symbol::Imported(hir::AnnotationTargetV1::Callable(
                            hir::AnnotationCallableV1::Declaration(
                                candidate.interface().declaration(),
                            ),
                        ))
                    }
                })
            })
            .collect()
    }
}
