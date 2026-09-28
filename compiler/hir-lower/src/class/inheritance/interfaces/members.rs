use super::*;
use crate::imports::lookup::calls::wire_operator;

/// The signature used for override and conformance matching. Default argument
/// expressions stay on the referenced declaration, outside signature equality.
#[derive(Clone)]
pub(in crate::class) struct InterfaceSignature {
    pub name: String,
    pub parameters: Vec<TypeId>,
    pub result: TypeId,
    pub is_suspend: bool,
    pub safety: hir::CallableSafetyV1,
    pub gc_effect: scoop_identity::GcEffect,
    pub operator: hir::CallableOperatorRoleV1,
    pub infix: hir::CallableInfixV1,
}

#[derive(Clone)]
pub(in crate::class) struct InterfaceMemberInstance {
    pub member: hir::InterfaceMethodReference,
    pub owner: TypeId,
    pub signature: InterfaceSignature,
    pub implementation: hir::InterfaceMemberImplementation,
    pub mutable_property: Option<String>,
}

impl InterfaceSignature {
    pub fn local(name: &str, signature: &crate::FnSig) -> Self {
        let operator = match (
            signature.modifiers.operator,
            signature.modifiers.property_delegate_operator,
        ) {
            (Some(operator), _) => hir::CallableOperatorRoleV1::Language(wire_operator(operator)),
            (_, Some(operator)) => hir::CallableOperatorRoleV1::PropertyDelegate(match operator {
                hir::PropertyDelegateOperatorKind::ProvideDelegate => {
                    hir::PropertyDelegateOperatorV1::ProvideDelegate
                }
                hir::PropertyDelegateOperatorKind::GetValue => {
                    hir::PropertyDelegateOperatorV1::GetValue
                }
                hir::PropertyDelegateOperatorKind::SetValue => {
                    hir::PropertyDelegateOperatorV1::SetValue
                }
            }),
            (None, None) => hir::CallableOperatorRoleV1::None,
        };
        Self {
            name: name
                .rsplit('.')
                .next()
                .expect("callables have a name")
                .into(),
            parameters: signature.params.iter().map(|p| p.ty).collect(),
            result: signature.return_ty,
            is_suspend: signature.is_suspend,
            safety: match signature.attributes.safety {
                hir::Safety::Safe => hir::CallableSafetyV1::Safe,
                hir::Safety::Unsafe => hir::CallableSafetyV1::Unsafe,
            },
            gc_effect: match signature.attributes.gc_effect {
                hir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
                hir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
            },
            operator,
            infix: if signature.modifiers.is_infix {
                hir::CallableInfixV1::Infix
            } else {
                hir::CallableInfixV1::Ordinary
            },
        }
    }
}

