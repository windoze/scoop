//! Shared semantic queries over the frontend's established declaration graph.

use crate::{CoreLoweringAuthority, Lowerer, Owner};
use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;

mod body_projection;
mod context;
use context::imported_location;
mod constructors;
mod declaration_references;
mod declarations;
mod defaults;
mod exports;
mod fields;
mod methods;
mod parameters;
mod properties;
mod signatures;
mod type_references;
mod types;

pub(crate) struct FrontendQueries<'a, 'input> {
    pub(crate) lowerer: &'a mut Lowerer,
    pub(crate) world: Option<&'a hir::ImportedSemanticWorld<'input>>,
    body_queries: body_projection::BodyQueries<'a>,
    nominal_references: std::cell::OnceCell<type_references::NominalReferences>,
    declaration_references:
        std::cell::OnceCell<Result<declaration_references::DeclarationReferences, Diagnostic>>,
}

impl<'a, 'input> FrontendQueries<'a, 'input> {
    pub(crate) fn new(
        lowerer: &'a mut Lowerer,
        original: &'a Lowerer,
        world: Option<&'a hir::ImportedSemanticWorld<'input>>,
    ) -> Self {
        Self {
            lowerer,
            world,
            body_queries: body_projection::BodyQueries::new(original),
            nominal_references: std::cell::OnceCell::new(),
            declaration_references: std::cell::OnceCell::new(),
        }
    }

    pub(crate) fn retain_identities(self) {
        self.body_queries.retain_identities(self.lowerer);
    }
}

impl hir::SemanticQueries for FrontendQueries<'_, '_> {
    fn current_cone(&self) -> scoop_identity::ConeIdentity {
        self.lowerer.current_cone()
    }

    fn resolve_provider(
        &self,
        reference: scoop_identity::DecodedPersistentId<scoop_identity::ConeIdentity>,
    ) -> Option<scoop_identity::ConeIdentity> {
        self.query_provider(reference)
    }

