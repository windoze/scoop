use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WireErrorKind};

use super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceClosureValidationError,
    ExportDefaultReferenceOccurrenceSiteV1, FieldTargetView, Validator,
};
use crate::{
    DefaultAnonymousFunctionV1, DefaultAppliedOptionV1, DefaultArrayAssemblyV1,
    DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBoundCallableRefV1,
    DefaultCallableRefV1, DefaultCallableReferenceV1, DefaultCaptureV1, DefaultCatchV1,
    DefaultExpressionV1, DefaultForIterationPlanV1, DefaultIntegerArgumentsV1,
    DefaultIntegerOperationV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    DefaultLambdaV1, DefaultLiteralEqualityV1, DefaultLocalFunctionV1, DefaultMethodCalleeV1,
    DefaultPatternV1, DefaultStatementV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenGuardV1, DefaultWhenV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
};

mod expression;
mod nested;
mod statement;

impl Validator<'_> {
    pub(super) fn walk_body(
        &mut self,
        body: &ExportDefaultBodyV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let mut pending = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut pending, 1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        pending.push(WorkItem::Body {
            node: BodyNode::Body(body),
            depth: 1,
        });

        while let Some(work) = pending.pop() {
            match work {
                WorkItem::Body { node, depth } => {
                    self.enter_node(depth)?;
                    self.process_node(node, depth, &mut pending)?;
                }
                WorkItem::Type {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.match_type(target, origin, site)?;
                }
                WorkItem::Callable {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.observe_callable(target, origin, site)?;
                }
                WorkItem::Constructor {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.observe_constructor(target, origin, site)?;
                }
                WorkItem::Global {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.observe_global(target, origin, site)?;
                }
                WorkItem::Singleton {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.observe_singleton(target, origin, site)?;
                }
                WorkItem::Field {
                    target,
                    origin,
                    site,
                } => {
                    self.enter_scheduled_leaf()?;
                    self.observe_field(target, origin, site)?;
                }
            }
        }
        Ok(())
    }

    fn process_node<'body>(
        &mut self,
        node: BodyNode<'body>,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match node {
            BodyNode::Body(body) => self.process_body(body, depth, pending),
            BodyNode::Statement(statement) => self.process_statement(statement, depth, pending),
            BodyNode::Expression(expression) => self.process_expression(expression, depth, pending),
            BodyNode::Pattern { pattern, origin } => {
                self.process_pattern(pattern, origin, depth, pending)
            }
            BodyNode::AssignTarget { target, origin } => {
                self.process_assign_target(target, origin, depth, pending)
            }
            BodyNode::When { value, origin } => self.process_when(value, origin, depth, pending),
            BodyNode::WhenArm(arm) => self.process_when_arm(arm, depth, pending),
            BodyNode::WhenGuard(guard) => self.process_when_guard(guard, depth, pending),
            BodyNode::WhenFallback { fallback, origin } => {
                self.process_when_fallback(fallback, origin, depth, pending)
            }
            BodyNode::Try(value) => self.process_try(value, depth, pending),
            BodyNode::Catch(catch) => self.process_catch(catch, depth, pending),
            BodyNode::For { plan, origin } => self.process_for(plan, origin, depth, pending),
            BodyNode::BindingPlan { plan, origin } => {
                self.process_binding_plan(plan, origin, depth, pending)
            }
            BodyNode::BindingAction(action) => self.process_binding_action(action, depth, pending),
            BodyNode::BindingShape { shape, origin } => {
                self.process_binding_shape(shape, origin, depth, pending)
            }
            BodyNode::BindingProjection { projection, origin } => {
                self.process_binding_projection(projection, origin, depth, pending)
            }
            BodyNode::BindingTemporary { value_type, origin } => self.push_type(
                pending,
                value_type,
                origin,
                crate::DefaultBodyProviderTypeSiteV1::BindingTemporaryValue,
            ),
            BodyNode::BindingLeaf { value_type, origin } => self.push_type(
                pending,
                value_type,
                origin,
                crate::DefaultBodyProviderTypeSiteV1::BindingLeafValue,
            ),
            BodyNode::IteratorConformance(conformance) => {
                self.process_iterator_conformance(conformance, depth, pending)
            }
            BodyNode::IteratorNext(next) => self.process_iterator_next(next, depth, pending),
            BodyNode::AppliedOption { option, origin } => {
                self.process_applied_option(option, origin, depth, pending)
            }
            BodyNode::LocalFunction { function, origin } => {
                self.process_local_function(function, origin, depth, pending)
            }
            BodyNode::Lambda { lambda, origin } => {
                self.process_lambda(lambda, origin, depth, pending)
            }
            BodyNode::Anonymous { function, origin } => {
                self.process_anonymous(function, origin, depth, pending)
            }
            BodyNode::CallableReference { reference, origin } => {
                self.process_callable_reference(reference, origin, depth, pending)
            }
            BodyNode::Capture(capture) => self.process_capture(capture, pending),
            BodyNode::CallableUse { callable, origin } => {
                self.process_callable_use(callable, origin, depth, pending)
            }
            BodyNode::CallableShape { callable, origin } => {
                self.process_callable_shape(callable, origin, pending)
            }
            BodyNode::BoundCallableUse { callable, origin } => {
                self.process_bound_callable_use(callable, origin, depth, pending)
            }
            BodyNode::BoundCallableShape { callable, origin } => {
                self.process_bound_callable_shape(callable, origin, depth, pending)
            }
            BodyNode::MethodCallee { callee, origin } => {
                self.process_method_callee(callee, origin, depth, pending)
            }
            BodyNode::MethodCalleeShape { callee, origin } => {
                self.process_method_callee_shape(callee, origin, depth, pending)
            }
            BodyNode::ConstructorUse {
                target,
                origin,
                site,
            } => self.process_constructor_use(target, origin, site, pending),
            BodyNode::VariantFieldShape {
                field,
                origin,
                site,
            } => self.push_type(pending, field.owner_type(), origin, site),
            BodyNode::FieldUse {
                target,
                origin,
                site,
            } => self.process_field_use(target, origin, site, pending),
            BodyNode::LiteralEquality { equality, origin } => {
                self.process_literal_equality(equality, origin, depth, pending)
            }
            BodyNode::ArrayAssembly { assembly, origin } => {
                self.process_array_assembly(assembly, origin, depth, pending)
            }
            BodyNode::IntegerOperation { operation, origin } => {
                self.process_integer_operation(operation, origin, depth, pending)
            }
            BodyNode::IntegerArguments(arguments) => {
                self.process_integer_arguments(arguments, depth, pending)
            }
        }
    }

    fn process_body<'body>(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(pending, depth, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, depth, body.statements())
    }

    pub(super) fn push_child<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        parent_depth: u64,
        node: BodyNode<'body>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let depth = parent_depth.checked_add(1).ok_or_else(|| {
            ExportDefaultReferenceClosureValidationError::Resource(integer_out_of_range(self.path))
        })?;
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.reserve_edge(pending)?;
        pending.push(WorkItem::Body { node, depth });
        Ok(())
    }

    pub(super) fn push_type<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: &'body SignatureTypeKey,
        origin: &'body ExportDefinitionSourceV1,
        site: crate::DefaultBodyProviderTypeSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Type {
                target,
                origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::BodyType(site),
            },
        )
    }

    pub(super) fn push_callable<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: CallableTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Callable {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_constructor<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: ConstructorTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Constructor {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_global<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: PersistentPropertyId,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Global {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_singleton<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: PersistentObjectValueId,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Singleton {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_field<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        target: FieldTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_leaf(
            pending,
            WorkItem::Field {
                target,
                origin,
                site,
            },
        )
    }

    fn push_leaf<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        work: WorkItem<'body>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.meter
            .check_semantic_depth(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.reserve_edge(pending)?;
        pending.push(work);
        Ok(())
    }

    fn reserve_edge<T>(
        &mut self,
        pending: &mut Vec<T>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.meter
            .charge_edges(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)
    }
}

#[derive(Clone, Copy)]
pub(super) enum WorkItem<'a> {
    Body {
        node: BodyNode<'a>,
        depth: u64,
    },
    Type {
        target: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Callable {
        target: CallableTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Constructor {
        target: ConstructorTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Global {
        target: PersistentPropertyId,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Singleton {
        target: PersistentObjectValueId,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    Field {
        target: FieldTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
}

#[derive(Clone, Copy)]
pub(super) enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    Pattern {
        pattern: &'a DefaultPatternV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    AssignTarget {
        target: &'a DefaultAssignTargetV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    When {
        value: &'a DefaultWhenV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    WhenArm(&'a DefaultWhenArmV1),
    WhenGuard(&'a DefaultWhenGuardV1),
    WhenFallback {
        fallback: &'a DefaultWhenFallbackV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Try(&'a DefaultTryV1),
    Catch(&'a DefaultCatchV1),
    For {
        plan: &'a DefaultForIterationPlanV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingPlan {
        plan: &'a DefaultBindingPlanV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingAction(&'a DefaultBindingActionV1),
    BindingShape {
        shape: &'a DefaultBindingShapeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingProjection {
        projection: &'a DefaultBindingProjectionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingTemporary {
        value_type: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
    },
    BindingLeaf {
        value_type: &'a SignatureTypeKey,
        origin: &'a ExportDefinitionSourceV1,
    },
    IteratorConformance(&'a DefaultIteratorConformanceV1),
    IteratorNext(&'a DefaultIteratorNextV1),
    AppliedOption {
        option: &'a DefaultAppliedOptionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    LocalFunction {
        function: &'a DefaultLocalFunctionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Lambda {
        lambda: &'a DefaultLambdaV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Anonymous {
        function: &'a DefaultAnonymousFunctionV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    CallableReference {
        reference: &'a DefaultCallableReferenceV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    Capture(&'a DefaultCaptureV1),
    CallableUse {
        callable: &'a DefaultCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    CallableShape {
        callable: &'a DefaultCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableUse {
        callable: &'a DefaultBoundCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    BoundCallableShape {
        callable: &'a DefaultBoundCallableRefV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    MethodCallee {
        callee: &'a DefaultMethodCalleeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    MethodCalleeShape {
        callee: &'a DefaultMethodCalleeV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    ConstructorUse {
        target: ConstructorTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    VariantFieldShape {
        field: &'a crate::DefaultEnumVariantFieldRefV1,
        origin: &'a ExportDefinitionSourceV1,
        site: crate::DefaultBodyProviderTypeSiteV1,
    },
    FieldUse {
        target: FieldTargetView<'a>,
        origin: &'a ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    },
    LiteralEquality {
        equality: &'a DefaultLiteralEqualityV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    ArrayAssembly {
        assembly: &'a DefaultArrayAssemblyV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    IntegerOperation {
        operation: &'a DefaultIntegerOperationV1,
        origin: &'a ExportDefinitionSourceV1,
    },
    IntegerArguments(&'a DefaultIntegerArgumentsV1),
}

fn integer_out_of_range(path: &scoop_wire::WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
