//! Capture only source associations; annotation and body evaluation stay deferred.

use super::*;

impl Lowerer {
    pub(super) fn capture_editing_documentation(
        &mut self,
        declarations: &SourceDeclarations<'_>,
        check: &impl Fn() -> Result<(), EditingCancelled>,
    ) -> Result<(), EditingCancelled> {
        if self.documentation.is_none() {
            return Ok(());
        }
        for &(function, source, _) in &declarations.function_headers {
            check()?;
            self.document_editing_function(function, source);
        }
        for &(function, source, _, _) in &declarations.method_headers {
            check()?;
            self.document_editing_function(function, source);
        }
        for &(id, source, _) in &declarations.pending_structs {
            check()?;
            self.in_context(
                hir::SemanticContext::Nominal(hir::NominalOwner::Struct(id)),
                |lowerer| {
                    lowerer.document_struct(id, source);
                    for member in source.members {
                        if let ast::StructMember::Property(property) = member {
                            lowerer.document_editing_property(property);
                        }
                    }
                },
            );
        }
        for &(id, source, _) in &declarations.pending_classes {
            check()?;
            self.in_context(
                hir::SemanticContext::Nominal(hir::NominalOwner::Class(id)),
                |lowerer| {
                    lowerer.document_class(id, source);
                    lowerer.document_editing_class_members(source.members);
                },
            );
        }
        for &(id, source, _) in &declarations.pending_interfaces {
            check()?;
            self.in_context(
                hir::SemanticContext::Nominal(hir::NominalOwner::Interface(id)),
                |lowerer| {
                    lowerer.document_interface(id, source);
                    for property in source.properties {
                        lowerer.document_editing_property(property);
                    }
                },
            );
        }
        for &(id, source, _) in &declarations.pending_enums {
            check()?;
            self.in_context(
                hir::SemanticContext::Nominal(hir::NominalOwner::Enum(id)),
                |lowerer| {
                    lowerer.document_enum(id, source);
                    for property in &source.properties {
                        lowerer.document_editing_property(property);
                    }
                },
            );
        }
        for &(id, source, _) in &declarations.pending_objects {
            check()?;
            self.in_context(
                hir::SemanticContext::Nominal(hir::NominalOwner::Object(id)),
                |lowerer| {
                    lowerer.document_object(id, source);
                    lowerer.document_editing_class_members(source.members());
                },
            );
        }
        let previous = (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope.clone(),
        );
        self.current_owner = None;
        self.type_params_in_scope.clear();
        for &(source, file) in &declarations.pending_globals {
            check()?;
            self.current_file = file;
            self.document_editing_property(source);
        }
        (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope,
        ) = previous;
        Ok(())
    }

    fn document_editing_function(
        &mut self,
        function: hir::FunctionId,
        source: ast::FunctionHeaderRef<'_>,
    ) {
        if self
            .declaration_surface
            .unavailable_signature(function)
            .is_none()
            && self
                .signatures
                .get(&function)
                .is_some_and(|signature| !signature.parameter_pending)
        {
            self.in_context(hir::SemanticContext::Function(function), |lowerer| {
                lowerer.document_function(function, source);
            });
        }
    }

    fn document_editing_class_members(&mut self, members: &[ast::ClassMember]) {
        for member in members {
            if let ast::ClassMember::StoredProperty(property) = member {
                self.document_editing_property(property);
            }
        }
    }

    fn document_editing_property(&mut self, source: &ast::PropertyDecl) {
        let Some(property) = self.property_declaration_id(source, self.current_file) else {
            return;
        };
        let previous = self.type_params_in_scope.clone();
        if let hir::PropertyOwner::Extension(extension) = self.properties[property].owner {
            self.type_params_in_scope = self.extension_properties[extension].type_params.clone();
        }
        self.document_property(property, source);
        self.type_params_in_scope = previous;
    }
}
