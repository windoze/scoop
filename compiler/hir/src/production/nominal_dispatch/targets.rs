use super::*;

impl Projection<'_> {
    pub(super) fn imported_selection(
        &self,
        id: ImportedDependencyCallableUseId,
    ) -> Result<Selection, Error> {
        let target = self.export.imported_dependency_callables[id]
            .reference()
            .declaration();
        self.export
            .types
            .iter()
            .filter_map(|(_, ty)| match ty {
                Type::ImportedClass(class) => Some(&class.declaration),
                Type::ImportedInterface(interface) => Some(&interface.declaration),
                Type::ImportedStruct(structure) => Some(&structure.declaration),
                Type::ImportedEnum(enumeration) => Some(&enumeration.declaration),
                _ => None,
            })
            .flat_map(|declaration| {
                declaration
                    .interface
                    .declaration_details()
                    .dispatch_selections()
                    .records()
            })
            .find(|selection| selection.callable_target() == target)
            .map(NominalDispatchSelectionV1::selection)
            .ok_or_else(|| invalid("imported dispatch target has no actual declaration selection"))
    }

    pub(super) fn imported_callable(
        &self,
        id: ImportedDependencyCallableUseId,
    ) -> Result<InheritanceCallableDeclarationV1, Error> {
        self.imported_selection(id).map(Selection::declaration)
    }

    pub(super) fn method(&self, function: FunctionId) -> Result<Method, Error> {
        self.export.functions[function]
            .method
            .ok_or_else(|| invalid("selected method has no dispatch metadata"))
    }

    pub(super) fn callable(
        &self,
        function: FunctionId,
    ) -> Result<InheritanceCallableDeclarationV1, Error> {
        if !matches!(
            self.export.functions[function].genericity,
            FunctionGenericity::Plain | FunctionGenericity::OwnerParameterizedMethod { .. }
        ) {
            return Err(invalid(
                "a dispatch declaration cannot have its own generic parameters",
            ));
        }
        if matches!(
            self.export.functions[function].kind,
            FunctionKind::Extern(_)
        ) {
            return Err(invalid(
                "extern function cannot implement a source dispatch slot",
            ));
        }
        match &self.export.function_identities[function] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                Ok(InheritanceCallableDeclarationV1::Function(record.id()))
            }
            HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(id)) => {
                Ok(InheritanceCallableDeclarationV1::Getter(
                    self.export.property_accessor_identities[*id].record().id(),
                ))
            }
            HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(id)) => {
                Ok(InheritanceCallableDeclarationV1::Setter(
                    self.export.property_accessor_identities[*id].record().id(),
                ))
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(_)) => {
                Err(invalid("generic callable cannot implement a dispatch slot"))
            }
            HirFunctionIdentity::LexicalGenerated(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => Err(invalid(
                "generated callable cannot stand in for a source dispatch declaration",
            )),
        }
    }

    pub(super) fn interface_selection(
        &self,
        member: InterfaceMethodReference,
    ) -> Result<Selection, Error> {
        let member = match member {
            InterfaceMethodReference::Local(member) => member,
            InterfaceMethodReference::Imported { owner, slot } => {
                let Type::ImportedInterface(interface) = &self.export.types[owner] else {
                    return Err(invalid("imported slot has no interface owner"));
                };
                let method = interface
                    .methods
                    .iter()
                    .find(|method| method.slot.id() == slot)
                    .ok_or_else(|| invalid("imported interface has no referenced slot"))?;
                let target = match method.declaration.declaration() {
                    scoop_identity::CallableTemplateOrigin::Function(id) => {
                        InheritanceCallableDeclarationV1::Function(id)
                    }
                    scoop_identity::CallableTemplateOrigin::Accessor(id) => {
                        match method.slot.key().role() {
                            scoop_identity::DispatchRole::PropertyGetter => {
                                InheritanceCallableDeclarationV1::Getter(id)
                            }
                            scoop_identity::DispatchRole::PropertySetter => {
                                InheritanceCallableDeclarationV1::Setter(id)
                            }
                            _ => return Err(invalid("interface accessor has no accessor role")),
                        }
                    }
                    _ => return Err(invalid("interface member has no callable declaration")),
                };
                return Ok(
                    if method.declaration.modality() == CallableModalityV1::Abstract {
                        Selection::Abstract(target)
                    } else {
                        Selection::InterfaceDefault(target)
                    },
                );
            }
        };
        let declaration = &self.export.interface_methods[member];
        match declaration.implementation {
            InterfaceMemberImplementation::Body => Ok(Selection::InterfaceDefault(
                self.callable(declaration.function)?,
            )),
            InterfaceMemberImplementation::AbstractSlot => {
                Ok(Selection::Abstract(self.callable(declaration.function)?))
            }
        }
    }

    pub(super) fn target(&self, application: MethodApplicationId) -> Result<Selection, Error> {
        let application = &self.export.method_applications[application];
        let owner = match application.owner {
            MethodOwnerApplication::Class(id) => {
                NominalOwner::Class(self.export.class_applications[id].template)
            }
            MethodOwnerApplication::Struct(id) => {
                NominalOwner::Struct(self.export.struct_applications[id].template)
            }
            MethodOwnerApplication::Enum(id) => {
                NominalOwner::Enum(self.export.enum_applications[id].template)
            }
            MethodOwnerApplication::Interface(id) => {
                NominalOwner::Interface(self.export.interface_applications[id].template)
            }
            MethodOwnerApplication::Object(id) => {
                NominalOwner::Object(self.export.object_types[id].declaration)
            }
        };
        let method = self.method(application.function)?;
        if method.owner != self.declaration_type(owner) {
            return Err(invalid(
                "selected method application disagrees with its declaration owner",
            ));
        }
        if method.modifier == MethodModifier::Abstract {
            return Err(invalid(
                "concrete interface target selects an abstract method",
            ));
        }
        let callable = self.callable(application.function)?;
        match owner {
            NominalOwner::Interface(id) => {
                let MethodDispatch::Interface(member) = method.dispatch else {
                    return Err(invalid(
                        "interface default target has no interface member identity",
                    ));
                };
                let member = &self.export.interface_methods[member];
                if member.owner != id
                    || member.function != application.function
                    || member.implementation != InterfaceMemberImplementation::Body
                {
                    return Err(invalid(
                        "interface default target disagrees with its source member",
                    ));
                }
                Ok(Selection::InterfaceDefault(callable))
            }
            NominalOwner::Class(_)
            | NominalOwner::Struct(_)
            | NominalOwner::Enum(_)
            | NominalOwner::Object(_) => Ok(Selection::Concrete(callable)),
        }
    }

    fn declaration_type(&self, owner: NominalOwner) -> TypeId {
        match owner {
            NominalOwner::Class(id) => {
                self.export.class_applications[self.export.classes[id].self_application]
                    .canonical_type
            }
            NominalOwner::Struct(id) => {
                self.export.struct_applications[self.export.structs[id].self_application]
                    .canonical_type
            }
            NominalOwner::Enum(id) => {
                self.export.enum_applications[self.export.enums[id].self_application].canonical_type
            }
            NominalOwner::Interface(id) => {
                self.export.interface_applications[self.export.interfaces[id].self_application]
                    .canonical_type
            }
            NominalOwner::Object(id) => {
                self.export.object_types[self.export.objects[id].object_type].canonical_type
            }
        }
    }
}
