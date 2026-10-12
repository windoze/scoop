use super::*;
use crate::call_resolution::candidates::ValueParameterCalling;
use crate::{FnParamCalling, FnVarargOmission};
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn interface_forwarder_signature(
        &mut self,
        member: &InterfaceMemberInstance,
        span: ast::Span,
    ) -> FnSig {
        let mut signature = match member.member {
            hir::InterfaceMethodReference::Local(method) => {
                let Type::Interface(application) = self.types[member.owner] else {
                    unreachable!("a slot has an interface application")
                };
                let arguments = self.interface_applications[application].arguments.clone();
                self.instantiated_signature(
                    self.interface_method_entities[method].function,
                    &arguments,
                    &[],
                )
            }
            hir::InterfaceMethodReference::Imported { .. } => {
                let declaration = self.delegated_imported_member(member);
                let effects = declaration.interface().effects();
                let parameters = self
                    .imported_parameter_views(&declaration, member.signature.parameters.clone());
                FnSig {
                    result_pending: false,
                    parameter_pending: false,
                    context_parameters: declaration
                        .interface()
                        .context_parameters()
                        .iter()
                        .zip(&member.signature.context_parameters)
                        .map(|(parameter, ty)| hir::ContextParameter {
                            label: match parameter.name().as_str() {
                                "_" => hir::ContextParameterLabel::Unnamed,
                                name => hir::ContextParameterLabel::Named(name.into()),
                            },
                            ty: *ty,
                            span,
                        })
                        .collect(),
                    is_suspend: member.signature.is_suspend,
                    modifiers: effects.callable_modifiers(),
                    attributes: effects.function_attributes(),
                    owner_type_param_count: 0,
                    type_params: Vec::new(),
                    params: parameters
                        .into_iter()
                        .map(|parameter| FnParam {
                            name: ast::Ident {
                                text: parameter.name,
                                span,
                            },
                            ty: parameter.ty,
                            calling: match parameter.calling {
                                ValueParameterCalling::Required
                                | ValueParameterCalling::Default(_) => FnParamCalling::Required,
                                ValueParameterCalling::Vararg { element_type, .. } => {
                                    FnParamCalling::Vararg {
                                        element_ty: element_type,
                                        omission: FnVarargOmission::EmptyArray,
                                    }
                                }
                            },
                        })
                        .collect(),
                    return_ty: member.signature.result,
                }
            }
        };
        for parameter in &mut signature.params {
            parameter.name.span = span;
            parameter.calling = match parameter.calling {
                FnParamCalling::Required | FnParamCalling::Default { .. } => {
                    FnParamCalling::Required
                }
                FnParamCalling::Vararg { element_ty, .. } => FnParamCalling::Vararg {
                    element_ty,
                    omission: FnVarargOmission::EmptyArray,
                },
            };
        }
        signature
    }

    pub(super) fn delegated_imported_member(
        &self,
        member: &InterfaceMemberInstance,
    ) -> hir::ImportedCallableDeclaration {
        let hir::InterfaceMethodReference::Imported { owner, slot } = member.member else {
            unreachable!("the slot comes from a dependency")
        };
        let interface = self
            .dependency_interface_definition(owner)
            .expect("a loaded slot retains its interface");
        self.dependencies
            .as_ref()
            .expect("a loaded slot has a catalog")
            .callable_for_slot(interface.declaration.owner(), slot)
            .expect("a loaded interface has valid slots")
            .expect("a slot retains its callable declaration")
    }

    pub(crate) fn infer_interface_forwarder_signature(
        &mut self,
        function: FunctionId,
    ) -> Option<TypeId> {
        let signature = &self.signatures[&function];
        if !signature.result_pending && !signature.parameter_pending {
            return Some(signature.return_ty);
        }
        if !self.declaration_inference.ready {
            return Some(self.signatures[&function].return_ty);
        }
        let forwarder = self.interface_forwarders[&function].clone();
        if let hir::InterfaceMethodReference::Local(method) = forwarder.member.member {
            self.infer_function_result(self.interface_method_entities[method].function)?;
        }
        let mut signature =
            self.interface_forwarder_signature(&forwarder.member, self.functions[function].span);
        let parameters = self.owner_type_params(self.function_owner[&function]);
        signature.owner_type_param_count = parameters.len();
        signature.type_params = parameters;
        self.functions[function].return_ty = signature.return_ty;
        if let ForwardingRole::Getter(property) = forwarder.role {
            self.properties[property].ty = signature.return_ty;
        }
        let result = signature.return_ty;
        self.signatures.insert(function, signature);
        if let ForwardingRole::Getter(property) = forwarder.role
            && let Some(setter) = self.properties[property].capability.setter()
            && let hir::PropertyAccessorImplementation::Body(setter) =
                self.property_setters[setter].implementation
        {
            self.infer_interface_forwarder_signature(setter)?;
        }
        Some(result)
    }

    pub(crate) fn forwarded_property_getter(
        &self,
        property: hir::PropertyId,
    ) -> Option<FunctionId> {
        let getter = self.properties[property].capability.getter();
        match self.property_getters[getter].implementation {
            hir::PropertyAccessorImplementation::Body(function)
                if self.interface_forwarders.contains_key(&function) =>
            {
                Some(function)
            }
            _ => None,
        }
    }
}