impl Lowerer {
    pub(in crate::class) fn conformance_members(
        &mut self,
        ty: TypeId,
    ) -> Vec<InterfaceMemberInstance> {
        let mut members = Vec::new();
        self.collect_conformance_members(ty, &mut Vec::new(), &mut members);
        let mut suppressed = std::collections::HashSet::new();
        let mut imported_suppressed = std::collections::HashSet::new();
        for member in &members {
            match member.member {
                hir::InterfaceMethodReference::Local(id) => {
                    suppressed.extend(self.interface_method_entities[id].overrides.iter().copied());
                }
                hir::InterfaceMethodReference::Imported { owner, slot } => {
                    let Type::ImportedInterface(interface) = &self.types[owner] else {
                        unreachable!("an imported slot retains its declaring interface");
                    };
                    let method = interface
                        .methods
                        .iter()
                        .find(|method| method.slot.id() == slot)
                        .expect("the declaring interface contains its slot");
                    imported_suppressed.extend(method.overrides.iter().copied());
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        members.retain(|member| {
            !suppressed.contains(&member.member)
                && !matches!(member.member, hir::InterfaceMethodReference::Imported { slot, .. }
                    if imported_suppressed.contains(&slot))
                && seen.insert((member.member, member.owner))
        });
        members
    }

    fn collect_conformance_members(
        &mut self,
        ty: TypeId,
        seen: &mut Vec<TypeId>,
        out: &mut Vec<InterfaceMemberInstance>,
    ) {
        if seen.contains(&ty) {
            return;
        }
        seen.push(ty);
        let members = match self.types[ty].clone() {
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let declaration = self.interfaces[application.template].clone();
                for parent in declaration.parents {
                    let parent = self.instantiate_ty(parent, &application.arguments);
                    self.collect_conformance_members(parent, seen, out);
                }
                declaration
                    .methods
                    .into_iter()
                    .filter_map(|member| {
                        let function = self.interface_method_entities[member].function;
                        let arguments = application.arguments.clone();
                        if self.functions[function].method_type_param_count() != 0 {
                            // Invalid generic interface members already have a declaration diagnostic.
                            return None;
                        }
                        let declaration = self.interface_method_entities[member].clone();
                        let signature = self.instantiated_signature(function, &arguments, &[]);
                        let name = self.functions[function].name.clone();
                        let owner = self.intern_interface_application(declaration.owner, arguments);
                        Some(InterfaceMemberInstance {
                            member: hir::InterfaceMethodReference::Local(member),
                            owner,
                            signature: InterfaceSignature::local(&name, &signature),
                            implementation: declaration.implementation,
                            mutable_property: match declaration.role {
                                hir::InterfaceMemberRole::PropertySetter(property) => {
                                    Some(self.properties[property].name.clone())
                                }
                                _ => None,
                            },
                        })
                    })
                    .collect::<Vec<_>>()
            }
            Type::ImportedInterface(interface) => interface
                .methods
                .iter()
                .map(|method| {
                    let hir::PublicDeclarationOwnerV1::Nominal(owner) = method.declaration.owner()
                    else {
                        unreachable!("interface members have nominal declaration owners")
                    };
                    let owner = self
                        .imported_member_owner_type(ty, owner)
                        .expect("the imported member owner was resolved with its interface");
                    let effects = method.declaration.effects();
                    let mutable_property = match method.declaration.declaration() {
                        scoop_identity::CallableTemplateOrigin::Accessor(accessor) => self
                            .dependencies
                            .as_ref()
                            .and_then(|dependencies| dependencies.property_for_accessor(accessor))
                            .filter(|property| property.accessors().setter() == Some(accessor))
                            .map(|_| method.name.clone()),
                        _ => None,
                    };
                    InterfaceMemberInstance {
                        member: hir::InterfaceMethodReference::Imported {
                            owner,
                            slot: method.slot.id(),
                        },
                        owner,
                        signature: InterfaceSignature {
                            name: match method.slot.key().role() {
                                scoop_identity::DispatchRole::PropertyGetter => {
                                    format!("$get${}", method.name)
                                }
                                scoop_identity::DispatchRole::PropertySetter => {
                                    format!("$set${}", method.name)
                                }
                                _ => method.name.clone(),
                            },
                            parameters: method.parameters.iter().map(|(_, ty)| *ty).collect(),
                            result: method.return_type,
                            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
                            safety: effects.safety(),
                            gc_effect: effects.gc_effect(),
                            operator: effects.operator_role(),
                            infix: effects.infix(),
                        },
                        implementation: if method.declaration.modality()
                            == hir::CallableModalityV1::Abstract
                        {
                            hir::InterfaceMemberImplementation::AbstractSlot
                        } else {
                            hir::InterfaceMemberImplementation::Body
                        },
                        mutable_property,
                    }
                })
                .collect(),
            _ => unreachable!("conformance members belong to an interface"),
        };
        out.extend(members);
    }

    pub(in crate::class) fn same_interface_signature_shape(
        &self,
        a: &InterfaceSignature,
        b: &InterfaceSignature,
    ) -> bool {
        a.name == b.name
            && a.parameters.len() == b.parameters.len()
            && a.parameters
                .iter()
                .zip(&b.parameters)
                .all(|(a, b)| self.types_equal(*a, *b))
            && self.types_equal(a.result, b.result)
    }

    pub(in crate::class) fn same_interface_signature(
        &self,
        a: &InterfaceSignature,
        b: &InterfaceSignature,
    ) -> bool {
        self.same_interface_signature_shape(a, b)
            && a.is_suspend == b.is_suspend
            && a.safety == b.safety
            && a.gc_effect == b.gc_effect
            && a.operator == b.operator
            && a.infix == b.infix
    }

    pub(super) fn conformance_target(
        &mut self,
        member: &InterfaceMemberInstance,
        span: ast::Span,
    ) -> Option<hir::InterfaceImplementationTarget> {
        let abstract_slot =
            member.implementation == hir::InterfaceMemberImplementation::AbstractSlot;
        match member.member {
            hir::InterfaceMethodReference::Local(id) => {
                let Type::Interface(owner) = self.types[member.owner] else {
                    unreachable!("local interface member has a local application")
                };
                let function = self.interface_method_entities[id].function;
                let application = self.record_method_application(
                    function,
                    hir::MethodOwnerApplication::Interface(owner),
                );
                Some(if abstract_slot {
                    hir::InterfaceImplementationTarget::Abstract(application)
                } else {
                    hir::InterfaceImplementationTarget::Method(application)
                })
            }
            hir::InterfaceMethodReference::Imported { owner, slot } => {
                let Type::ImportedInterface(interface) = &self.types[owner] else {
                    unreachable!("imported interface member has an imported owner")
                };
                let declaration = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| {
                        dependencies
                            .callable_for_slot(interface.declaration.owner(), slot)
                            .expect("resolved interface callable is available")
                    })
                    .expect("resolved interface slots have a callable declaration");
                self.imported_conformance_target(declaration, owner, span)
            }
        }
    }

    pub(super) fn imported_conformance_target(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        owner: TypeId,
        span: ast::Span,
    ) -> Option<hir::InterfaceImplementationTarget> {
        let abstract_slot = declaration.interface().modality() == hir::CallableModalityV1::Abstract;
        let callable = match self.resolve_imported_dispatch_callable(declaration, owner) {
            Ok(callable) => callable,
            Err(error) => {
                self.error(
                    span,
                    format!("invalid inherited interface target: {error:?}"),
                );
                return None;
            }
        };
        Some(match (abstract_slot, callable) {
            (false, hir::ImportedDispatchCallable::External(callable)) => {
                hir::InterfaceImplementationTarget::Imported(callable)
            }
            (true, hir::ImportedDispatchCallable::External(callable)) => {
                hir::InterfaceImplementationTarget::ImportedAbstract(callable)
            }
            (false, hir::ImportedDispatchCallable::Template(application)) => {
                hir::InterfaceImplementationTarget::ImportedTemplate(application)
            }
            (true, hir::ImportedDispatchCallable::Template(application)) => {
                hir::InterfaceImplementationTarget::ImportedAbstractTemplate(application)
            }
        })
    }
}
