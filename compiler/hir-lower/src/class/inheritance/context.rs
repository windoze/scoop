//! Context requirements are source contracts, separate from overload signatures.

use super::interfaces::{InterfaceMemberInstance, InterfaceSignature};
use super::*;

impl Lowerer {
    fn same_context_types(&self, left: &[TypeId], right: &[TypeId]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(a, b)| self.types_equal(*a, *b))
    }

    pub(super) fn check_override_context(
        &mut self,
        signature: &FnSig,
        required: &[TypeId],
        span: ast::Span,
        target: &str,
    ) {
        let actual = signature
            .context_parameters
            .iter()
            .map(|p| p.ty)
            .collect::<Vec<_>>();
        if !self.same_context_types(&actual, required) {
            self.error(span, format!(
                "context requirements must have the same count, order and exact types as `{target}`"
            ));
        }
    }

    pub(super) fn check_inherited_context_contracts(&mut self, owner: Owner) {
        let span = match owner {
            Owner::Class(id) => self.classes[id].span,
            Owner::Struct(id) => self.structs[id].span,
            Owner::Enum(id) => self.enums[id].span,
            Owner::Interface(id) => self.interfaces[id].span,
            Owner::Object(id) => self.objects[id].span,
        };
        let mut members: Vec<InterfaceMemberInstance> = Vec::new();
        for interface in self.owner_interfaces(owner) {
            for member in self.conformance_members(interface) {
                if members
                    .iter()
                    .any(|earlier| earlier.member == member.member && earlier.owner == member.owner)
                {
                    continue;
                }
                if members.iter().any(|earlier| {
                    self.same_interface_signature(&earlier.signature, &member.signature)
                        && !self.same_context_types(
                            &earlier.signature.context_parameters,
                            &member.signature.context_parameters,
                        )
                }) {
                    self.error(
                        span,
                        format!(
                            "{} inherits conflicting context requirements for `{}`",
                            owner.describe(self),
                            member.signature.name
                        ),
                    );
                }
                members.push(member);
            }
        }
    }

    pub(super) fn check_conformance_context(
        &mut self,
        owner: Owner,
        local: Option<&crate::CallableCandidate>,
        imported: Option<&super::imported::ImportedInheritedMethod>,
        required: &InterfaceMemberInstance,
        span: ast::Span,
    ) {
        let target = format!(
            "{}.{}",
            self.type_name(required.owner),
            required.signature.name
        );
        if let Some(candidate) = local {
            // Own declarations were checked against every inherited slot by
            // check_override_rules; only a newly inherited implementation remains.
            if self.function_owner.get(&candidate.function) == Some(&owner) {
                return;
            }
            let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                unreachable!("interface implementations are methods")
            };
            let arguments = self.method_owner_arguments(owner).to_vec();
            let signature = self.instantiated_signature(candidate.function, &arguments, &[]);
            self.check_override_context(
                &signature,
                &required.signature.context_parameters,
                span,
                &target,
            );
        } else if let Some(imported) = imported {
            self.check_interface_context(&imported.signature, &required.signature, span, &target);
        }
    }

    fn check_interface_context(
        &mut self,
        actual: &InterfaceSignature,
        required: &InterfaceSignature,
        span: ast::Span,
        target: &str,
    ) {
        if !self.same_context_types(&actual.context_parameters, &required.context_parameters) {
            self.error(span, format!(
                "context requirements must have the same count, order and exact types as `{target}`"
            ));
        }
    }
}
