//! Shared semantic queries over the frontend's established declaration graph.

use crate::{CoreLoweringAuthority, Lowerer, Owner};
use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;

mod context;
mod declarations;
mod types;

pub(crate) struct FrontendQueries<'a> {
    pub(crate) lowerer: &'a mut Lowerer,
}

impl hir::SemanticQueries for FrontendQueries<'_> {
    fn current_cone(&self) -> scoop_identity::ConeIdentity {
        self.lowerer.current_cone()
    }

    fn pending_methods(&self) -> Vec<hir::PendingMethod> {
        self.lowerer
            .generation
            .methods
            .values()
            .map(|method| method.task.clone())
            .collect()
    }

    fn annotations(&self) -> &hir::SourceAnnotations {
        &self.lowerer.annotation_metadata
    }

    fn annotation_declaration(
        &self,
        annotation: scoop_identity::PersistentAnnotationId,
    ) -> Option<hir::SemanticAnnotationDeclaration<'_>> {
        self.query_annotation_declaration(annotation)
    }

    fn describe_annotations(
        &self,
        target: hir::SemanticAnnotationTarget,
    ) -> hir::SemanticAnnotations<'_> {
        self.query_annotations(target)
    }

    fn body(
        &self,
        callable: hir::SemanticCallable,
    ) -> Option<hir::SemanticBodyAvailability<hir::SemanticBody<'_>>> {
        self.query_body(callable)
    }

    fn default_source(&self, source: hir::ExportDefaultSourceId) -> hir::SemanticDefaultSource<'_> {
        let value = &self.lowerer.export_default_sources[source];
        hir::SemanticDefaultSource {
            source,
            definition: match value {
                hir::ExportDefaultSource::Declared { expression, .. } => {
                    hir::SemanticDefaultDefinition::Current {
                        expression: *expression,
                        template: &self.lowerer.export_default_exprs[*expression],
                    }
                }
                hir::ExportDefaultSource::Imported { template, .. } => {
                    hir::SemanticDefaultDefinition::Imported(template)
                }
            },
            type_arguments: value.type_arguments(),
        }
    }

    fn source_location(&self, origin: hir::DefinitionOrigin) -> hir::SemanticSourceLocation {
        hir::SemanticSourceLocation {
            source: self.lowerer.source_identity(origin.file as usize).clone(),
            span: origin.span,
        }
    }

    fn signature(&self, function: hir::FunctionId) -> hir::SemanticFunctionSignature {
        let lowerer = &self.lowerer;
        let signature = &lowerer.signatures[&function];
        let parameters = lowerer
            .source_parameter_interfaces
            .iter()
            .find(|interface| {
                matches!(interface.owner, hir::ExportParameterOwner::Function(id) if id == function)
            })
            .expect("complete source signatures retain their parameter interface");
        hir::SemanticFunctionSignature {
            function,
            name: lowerer.source_function_name(function).into(),
            receiver: lowerer.functions[function]
                .method
                .map(|method| method.owner)
                .or_else(|| lowerer.extension_receivers.get(&function).copied()),
            parameters: parameters.parameters.clone(),
            type_parameters: signature.type_params.clone(),
            owner_parameter_count: signature.owner_type_param_count,
            context_parameters: signature.context_parameters.clone(),
            result: signature.return_ty,
            is_suspend: signature.is_suspend,
            modifiers: signature.modifiers,
            attributes: signature.attributes,
            origin: hir::SemanticSourceLocation {
                source: lowerer
                    .source_identity(
                        lowerer
                            .function_definition_file(function)
                            .expect("source callable signatures retain their definition file"),
                    )
                    .clone(),
                span: lowerer.functions[function].span,
            },
        }
    }

    fn method_identity(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<hir::HirSourceFunctionIdentity, Diagnostic> {
        if let Some(identity) = self.lowerer.query_source_function_identities.get(&function) {
            return Ok(identity.clone());
        }
        let identity = crate::persistent_functions::direct_source_identity(
            self.lowerer,
            function,
            self.core_types(),
        )
        .map_err(|error| {
            let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
            diagnostic.file = error.file();
            diagnostic
        })?;
        self.lowerer
            .query_source_function_identities
            .insert(function, identity.clone());
        Ok(identity)
    }

    fn type_signature(
        &self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<SignatureTypeKey, Diagnostic> {
        let mapper = crate::persistent_types::SignatureTypeMapper::new(
            self.lowerer,
            self.lowerer
                .nominal_identities
                .as_ref()
                .expect("queries follow declaration identity construction"),
            self.core_types(),
        );
        mapper
            .map(ty, &self.signature_binders(context))
            .map_err(|error| {
                Diagnostic::at(Span::new(0, 0), format!("cannot describe type: {error}"))
            })
    }

    fn describe_type(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<hir::SemanticTypeDescription, hir::StaticShapeUnboundParameter> {
        self.in_context(context, |lowerer| lowerer.describe_semantic_type(ty))
    }

    fn type_relation(
        &mut self,
        context: hir::SemanticContext,
        relation: hir::SemanticTypeRelation,
        left: hir::TypeId,
        right: hir::TypeId,
    ) -> bool {
        self.in_context(context, |lowerer| match relation {
            hir::SemanticTypeRelation::Equal => lowerer.types_equal(left, right),
            hir::SemanticTypeRelation::Subtype => lowerer.is_subtype(left, right),
        })
    }

    fn interfaces(&mut self, owner: hir::NominalOwner) -> Vec<hir::TypeId> {
        self.in_context(hir::SemanticContext::Nominal(owner), |lowerer| {
            lowerer.query_owner_interfaces(owner)
        })
    }

    fn requirements(&mut self, interface: hir::TypeId) -> Vec<hir::AppliedRequirement> {
        self.lowerer.query_interface_requirements(interface)
    }

    fn resolve_type(
        &mut self,
        context: hir::SemanticContext,
        syntax: &scoop_ast::TypeRef,
    ) -> Result<hir::TypeId, Vec<Diagnostic>> {
        self.in_context(context, |lowerer| {
            let before = lowerer.diagnostics.len();
            let result = lowerer
                .resolve_type_ref(syntax)
                .filter(|ty| lowerer.check_query_type_representation(*ty, syntax.span));
            let mut diagnostics = lowerer.diagnostics.split_off(before);
            let sources = lowerer.diagnostic_source_identities();
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            match result {
                Some(ty) if diagnostics.is_empty() => Ok(ty),
                _ => Err(diagnostics),
            }
        })
    }
}

impl FrontendQueries<'_> {
    fn core_types(&self) -> hir::HirCoreTypeIdentityAuthority<'_> {
        match &self.lowerer.core {
            CoreLoweringAuthority::Defined => hir::HirCoreTypeIdentityAuthority::Defined(
                self.lowerer
                    .defined_intrinsic_type_core
                    .as_ref()
                    .expect("fixed queries follow core protocol resolution"),
            ),
            CoreLoweringAuthority::Imported(core) => {
                hir::HirCoreTypeIdentityAuthority::Imported(core.fundamental_types())
            }
        }
    }
}
