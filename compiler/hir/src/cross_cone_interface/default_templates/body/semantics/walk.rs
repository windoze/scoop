use std::marker::PhantomData;

use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::{
    DefaultBodyOriginSiteV1, DefaultBodyProviderEnvelopeSemanticValidationError,
    DefaultBodyProviderTypeSiteV1,
};
use crate::{
    DefaultAnonymousFunctionV1, DefaultAppliedOptionV1, DefaultArrayAssemblyV1,
    DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingLeafV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBindingTemporaryV1,
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableRefV1,
    DefaultCallableReferenceV1, DefaultCaptureV1, DefaultCatchV1, DefaultConstructorRefV1,
    DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1, DefaultExpressionV1, DefaultFieldRefV1,
    DefaultForIterationPlanV1, DefaultIntegerArgumentsV1, DefaultIntegerOperationV1,
    DefaultIteratorConformanceV1, DefaultIteratorNextV1, DefaultLambdaV1, DefaultLiteralEqualityV1,
    DefaultLocalFunctionV1, DefaultMethodCalleeV1, DefaultPatternV1, DefaultStatementV1,
    DefaultTemplateProviderShapeV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenGuardV1, DefaultWhenV1, ExportDefaultBodyV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1,
    MeteredSignatureTypeSemanticError, NominalInterfaceShapeAuthority,
};

mod expression;
mod nested;
mod origin;
mod statement;

pub(super) fn validate<A, E>(
    body: &ExportDefaultBodyV1,
    provider: DefaultTemplateProviderShapeV1,
    authority: &mut A,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>>
where
    A: NominalInterfaceShapeAuthority<E> + ExportDefinitionSourceSemanticAuthority<E>,
{
    Validator {
        mode: SemanticValidation::<A, E> {
            scope: provider.signature_scope(),
            authority,
            error: PhantomData,
        },
        meter,
        path,
    }
    .run(body)
}

pub(super) fn visit_definition_sources<V, E>(
    body: &ExportDefaultBodyV1,
    visitor: &mut V,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), E>
where
    V: FnMut(
        &ExportDefinitionSourceV1,
        DefaultBodyOriginSiteV1,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), E>,
    E: From<WireError>,
{
    Validator {
        mode: origin::DefinitionSourceVisitor { visitor },
        meter,
        path,
    }
    .run(body)
}

pub(super) trait BodyWalkMode {
    type Error;

    fn resource(error: WireError) -> Self::Error;

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
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
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), Self::Error>;
}

struct SemanticValidation<'a, A, E> {
    scope: crate::SignatureBinderScopeV1,
    authority: &'a mut A,
    error: PhantomData<fn() -> E>,
}

impl<A, E> BodyWalkMode for SemanticValidation<'_, A, E>
where
    A: NominalInterfaceShapeAuthority<E> + ExportDefinitionSourceSemanticAuthority<E>,
{
    type Error = DefaultBodyProviderEnvelopeSemanticValidationError<E>;

    fn resource(error: WireError) -> Self::Error {
        DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error)
    }

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        match self.scope.validate_signature_semantics_metered(
            signature,
            self.authority,
            meter,
            path,
        ) {
            Ok(()) => Ok(()),
            Err(MeteredSignatureTypeSemanticError::Semantic(error)) => {
                Err(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
                    site,
                    definition_origin: Box::new(definition_origin.clone()),
                    error: Box::new(error),
                })
            }
            Err(MeteredSignatureTypeSemanticError::Resource(error)) => {
                Err(DefaultBodyProviderEnvelopeSemanticValidationError::Resource(error))
            }
        }
    }

    fn validate_binder(
        &mut self,
        depth: u32,
        index: u32,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
    ) -> Result<(), Self::Error> {
        self.scope
            .validate(&SignatureTypeKey::Binder { depth, index })
            .map_err(
                |error| DefaultBodyProviderEnvelopeSemanticValidationError::Binder {
                    site,
                    definition_origin: Box::new(definition_origin.clone()),
                    error,
                },
            )
    }

    fn visit_origin(
        &mut self,
        source: &ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), Self::Error> {
        source.validate_semantics(self.authority).map_err(|error| {
            DefaultBodyProviderEnvelopeSemanticValidationError::Origin {
                site,
                definition_origin: Box::new(source.clone()),
                error: Box::new(error),
            }
        })
    }
}