    fn lookup_export(
        &mut self,
        context: hir::SemanticContext,
        provider: scoop_identity::ConeIdentity,
        path: &[scoop_identity::CanonicalIdentifier],
        namespace: scoop_identity::BindingNamespace,
    ) -> Result<Vec<scoop_identity::BindableEntity>, Diagnostic> {
        let world = self.world;
        self.lowerer.in_context(context, |lowerer| {
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

    fn resolve_declaration(
        &mut self,
        reference: hir::DecodedAnnotationTargetV1,
    ) -> Result<Option<hir::SemanticDeclaration>, Diagnostic> {
        self.query_declaration(reference)
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

    fn projected_body(
        &mut self,
        callable: hir::SemanticCallable,
    ) -> Result<
        Option<hir::SemanticBodyAvailability<std::sync::Arc<hir::ExportGenericCallableBodyV1>>>,
        Diagnostic,
    > {
        self.query_projected_body(callable)
    }

    fn projected_default(
        &mut self,
        source: hir::SemanticDefaultReference,
    ) -> Result<Option<std::sync::Arc<hir::ExportDefaultTemplateV1>>, Diagnostic> {
        self.body_queries.default(source)
    }

    fn constructor_body(
        &mut self,
        constructor: hir::SemanticConstructor,
    ) -> Result<
        Option<hir::SemanticBodyAvailability<std::sync::Arc<hir::SemanticConstructorBody>>>,
        Diagnostic,
    > {
        self.body_queries.constructor(constructor)
    }

    fn default_application(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        definition: hir::SemanticDefaultReference,
        arguments: &[hir::TypeId],
    ) -> Result<Option<hir::SemanticDefaultApplication>, Vec<Diagnostic>> {
        self.query_default_application(context, definition, arguments)
    }

    fn resolve_default_reference(
        &mut self,
        owner: hir::DecodedAnnotationCallableV1,
        parameter: u32,
    ) -> Result<Option<hir::SemanticDefaultReference>, Diagnostic> {
        self.query_default_reference(owner, parameter)
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
        let parameters = self
            .lowerer
            .source_parameter_interfaces
            .iter()
            .find(|interface| {
                matches!(interface.owner, hir::ExportParameterOwner::Function(id) if id == function)
            })
            .expect("complete source signatures retain their parameter interface");
        self.lowerer.semantic_function_signature(
            function,
            self.lowerer.signatures[&function].return_ty,
            parameters.parameters.clone(),
        )
    }

    fn method_identity(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<hir::HirSourceFunctionIdentity, Diagnostic> {
        self.lowerer.query_function_identity(function)
    }

    fn function_parameters(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<Vec<hir::SemanticParameterDescription>, Diagnostic> {
        let identity = match self.lowerer.query_function_identity(function)? {
            hir::HirSourceFunctionIdentity::Plain(record) => {
                scoop_identity::CallableTemplateOrigin::Function(record.id())
            }
            hir::HirSourceFunctionIdentity::Generic(record) => {
                scoop_identity::CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        let parameters = self.lowerer.signatures[&function].type_params.clone();
        let substitutions = self
            .lowerer
            .complete_parameter_substitution(parameters.iter(), &[]);
        Ok(self.lowerer.query_current_parameters(
            hir::ExportParameterOwner::Function(function),
            identity,
            &substitutions,
        ))
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
            .map(ty, &self.lowerer.signature_binders(context))
            .map_err(|error| {
                Diagnostic::at(Span::new(0, 0), format!("cannot describe type: {error}"))
            })
    }

    fn describe_type(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        ty: hir::TypeId,
    ) -> Result<hir::SemanticTypeDescription, Diagnostic> {
        self.lowerer
            .in_type_context(context, |lowerer| lowerer.describe_semantic_type(ty))
    }

    fn nominal_declaration(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticNominalDescription>, Diagnostic> {
        self.lowerer.describe_semantic_nominal(ty)
    }

    fn nominal_fields(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticNominalFields>, Diagnostic> {
        self.lowerer
            .in_context(context, |lowerer| lowerer.query_nominal_fields(ty))
    }

    fn nominal_properties(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticPropertyDescription>>, Diagnostic> {
        self.lowerer
            .in_context(context, |lowerer| lowerer.query_nominal_properties(ty))
    }

    fn nominal_constructors(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticConstructorDescription>>, Diagnostic> {
        self.lowerer
            .in_context(context, |lowerer| lowerer.query_nominal_constructors(ty))
    }

    fn nominal_methods(
        &mut self,
        context: hir::SemanticContext,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticMethodDescription>>, Diagnostic> {
        self.lowerer
            .in_context(context, |lowerer| lowerer.query_nominal_methods(ty))
    }

    fn type_relation(
        &mut self,
        context: hir::SemanticContext,
        relation: hir::SemanticTypeRelation,
        left: hir::TypeId,
        right: hir::TypeId,
    ) -> bool {
        self.lowerer.in_context(context, |lowerer| match relation {
            hir::SemanticTypeRelation::Equal => lowerer.types_equal(left, right),
            hir::SemanticTypeRelation::Subtype => lowerer.is_subtype(left, right),
        })
    }

    fn interfaces(&mut self, owner: hir::NominalOwner) -> Vec<hir::TypeId> {
        self.lowerer
            .in_context(hir::SemanticContext::Nominal(owner), |lowerer| {
                lowerer.query_owner_interfaces(owner)
            })
    }

    fn requirements(&mut self, interface: hir::TypeId) -> Vec<hir::AppliedRequirement> {
        self.lowerer.query_interface_requirements(interface)
    }

    fn requirement_slot(
        &mut self,
        declaration: hir::InterfaceMethodReference,
    ) -> Result<scoop_identity::PersistentDispatchSlotId, Diagnostic> {
        let method = match declaration {
            hir::InterfaceMethodReference::Local(method) => method,
            hir::InterfaceMethodReference::Imported { slot, .. } => return Ok(slot),
        };
        let data = self.body_queries.data()?;
        crate::persistent_dispatch::interface_slot(
            self.lowerer,
            &data.functions,
            &data.accessors,
            method,
        )
        .map(|record| record.id())
        .map_err(|error| Diagnostic::at_file(error.file(), error.span(), error.to_string()))
    }

    fn resolve_type(
        &mut self,
        context: hir::SemanticContext,
        syntax: &scoop_ast::TypeRef,
    ) -> Result<hir::TypeId, Vec<Diagnostic>> {
        self.lowerer
            .type_query(context.into(), syntax.span, |lowerer| {
                lowerer.resolve_type_ref(syntax)
            })
    }

    fn resolve_type_reference(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        reference: scoop_identity::DecodedSignatureTypeKey,
        span: Span,
    ) -> Result<hir::TypeId, Vec<Diagnostic>> {
        self.query_type_reference(context, reference, span)
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
