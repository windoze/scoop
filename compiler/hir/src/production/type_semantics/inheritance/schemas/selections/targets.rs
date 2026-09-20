use super::*;

impl Projection<'_, '_> {
    pub(super) fn method(&self, function: FunctionId) -> Result<Method, Error> {
        self.export.functions[function]
            .method
            .ok_or_else(|| self.invalid("selected method has no dispatch metadata"))
    }

    pub(super) fn callable(
        &self,
        function: FunctionId,
    ) -> Result<InheritanceCallableDeclarationV1, Error> {
        if !matches!(
            self.export.functions[function].genericity,
            FunctionGenericity::Plain
        ) {
            return Err(Error::GenericOdrRequired(self.owner));
        }
        if matches!(
            self.export.functions[function].kind,
            FunctionKind::Extern(_)
        ) {
            return Err(self.invalid("extern function cannot implement a source dispatch slot"));
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
                Err(Error::GenericOdrRequired(self.owner))
            }
            HirFunctionIdentity::LexicalGenerated(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => Err(self
                .invalid("generated callable cannot stand in for a source dispatch declaration")),
        }
    }

    pub(super) fn interface_selection(
        &self,
        member: InterfaceMethodId,
    ) -> Result<Selection, Error> {
        let declaration = &self.export.interface_methods[member];
        match declaration.implementation {
            InterfaceMemberImplementation::Body => Ok(Selection::InterfaceDefault(
                self.callable(declaration.function)?,
            )),
            InterfaceMemberImplementation::AbstractSlot => Ok(Selection::Abstract),
        }
    }

    pub(super) fn target(&self, application: MethodApplicationId) -> Result<Selection, Error> {
        let application = &self.export.method_applications[application];
        let ty = match application.owner {
            MethodOwnerApplication::Class(id) => self.export.class_applications[id].canonical_type,
            MethodOwnerApplication::Struct(id) => {
                self.export.struct_applications[id].canonical_type
            }
            MethodOwnerApplication::Enum(id) => self.export.enum_applications[id].canonical_type,
            MethodOwnerApplication::Interface(id) => {
                self.export.interface_applications[id].canonical_type
            }
            MethodOwnerApplication::Object(id) => self.export.object_types[id].canonical_type,
        };
        let owner = exact(self.export, ty)?;
        let method = self.method(application.function)?;
        if exact(self.export, method.owner)? != owner {
            return Err(
                self.invalid("selected method application disagrees with its declaration owner")
            );
        }
        if method.modifier == MethodModifier::Abstract {
            return Err(self.invalid("concrete interface target selects an abstract method"));
        }
        let callable = self.callable(application.function)?;
        match application.owner {
            MethodOwnerApplication::Interface(id) => {
                let MethodDispatch::Interface(member) = method.dispatch else {
                    return Err(
                        self.invalid("interface default target has no interface member identity")
                    );
                };
                let member = &self.export.interface_methods[member];
                if member.owner != self.export.interface_applications[id].template
                    || member.function != application.function
                    || member.implementation != InterfaceMemberImplementation::Body
                {
                    return Err(
                        self.invalid("interface default target disagrees with its source member")
                    );
                }
                Ok(Selection::InterfaceDefault(callable))
            }
            MethodOwnerApplication::Class(_)
            | MethodOwnerApplication::Struct(_)
            | MethodOwnerApplication::Enum(_)
            | MethodOwnerApplication::Object(_) => Ok(Selection::Concrete(callable)),
        }
    }
}