pub(super) struct Validator<'a, M> {
    mode: M,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
}

impl<M> Validator<'_, M>
where
    M: BodyWalkMode,
{
    fn run(&mut self, body: &ExportDefaultBodyV1) -> Result<(), M::Error> {
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(M::resource)?;
        pending.push(WorkItem::Body {
            node: BodyNode::Body(body),
            depth: 1,
        });

        while let Some(work) = pending.pop() {
            match work {
                WorkItem::Type {
                    signature,
                    site,
                    definition_origin,
                } => self.validate_type(signature, site, definition_origin)?,
                WorkItem::Body { node, depth } => {
                    self.enter_node(depth)?;
                    self.process_node(node, depth, &mut pending)?;
                }
            }
        }
        Ok(())
    }

    fn enter_node(&mut self, depth: u64) -> Result<(), M::Error> {
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(M::resource)?;
        self.meter.charge_nodes(1, self.path).map_err(M::resource)?;
        self.meter.charge_work(1, self.path).map_err(M::resource)
    }

    pub(super) fn push_child<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        parent_depth: u64,
        node: BodyNode<'body>,
    ) -> Result<(), M::Error> {
        let depth = parent_depth
            .checked_add(1)
            .ok_or_else(|| M::resource(integer_out_of_range(self.path)))?;
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(M::resource)?;
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Body { node, depth });
        Ok(())
    }

    pub(super) fn push_type<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        signature: &'body SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.meter
            .check_semantic_depth(1, self.path)
            .map_err(M::resource)?;
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
        self.meter
            .check_semantic_depth(1, self.path)
            .map_err(M::resource)?;
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Body {
            node: BodyNode::Binder {
                depth,
                index,
                site,
                definition_origin,
            },
            depth: 1,
        });
        Ok(())
    }

    fn reserve_edge<T>(&mut self, pending: &mut Vec<T>) -> Result<(), M::Error> {
        self.meter.charge_edges(1, self.path).map_err(M::resource)?;
        self.meter.charge_work(1, self.path).map_err(M::resource)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(M::resource)
    }

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        self.mode
            .validate_type(signature, site, definition_origin, self.meter, self.path)
    }

    fn process_node<'body>(
        &mut self,
        node: BodyNode<'body>,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match node {
            BodyNode::Body(body) => self.process_body(body, depth, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, depth, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, depth, pending),
            BodyNode::Pattern {
                pattern,
                definition_origin,
            } => self.process_pattern(pattern, definition_origin, depth, pending),
            BodyNode::AssignTarget {
                target,
                definition_origin,
            } => self.process_assign_target(target, definition_origin, depth, pending),
            BodyNode::When {
                value,
                definition_origin,
            } => self.process_when(value, definition_origin, depth, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, depth, pending),
            BodyNode::WhenGuard {
                guard,
                definition_origin,
            } => self.process_when_guard(guard, definition_origin, depth, pending),
            BodyNode::WhenFallback {
                fallback,
                definition_origin,
            } => self.process_when_fallback(fallback, definition_origin, depth, pending),
            BodyNode::Try {
                value,
                definition_origin,
            } => self.process_try(value, definition_origin, depth, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, depth, pending),
            BodyNode::For {
                plan,
                definition_origin,
            } => self.process_for(plan, definition_origin, depth, pending),
            BodyNode::BindingPlan {
                plan,
                definition_origin,
            } => self.process_binding_plan(plan, definition_origin, depth, pending),
            BodyNode::BindingAction(action) => self.process_binding_action(action, depth, pending),
            BodyNode::BindingShape {
                shape,
                definition_origin,
            } => self.process_binding_shape(shape, definition_origin, depth, pending),
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
                self.process_iterator_conformance(conformance, depth, pending)
            }
            BodyNode::IteratorNext(next) => self.process_iterator_next(next, depth, pending),
            BodyNode::AppliedOption {
                option,
                definition_origin,
            } => self.process_applied_option(option, definition_origin, depth, pending),
            BodyNode::LocalFunction {
                function,
                definition_origin,
            } => self.process_local_function(function, definition_origin, depth, pending),
            BodyNode::Lambda {
                lambda,
                definition_origin,
            } => self.process_lambda(lambda, definition_origin, depth, pending),
            BodyNode::AnonymousFunction {
                function,
                definition_origin,
            } => self.process_anonymous(function, definition_origin, depth, pending),
            BodyNode::CallableReference {
                reference,
                definition_origin,
            } => self.process_callable_reference(reference, definition_origin, depth, pending),
            BodyNode::Capture(capture) => self.process_capture(capture, depth, pending),
            BodyNode::CallableRef {
                callable,
                definition_origin,
            } => self.process_callable_ref(callable, definition_origin, pending),
            BodyNode::BoundCallableRef {
                callable,
                definition_origin,
            } => self.process_bound_callable_ref(callable, definition_origin, depth, pending),
            BodyNode::BoundCallableSource {
                source,
                definition_origin,
            } => self.process_bound_callable_source(source, definition_origin, depth, pending),
            BodyNode::MethodCallee {
                callee,
                definition_origin,
            } => self.process_method_callee(callee, definition_origin, depth, pending),
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
            } => self.process_literal_equality(equality, definition_origin, depth, pending),
            BodyNode::ArrayAssembly {
                assembly,
                definition_origin,
            } => self.process_array_assembly(assembly, definition_origin, depth, pending),
            BodyNode::IntegerOperation {
                operation,
                definition_origin,
            } => self.process_integer_operation(operation, definition_origin, depth, pending),
            BodyNode::IntegerArguments(arguments) => {
                self.process_integer_arguments(arguments, depth, pending)
            }
            BodyNode::Origin { source, site } => {
                self.mode.visit_origin(source, site, self.meter, self.path)
            }
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
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_child(pending, depth, BodyNode::Expression(body.value()))?;
        for statement in body.statements().iter().rev() {
            self.push_child(pending, depth, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum WorkItem<'a> {
    Body {
        node: BodyNode<'a>,
        depth: u64,
    },
    Type {
        signature: &'a SignatureTypeKey,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
}

#[derive(Clone, Copy)]
pub(super) enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    Pattern {
        pattern: &'a DefaultPatternV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    AssignTarget {
        target: &'a DefaultAssignTargetV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    When {
        value: &'a DefaultWhenV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    WhenArm(&'a DefaultWhenArmV1),
    WhenGuard {
        guard: &'a DefaultWhenGuardV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    WhenFallback {
        fallback: &'a DefaultWhenFallbackV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Try {
        value: &'a DefaultTryV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Catch(&'a DefaultCatchV1),
    For {
        plan: &'a DefaultForIterationPlanV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingPlan {
        plan: &'a DefaultBindingPlanV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingAction(&'a DefaultBindingActionV1),
    BindingShape {
        shape: &'a DefaultBindingShapeV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingTemporary {
        temporary: &'a DefaultBindingTemporaryV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingLeaf {
        leaf: &'a DefaultBindingLeafV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BindingProjection {
        projection: &'a DefaultBindingProjectionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    IteratorConformance(&'a DefaultIteratorConformanceV1),
    IteratorNext(&'a DefaultIteratorNextV1),
    AppliedOption {
        option: &'a DefaultAppliedOptionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    LocalFunction {
        function: &'a DefaultLocalFunctionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Lambda {
        lambda: &'a DefaultLambdaV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    AnonymousFunction {
        function: &'a DefaultAnonymousFunctionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    CallableReference {
        reference: &'a DefaultCallableReferenceV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Capture(&'a DefaultCaptureV1),
    CallableRef {
        callable: &'a DefaultCallableRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableRef {
        callable: &'a DefaultBoundCallableRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableSource {
        source: &'a DefaultBoundCallableSourceV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    MethodCallee {
        callee: &'a DefaultMethodCalleeV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    ConstructorRef {
        constructor: &'a DefaultConstructorRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    EnumVariantRef {
        variant: &'a DefaultEnumVariantRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    EnumVariantFieldRef {
        field: &'a DefaultEnumVariantFieldRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    FieldRef {
        field: &'a DefaultFieldRefV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    LiteralEquality {
        equality: &'a DefaultLiteralEqualityV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    ArrayAssembly {
        assembly: &'a DefaultArrayAssemblyV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    IntegerOperation {
        operation: &'a DefaultIntegerOperationV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    IntegerArguments(&'a DefaultIntegerArgumentsV1),
    Binder {
        depth: u32,
        index: u32,
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Origin {
        source: &'a ExportDefinitionSourceV1,
        site: DefaultBodyOriginSiteV1,
    },
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
