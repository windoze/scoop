use super::*;
use crate::imported_core::ImportedTypeBindings;
use hir::ImportedCallableSource;

mod parameters;

impl Lowerer {
    pub(super) fn query_imported_methods(
        &mut self,
        receiver: hir::TypeId,
        owner: &hir::ImportedNominalDeclaration,
        owner_parameters: &[hir::TypeParamDecl],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticMethodDescription>, Diagnostic> {
        owner
            .interface
            .declaration_details()
            .members()
            .values()
            .iter()
            .filter_map(|member| match *member {
                hir::NestedSourceMemberRefV1::Function(id) => {
                    Some(CallableTemplateOrigin::Function(id))
                }
                hir::NestedSourceMemberRefV1::GenericFunction(id) => {
                    Some(CallableTemplateOrigin::GenericFunction(id))
                }
                hir::NestedSourceMemberRefV1::Property(_) => None,
            })
            .map(|identity| {
                self.query_imported_method(
                    identity,
                    receiver,
                    owner,
                    owner_parameters,
                    substitutions,
                )
            })
            .collect()
    }

    fn query_imported_method(
        &mut self,
        identity: CallableTemplateOrigin,
        receiver: hir::TypeId,
        owner: &hir::ImportedNominalDeclaration,
        owner_parameters: &[hir::TypeParamDecl],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<hir::SemanticMethodDescription, Diagnostic> {
        let declaration = self
            .dependencies
            .as_ref()
            .expect("imported method has its catalog")
            .callable_declaration(identity)
            .expect("a checked nominal retains its method declaration");
        let origin = imported_location(declaration.definition_origin().origin())?;
        let interface = declaration.interface();
        let parameters =
            self.query_imported_method_binders(&declaration, owner_parameters, origin.span)?;
        let complete = self.complete_parameter_substitution(parameters.iter(), substitutions);
        let substitutions = &complete;
        let bindings = self.query_signature_bindings(owner_parameters.len(), &parameters);
        let context_parameters = self
            .imported_context_parameters(interface.context_parameters(), &bindings, origin.span)
            .map_err(|error| Diagnostic::at(origin.span, error))?;
        let arguments: Vec<_> = owner_parameters
            .iter()
            .map(|p| self.intern_type(hir::Type::Param(p.id)))
            .collect();
        let declaration_type = match self.types[receiver] {
            hir::Type::Ptr(_) => self.intern_type(hir::Type::Ptr(arguments[0])),
            _ if arguments.is_empty() => receiver,
            _ => self
                .apply_nominal_type(owner.owner(), arguments)
                .expect("a loaded declaration has its original owner application"),
        };
        let effects = interface.effects();
        let release_callability = self
            .imported_release_callability(&effects, &bindings)
            .map_err(|error| Diagnostic::at(origin.span, error))?;
        let callable = match identity {
            CallableTemplateOrigin::Function(id) => hir::DefaultCallableDeclarationV1::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                hir::DefaultCallableDeclarationV1::GenericFunction(id)
            }
            _ => unreachable!("nominal method inventories contain source functions"),
        };
        Ok(hir::SemanticMethodDescription {
            identity,
            callable: hir::SemanticCallable::Imported(callable),
            name: declaration.name().to_owned(),
            receiver: hir::SemanticTypeUse {
                declaration: declaration_type,
                applied: receiver,
            },
            owner_parameters: parameters[..owner_parameters.len()].to_vec(),
            type_parameters: self
                .query_method_type_parameters(&parameters[owner_parameters.len()..], substitutions),
            context_parameters: self.query_context_parameters(&context_parameters, substitutions),
            parameters: self.query_imported_parameters(&declaration, &bindings, substitutions)?,
            result: self.query_published_type_use(interface.result(), &bindings, substitutions)?,
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: effects.callable_modifiers(),
            attributes: effects.function_attributes(),
            release_callability,
            modality: interface.modality(),
            access: hir::SemanticMemberAccess {
                visibility: interface.declared_visibility(),
                domain: self.imported_member_access_domain(owner, interface.declared_visibility()),
                accessible: self.imported_callable_is_accessible(interface, Some(receiver)),
            },
            annotations: hir::SemanticAnnotationTarget::Imported(
                hir::AnnotationTargetV1::Callable(hir::AnnotationCallableV1::Declaration(identity)),
            ),
            origin,
        })
    }
}
