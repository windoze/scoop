//! Checked interface slots become ordinary methods and property accessors.

use super::interfaces::{InterfaceMemberInstance, InterfaceSignature};
use super::*;
use crate::properties::PropertyAccessorKind;

mod access;
mod bodies;
mod properties;
mod selection;
mod signatures;

#[derive(Clone)]
pub(crate) struct InterfaceForwarder {
    member: InterfaceMemberInstance,
    receiver: ForwardingReceiver,
    role: ForwardingRole,
}

#[derive(Clone, Copy)]
enum ForwardingRole {
    Method,
    Getter(hir::PropertyId),
    Setter(hir::PropertyId),
}

#[derive(Clone, Copy)]
enum ForwardingReceiver {
    Delegate(hir::ClassFieldId),
    Base {
        owner: TypeId,
        callee: hir::CallableTarget,
    },
}

#[derive(Clone)]
struct DelegateCandidate {
    field: hir::ClassFieldId,
    member: InterfaceMemberInstance,
}

enum DelegateSelection {
    Forward(ForwardingReceiver),
    Own,
    Conflict,
}

struct DelegateSlot {
    candidate: DelegateCandidate,
    selection: DelegateSelection,
}

impl InterfaceForwarder {
    pub(crate) fn name(&self) -> &str {
        &self.member.signature.name
    }

    pub(crate) fn property(&self) -> Option<hir::PropertyId> {
        match self.role {
            ForwardingRole::Getter(property) | ForwardingRole::Setter(property) => Some(property),
            ForwardingRole::Method => None,
        }
    }

    pub(crate) fn accessor(&self) -> Option<PropertyAccessorKind> {
        match self.role {
            ForwardingRole::Getter(_) => Some(PropertyAccessorKind::Getter),
            ForwardingRole::Setter(_) => Some(PropertyAccessorKind::Setter),
            ForwardingRole::Method => None,
        }
    }
}

impl Lowerer {
    pub(super) fn declare_interface_forwarders(&mut self, owner: Owner) -> Vec<FunctionId> {
        let class = match owner {
            Owner::Class(class) => class,
            Owner::Object(object) => self.objects[object].backing_class,
            _ => return Vec::new(),
        };
        let slots = self.select_interface_delegates(owner, class);
        let mut functions = Vec::new();
        for slot in &slots {
            if slot.candidate.member.signature.accessor.is_some() {
                continue;
            }
            if let DelegateSelection::Forward(receiver @ ForwardingReceiver::Delegate(_)) =
                slot.selection
            {
                functions.push(self.allocate_interface_forwarder(
                    owner,
                    &slot.candidate.member,
                    receiver,
                    ForwardingRole::Method,
                ));
            }
        }
        functions.extend(self.declare_delegated_properties(owner, class, &slots));
        functions
    }

    fn allocate_interface_forwarder(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        receiver: ForwardingReceiver,
        role: ForwardingRole,
    ) -> FunctionId {
        let span = match receiver {
            ForwardingReceiver::Delegate(field) => self.class_fields[field].span,
            ForwardingReceiver::Base { .. } => match owner {
                Owner::Class(class) => self.classes[class].span,
                Owner::Object(object) => self.objects[object].span,
                _ => unreachable!("only reference types delegate interfaces"),
            },
        };
        let signature = self.interface_forwarder_signature(member, span);
        let name = &member.signature.name;
        let class = match owner {
            Owner::Class(class) => class,
            Owner::Object(object) => self.objects[object].backing_class,
            _ => unreachable!("only reference types delegate interfaces"),
        };
        let modifier = if self.classes[class].modifier == hir::ClassModifier::Final {
            hir::MethodModifier::Final
        } else {
            hir::MethodModifier::Open
        };
        let visibility = self.interface_forwarder_visibility(owner, class, &member.signature);
        let function =
            self.allocate_synthesized_method(owner, name, signature, modifier, visibility, span);
        if matches!(role, ForwardingRole::Method) {
            self.source_function_declarations.insert(
                function,
                crate::SourceFunctionDeclaration { name: name.clone() },
            );
        }
        self.interface_forwarders.insert(
            function,
            InterfaceForwarder {
                member: member.clone(),
                receiver,
                role,
            },
        );
        function
    }

    pub(super) fn check_interface_forwarder(&mut self, owner: Owner, function: FunctionId) {
        let span = self.functions[function].span;
        let declaration = super::overrides::MemberOverride {
            name: ast::Ident {
                text: self.source_function_name(function).into(),
                span,
            },
            is_override: true,
            parameter_spans: vec![span; self.signatures[&function].params.len()],
        };
        self.check_member_access_contract(function, &declaration, owner);
        self.check_override_rules(function, &declaration, owner);
    }
}
