//! Ordinary missing interface methods retained for the driver generation step.

use super::inheritance::InterfaceMemberInstance;
use super::*;
use crate::generation::PendingGenerationMethod;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::class) fn declare_generated_interface_method(
        &mut self,
        owner: Owner,
        member: &InterfaceMemberInstance,
        candidates: &[crate::CallableCandidate],
        span: ast::Span,
        host: &str,
    ) -> Option<FunctionId> {
        if !self.generation.enabled
            || member.signature.accessor.is_some()
            || member.signature.result_pending
            || member.signature.parameter_pending
            || self.coding_candidate_declared(
                &member.signature.name,
                &member.signature.parameters,
                candidates,
            )
        {
            return None;
        }
        let signature = self.interface_forwarder_signature(member, span);
        let function = self.allocate_synthesized_method(
            owner,
            &member.signature.name,
            signature,
            hir::MethodModifier::Final,
            ast::DeclaredVisibility::Public,
            span,
        );
        self.source_function_declarations.insert(
            function,
            crate::SourceFunctionDeclaration {
                name: member.signature.name.clone(),
                name_span: None,
            },
        );
        let mut diagnostic = ast::Diagnostic::at(
            span,
            format!(
                "{host} does not implement interface method `{}.{}`",
                self.type_name(member.owner),
                member.signature.name,
            ),
        );
        diagnostic.file = self.current_file;
        self.generation.methods.insert(
            function,
            PendingGenerationMethod {
                task: hir::PendingMethod {
                    function,
                    owner: owner.as_nominal_owner(),
                    requirements: Vec::new(),
                },
                diagnostic,
            },
        );
        self.record_generated_requirement(function, owner, member);
        Some(function)
    }

    pub(in crate::class) fn record_generated_requirement(
        &mut self,
        function: FunctionId,
        owner: Owner,
        member: &InterfaceMemberInstance,
    ) {
        let Some(pending) = self.generation.methods.get_mut(&function) else {
            return;
        };
        if pending.task.owner != owner.as_nominal_owner() {
            return;
        }
        let requirement = hir::AppliedRequirement {
            interface: member.owner,
            declaration: member.member,
        };
        if pending.task.requirements.contains(&requirement) {
            return;
        }
        pending.task.requirements.push(requirement);
        let source = match member.member {
            hir::InterfaceMethodReference::Local(method) => {
                let Type::Interface(application) = self.types[member.owner] else {
                    unreachable!("a requirement retains its interface application")
                };
                crate::defaults::DefaultOverrideSource::Local {
                    function: self.interface_method_entities[method].function,
                    type_arguments: self.interface_applications[application].arguments.clone(),
                }
            }
            hir::InterfaceMethodReference::Imported { .. } => {
                let declaration = self.delegated_imported_member(member);
                crate::defaults::DefaultOverrideSource::Imported {
                    declaration: declaration.interface().declaration(),
                    owner: member.owner,
                }
            }
        };
        self.override_default_sources
            .entry(function)
            .or_default()
            .push(source);
    }

    pub(crate) fn query_owner_interfaces(&mut self, owner: hir::NominalOwner) -> Vec<TypeId> {
        self.owner_interfaces(Owner::from_nominal_owner(owner))
    }

    pub(crate) fn query_interface_requirements(
        &mut self,
        interface: TypeId,
    ) -> Vec<hir::AppliedRequirement> {
        self.conformance_members(interface)
            .into_iter()
            .map(|member| hir::AppliedRequirement {
                interface: member.owner,
                declaration: member.member,
            })
            .collect()
    }
}
