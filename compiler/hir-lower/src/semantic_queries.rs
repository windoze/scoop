//! Shared semantic queries over the frontend's established declaration graph.

use crate::{CoreLoweringAuthority, Lowerer, Owner};
use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;

mod context;
use context::imported_location;
mod constructors;
mod declarations;
mod exports;
mod fields;
mod methods;
mod parameters;
mod properties;
mod types;

pub(crate) struct FrontendQueries<'a, 'input> {
    pub(crate) lowerer: &'a mut Lowerer,
    pub(crate) world: Option<&'a hir::ImportedSemanticWorld<'input>>,
}

impl hir::SemanticQueries for FrontendQueries<'_, '_> {
    fn current_cone(&self) -> scoop_identity::ConeIdentity {
        self.lowerer.current_cone()
    }

    fn lookup_export(
        &mut self,
        context: hir::SemanticContext,
        provider: scoop_identity::ConeIdentity,
        path: &[scoop_identity::CanonicalIdentifier],
        namespace: scoop_identity::BindingNamespace,
    ) -> Result<Vec<scoop_identity::BindableEntity>, Diagnostic> {
        let world = self.world;
        self.in_context(context, |lowerer| {
            lowerer.query_export(world, provider, path, namespace)
        })
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

    fn default_definition(
        &self,
        source: hir::SemanticDefaultReference,
    ) -> Option<hir::SemanticDefaultDefinition<'_>> {
        match source {
            hir::SemanticDefaultReference::Current(source) => {
                Some(self.default_source(source).definition)
            }
            hir::SemanticDefaultReference::Imported(key) => self
                .lowerer
                .dependencies
                .as_ref()?
                .default_template(key)
                .map(hir::SemanticDefaultDefinition::Imported),
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
        self.lowerer.query_function_identity(function)
    }

    fn type_signature(
        &self,
        context: hir::SemanticTypeContext<'_>,
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
        context: hir::SemanticTypeContext<'_>,
        ty: hir::TypeId,
    ) -> Result<hir::SemanticTypeDescription, hir::StaticShapeUnboundParameter> {
        self.in_type_context(context, |lowerer| lowerer.describe_semantic_type(ty))
    }

    fn nominal_fields(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticNominalFields>, Diagnostic> {
        self.in_context(context, |lowerer| lowerer.query_nominal_fields(ty))
    }

    fn nominal_properties(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticPropertyDescription>>, Diagnostic> {
        self.in_context(context, |lowerer| lowerer.query_nominal_properties(ty))
    }

    fn nominal_constructors(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticConstructorDescription>>, Diagnostic> {
        self.in_context(context, |lowerer| lowerer.query_nominal_constructors(ty))
    }

    fn nominal_methods(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticMethodDescription>>, Diagnostic> {
        self.in_context(context, |lowerer| lowerer.query_nominal_methods(ty))
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

impl FrontendQueries<'_, '_> {
    fn core_types(&self) -> hir::HirCoreTypeIdentityAuthority<'_> {
        self.lowerer.query_core_types()
    }
}

impl Lowerer {
    pub(crate) fn query_core_types(&self) -> hir::HirCoreTypeIdentityAuthority<'_> {
        match &self.core {
            CoreLoweringAuthority::Defined => hir::HirCoreTypeIdentityAuthority::Defined(
                self.defined_intrinsic_type_core
                    .as_ref()
                    .expect("fixed queries follow core protocol resolution"),
            ),
            CoreLoweringAuthority::Imported(core) => {
                hir::HirCoreTypeIdentityAuthority::Imported(core.fundamental_types())
            }
        }
    }
}
