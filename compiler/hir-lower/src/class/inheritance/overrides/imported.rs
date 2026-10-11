use super::*;

impl Lowerer {
    pub(super) fn check_imported_interface_override(
        &mut self,
        id: FunctionId,
        decl: &MemberOverride,
        signature: &FnSig,
        inherited: &[InterfaceMemberInstance],
    ) {
        use hir::ImportedCallableSource;
        for member in inherited {
            self.defer_imported_override_result(id, member.signature.result, decl.name.span);
            let target = format!("{}.{}", self.type_name(member.owner), member.signature.name);
            let covers = self.functions[id]
                .access
                .slot
                .as_ref()
                .is_some_and(|provided| {
                    self.access_domain_is_subset(&hir::AccessDomain::universal(), &provided.0)
                });
            if !covers {
                self.error(
                    decl.name.span,
                    format!(
                        "visibility of `{}` does not cover inherited slot `{target}`",
                        decl.name.text
                    ),
                );
            }
            let hir::InterfaceMethodReference::Imported { owner, slot } = member.member else {
                unreachable!("imported candidates retain imported slots")
            };
            let Some(interface) = self.dependency_interface_definition(owner) else {
                unreachable!("imported slot owner is an interface")
            };
            let declaration = self
                .dependencies
                .as_ref()
                .expect("an imported interface has a dependency catalog")
                .callable_for_slot(interface.declaration.owner(), slot)
                .expect("resolved interface callable")
                .expect("resolved interface slot");
            for (index, parameter) in signature.params.iter().enumerate() {
                let inherited_vararg = declaration.source_interface().is_some_and(|source| {
                    source.parameters().parameters()[index]
                        .calling()
                        .is_vararg()
                });
                if matches!(parameter.calling, crate::FnParamCalling::Vararg { .. })
                    != inherited_vararg
                {
                    self.error(decl.parameter_spans[index], format!("parameter `{}` of `{}` must have the same `vararg` shape as `{target}`", signature.params[index].name.text, decl.name.text));
                    break;
                }
            }
            let hir::PublicDeclarationOwnerV1::Nominal(nominal) = declaration.interface().owner()
            else {
                unreachable!("an inherited interface method has a nominal owner")
            };
            let owner = self
                .imported_member_owner_type(owner, nominal)
                .expect("the inherited method retains its declaring application");
            self.override_default_sources.entry(id).or_default().push(
                crate::defaults::DefaultOverrideSource::Imported {
                    declaration: declaration.interface().declaration(),
                    owner,
                },
            );
        }
    }
}
