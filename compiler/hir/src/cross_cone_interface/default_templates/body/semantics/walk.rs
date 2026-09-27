use scoop_identity::SignatureTypeKey;
use scoop_wire::{WireError, WirePath};

use super::{
    DefaultBodyOriginSiteV1, DefaultBodyProviderEnvelopeSemanticValidationError,
    DefaultBodyProviderTypeSiteV1, DefaultLocalFunctionSignatureAuthority,
};
use crate::{
    DefaultAnonymousFunctionV1, DefaultAppliedOptionV1, DefaultArrayAssemblyV1,
    DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingLeafV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBindingTemporaryV1,
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableRefV1,
    DefaultCallableReferenceV1, DefaultCaptureV1, DefaultCatchV1, DefaultConstructorRefV1,
    DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1, DefaultExpressionV1, DefaultFieldRefV1,
    DefaultForIterationPlanV1, DefaultIntegerArgumentsV1, DefaultIteratorConformanceV1,
    DefaultIteratorNextV1, DefaultLambdaV1, DefaultLiteralEqualityV1, DefaultLocalFunctionV1,
    DefaultMethodCalleeV1, DefaultPatternV1, DefaultStatementV1, DefaultTemplateProviderShapeV1,
    DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExportDefaultBodyV1, ExportDefinitionSourceV1, NominalInterfaceShapeAuthority,
};

mod expression;
mod nested;
mod nodes;
use nodes::{BodyNode, WorkItem};
mod origin;
mod semantics;
use semantics::SemanticValidation;
mod statement;

/// Reuses the full typed walk after the caller has bound every origin occurrence.
pub(super) fn validate_types<
    A: NominalInterfaceShapeAuthority<E> + DefaultLocalFunctionSignatureAuthority<E>,
    E,
>(
    body: &ExportDefaultBodyV1,
    provider: DefaultTemplateProviderShapeV1,
    authority: &mut A,

    path: &WirePath,
) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
    Validator {
        mode: SemanticValidation {
            scope: provider.signature_scope(),
            authority,
            error: std::marker::PhantomData,
        },

        path,
    }
    .run(body)
}

pub(super) fn visit_definition_sources<V, E>(
    body: &ExportDefaultBodyV1,
    visitor: &mut V,

    path: &WirePath,
) -> Result<(), E>
where
    V: FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1, &WirePath) -> Result<(), E>,
    E: From<WireError>,
{
    Validator {
        mode: origin::DefinitionSourceVisitor { visitor },

        path,
    }
    .run(body)
}

pub(super) fn visit_statement_sources<V, E>(
    statements: &[DefaultStatementV1],
    visitor: &mut V,
    path: &WirePath,
) -> Result<(), E>
where
    V: FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1, &WirePath) -> Result<(), E>,
    E: From<WireError>,
{
    Validator {
        mode: origin::DefinitionSourceVisitor { visitor },
        path,
    }
    .run_nodes(statements.iter().map(BodyNode::Statement))
}

pub(super) fn visit_fragment_sources<V, E>(
    fragment: &crate::ExportTemplateFragmentV1,
    visitor: &mut V,
    path: &WirePath,
) -> Result<(), E>
where
    V: FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1, &WirePath) -> Result<(), E>,
    E: From<WireError>,
{
    Validator {
        mode: origin::DefinitionSourceVisitor { visitor },
        path,
    }
    .run_nodes(
        fragment
            .statements()
            .iter()
            .map(BodyNode::Statement)
            .chain(fragment.results().iter().map(BodyNode::Expression)),
    )
}

pub(super) trait BodyWalkMode {
    type Error;

    fn resource(error: WireError) -> Self::Error;

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), Self::Error>;

    fn validate_local_function_signature(
        &mut self,
        function: &DefaultLocalFunctionV1,
        definition_origin: &ExportDefinitionSourceV1,

        path: &WirePath,
    ) -> Result<(), Self::Error>;

    fn validate_binder(
        &mut self,
        depth: u32,
        index: u32,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Self::Error>;

    fn visit_origin(
        &mut self,
        source: &ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,

        _path: &WirePath,
    ) -> Result<(), Self::Error>;
}

pub(super) struct Validator<'a, M> {
    mode: M,

    path: &'a WirePath,
}

