use super::*;

impl MemberIndex {
    pub(super) fn conformances(
        &mut self,
        lowerer: &mut Lowerer,
        owner: Owner,
        check: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        let conformances = match owner {
            Owner::Class(id) => &lowerer.classes[id].interface_implementations,
            Owner::Struct(id) => &lowerer.structs[id].interface_implementations,
            Owner::Enum(id) => &lowerer.enums[id].interface_implementations,
            Owner::Object(id) => {
                &lowerer.classes[lowerer.objects[id].backing_class].interface_implementations
            }
            Owner::Interface(_) => return Ok(()),
        }
        .clone();
        for conformance in conformances {
            for implementation in conformance.methods {
                if check() {
                    return Err(EditingCancelled);
                }
                let requirement = match implementation.member {
                    hir::InterfaceMethodReference::Local(id) => {
                        self.current_member(lowerer, lowerer.interface_method_entities[id].function)
                    }
                    hir::InterfaceMethodReference::Imported { owner, slot } => {
                        let owner = lowerer
                            .imported_nominal_owner(owner)
                            .expect("checked imported interface");
                        let declaration = lowerer
                            .dependencies
                            .as_ref()
                            .expect("interface catalog")
                            .callable_for_slot(owner, slot)
                            .expect("checked interface slot")
                            .expect("declared slot");
                        self.include_imported(
                            lowerer,
                            declaration.interface().declaration(),
                            check,
                        )?
                    }
                };
                let Some(requirement) = requirement else {
                    continue;
                };
                let target = match implementation.target {
                    hir::InterfaceImplementationTarget::Method(id)
                    | hir::InterfaceImplementationTarget::Abstract(id) => {
                        let function = lowerer.method_applications[id].function;
                        self.source_function(lowerer, function, check)?;
                        self.current_member(lowerer, function)
                    }
                    hir::InterfaceImplementationTarget::Imported(id)
                    | hir::InterfaceImplementationTarget::ImportedAbstract(id) => self
                        .include_imported(
                            lowerer,
                            lowerer.imported_dependency_callables[id]
                                .reference()
                                .declaration(),
                            check,
                        )?,
                    hir::InterfaceImplementationTarget::ImportedTemplate(id)
                    | hir::InterfaceImplementationTarget::ImportedAbstractTemplate(id) => {
                        let template = lowerer.imported_generic_applications[id].template;
                        let declaration = lowerer.imported_generic_templates[template]
                            .declaration
                            .declaration();
                        self.include_imported(lowerer, declaration, check)?
                    }
                };
                if let Some(target) = target {
                    self.relation(requirement, target);
                }
            }
        }
        Ok(())
    }
}