impl<M> Validator<'_, M>
where
    M: BodyWalkMode,
{
    fn run(&mut self, body: &ExportDefaultBodyV1) -> Result<(), M::Error> {
        self.run_nodes(std::iter::once(BodyNode::Body(body)))
    }

    fn run_nodes<'body>(
        &mut self,
        nodes: impl DoubleEndedIterator<Item = BodyNode<'body>>,
    ) -> Result<(), M::Error> {
        let mut pending = Vec::new();
        for node in nodes.rev() {
            self.push_child(&mut pending, node)?;
        }

        while let Some(work) = pending.pop() {
            match work {
                WorkItem::Type {
                    signature,
                    site,
                    definition_origin,
                } => self.validate_type(signature, site, definition_origin)?,
                WorkItem::Body { node } => {
                    self.process_node(node, &mut pending)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn push_child<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        node: BodyNode<'body>,
    ) -> Result<(), M::Error> {
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Body { node });
        Ok(())
    }

    pub(super) fn push_type<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        signature: &'body SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Type {
            signature,
            site,
            definition_origin,
        });
        Ok(())
    }

    pub(super) fn push_binder<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u32,
        index: u32,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Body {
            node: BodyNode::Binder {
                depth,
                index,
                site,
                definition_origin,
            },
        });
        Ok(())
    }

    fn reserve_edge<T>(&mut self, pending: &mut Vec<T>) -> Result<(), M::Error> {
        scoop_wire::allocation::try_reserve(pending, 1, self.path).map_err(M::resource)
    }

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.mode
            .validate_type(signature, site, definition_origin, self.path)
    }

    fn process_node<'body>(
        &mut self,
        node: BodyNode<'body>,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match node {
            BodyNode::Body(body) => self.process_body(body, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, pending),
            BodyNode::Pattern {
                pattern,
                definition_origin,
            } => self.process_pattern(pattern, definition_origin, pending),
            BodyNode::AssignTarget {
                target,
                definition_origin,
            } => self.process_assign_target(target, definition_origin, pending),
            BodyNode::When {
                value,
                definition_origin,
            } => self.process_when(value, definition_origin, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, pending),
            BodyNode::WhenGuard {
                guard,
                definition_origin,
            } => self.process_when_guard(guard, definition_origin, pending),
            BodyNode::WhenFallback {
                fallback,
                definition_origin,
            } => self.process_when_fallback(fallback, definition_origin, pending),
            BodyNode::Try {
                value,
                definition_origin,
            } => self.process_try(value, definition_origin, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, pending),
            BodyNode::For {
                plan,
                definition_origin,
            } => self.process_for(plan, definition_origin, pending),
            BodyNode::BindingPlan {
                plan,
                definition_origin,
            } => self.process_binding_plan(plan, definition_origin, pending),
            BodyNode::BindingAction(action) => self.process_binding_action(action, pending),
            BodyNode::BindingShape {
                shape,
                definition_origin,
            } => self.process_binding_shape(shape, definition_origin, pending),
            BodyNode::BindingTemporary {
                temporary,
                definition_origin,
            } => self.push_type(
                pending,
                temporary.value_type(),
                DefaultBodyProviderTypeSiteV1::BindingTemporaryValue,
                definition_origin,
            ),
            BodyNode::BindingLeaf {
                leaf,
                definition_origin,
            } => self.push_type(
                pending,
                leaf.value_type(),
                DefaultBodyProviderTypeSiteV1::BindingLeafValue,
                definition_origin,
            ),
            BodyNode::BindingProjection {
                projection,
                definition_origin,
            } => self.process_binding_projection(projection, definition_origin, pending),
            BodyNode::IteratorConformance(conformance) => {
                self.process_iterator_conformance(conformance, pending)
            }
            BodyNode::IteratorNext(next) => self.process_iterator_next(next, pending),
            BodyNode::AppliedOption {
                option,
                definition_origin,
            } => self.process_applied_option(option, definition_origin, pending),
            BodyNode::LocalFunction {
                function,
                definition_origin,
            } => self.process_local_function(function, definition_origin, pending),
            BodyNode::Lambda {
                lambda,
                definition_origin,
            } => self.process_lambda(lambda, definition_origin, pending),
            BodyNode::AnonymousFunction {
                function,
                definition_origin,
            } => self.process_anonymous(function, definition_origin, pending),
            BodyNode::CallableReference {
                reference,
                definition_origin,
            } => self.process_callable_reference(reference, definition_origin, pending),
            BodyNode::Capture(capture) => self.process_capture(capture, pending),
            BodyNode::CallableRef {
                callable,
                definition_origin,
            } => self.process_callable_ref(callable, definition_origin, pending),
            BodyNode::BoundCallableRef {
                callable,
                definition_origin,
            } => self.process_bound_callable_ref(callable, definition_origin, pending),
            BodyNode::BoundCallableSource {
                source,
                definition_origin,
            } => self.process_bound_callable_source(source, definition_origin, pending),
            BodyNode::MethodCallee {
                callee,
                definition_origin,
            } => self.process_method_callee(callee, definition_origin, pending),
            BodyNode::ConstructorRef {
                constructor,
                definition_origin,
            } => self.push_type(
                pending,
                constructor.owner_type(),
                DefaultBodyProviderTypeSiteV1::ConstructorOwner,
                definition_origin,
            ),
            BodyNode::EnumVariantRef {
                variant,
                definition_origin,
            } => self.push_type(
                pending,
                variant.owner_type(),
                DefaultBodyProviderTypeSiteV1::EnumVariantOwner,
                definition_origin,
            ),
            BodyNode::EnumVariantFieldRef {
                field,
                definition_origin,
            } => self.push_type(
                pending,
                field.owner_type(),
                DefaultBodyProviderTypeSiteV1::EnumVariantFieldOwner,
                definition_origin,
            ),
            BodyNode::FieldRef {
                field,
                definition_origin,
            } => self.process_field_ref(field, definition_origin, pending),
            BodyNode::LiteralEquality {
                equality,
                definition_origin,
            } => self.process_literal_equality(equality, definition_origin, pending),
            BodyNode::ArrayAssembly {
                assembly,
                definition_origin,
            } => self.process_array_assembly(assembly, definition_origin, pending),
            BodyNode::IntegerArguments(arguments) => {
                self.process_integer_arguments(arguments, pending)
            }
            BodyNode::Origin { source, site } => self.mode.visit_origin(source, site, self.path),
            BodyNode::Binder {
                depth: binder_depth,
                index,
                site,
                definition_origin,
            } => self
                .mode
                .validate_binder(binder_depth, index, site, definition_origin),
        }
    }

    fn process_body<'body>(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_child(pending, BodyNode::Expression(body.value()))?;
        for statement in body.statements().iter().rev() {
            self.push_child(pending, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}
