use scoop_identity::OptionalSignatureType;

use super::super::ExportDefaultReferenceClosureValidationError;
use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::{BodyNode, Validator, WorkItem};
use crate::{
    DefaultAnonymousFunctionV1, DefaultBodyProviderTypeSiteV1, DefaultBoundCallableRefV1,
    DefaultBoundCallableSourceV1, DefaultCallableBodyTypeArgumentsV1, DefaultCallableRefV1,
    DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1, DefaultCaptureV1,
    DefaultFieldRefV1, DefaultLambdaV1, DefaultLiteralEqualityV1, DefaultLocalFunctionV1,
    DefaultMethodCalleeV1, ExportDefinitionSourceV1,
};

impl Validator<'_> {
    pub(super) fn process_local_function<'body>(
        &mut self,
        function: &'body DefaultLocalFunctionV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_captures(pending, depth, function.captures())?;
        self.push_type(
            pending,
            function.function_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
        )?;
        self.push_callable(
            pending,
            CallableTargetView::LocalFunction(function.declaration()),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Statement,
        )
    }

    pub(super) fn process_lambda<'body>(
        &mut self,
        lambda: &'body DefaultLambdaV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_lexical_callable_shape(
            pending,
            depth,
            lambda.function_type(),
            lambda.body_type_arguments(),
            lambda.captures(),
            origin,
        )?;
        self.push_callable(
            pending,
            CallableTargetView::Lambda(lambda.body()),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    pub(super) fn process_anonymous<'body>(
        &mut self,
        function: &'body DefaultAnonymousFunctionV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_lexical_callable_shape(
            pending,
            depth,
            function.function_type(),
            function.body_type_arguments(),
            function.captures(),
            origin,
        )?;
        self.push_callable(
            pending,
            CallableTargetView::AnonymousFunction(function.body()),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    fn push_lexical_callable_shape<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        function_type: &'body scoop_identity::SignatureTypeKey,
        body_type_arguments: &'body DefaultCallableBodyTypeArgumentsV1,
        captures: &'body [DefaultCaptureV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_captures(pending, depth, captures)?;
        if let Some(arguments) = body_type_arguments.explicit_arguments() {
            for (index, argument) in arguments.iter().enumerate().rev() {
                self.push_type(
                    pending,
                    argument,
                    origin,
                    DefaultBodyProviderTypeSiteV1::NestedCallableBodyTypeArgument { index },
                )?;
            }
        }
        self.push_type(
            pending,
            function_type,
            origin,
            DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
        )
    }

    pub(super) fn process_callable_reference<'body>(
        &mut self,
        reference: &'body DefaultCallableReferenceV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_captures(pending, depth, reference.captures())?;
        self.push_callable_reference_target_shape(pending, depth, reference.target(), origin)?;
        self.push_type(
            pending,
            reference.function_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
        )?;
        self.push_callable(
            pending,
            CallableTargetView::CallableReference(reference.invoke()),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    fn push_callable_reference_target_shape<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        target: &'body DefaultCallableReferenceTargetV1,
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match target {
            DefaultCallableReferenceTargetV1::Named(callable)
            | DefaultCallableReferenceTargetV1::Local {
                callee: callable, ..
            } => self.push_child(pending, depth, BodyNode::CallableShape { callable, origin }),
            DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::MethodCalleeShape { callee, origin },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableShape {
                        callable: callee,
                        origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
        }
    }

    fn push_captures<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        captures: &'body [DefaultCaptureV1],
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for capture in captures.iter().rev() {
            self.push_child(pending, depth, BodyNode::Capture(capture))?;
        }
        Ok(())
    }

    pub(super) fn process_capture<'body>(
        &mut self,
        capture: &'body DefaultCaptureV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_type(
            pending,
            capture.value_type(),
            capture.first_use_origin(),
            DefaultBodyProviderTypeSiteV1::CaptureValue,
        )
    }

    pub(super) fn process_callable_use<'body>(
        &mut self,
        callable: &'body DefaultCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(pending, depth, BodyNode::CallableShape { callable, origin })?;
        self.push_callable(
            pending,
            CallableTargetView::Callable(callable),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    pub(super) fn process_callable_shape<'body>(
        &mut self,
        callable: &'body DefaultCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for (index, argument) in callable.type_arguments().iter().enumerate().rev() {
            self.push_type(
                pending,
                argument,
                origin,
                DefaultBodyProviderTypeSiteV1::CallableTypeArgument { index },
            )?;
        }
        match callable.owner() {
            OptionalSignatureType::Present(owner) => self.push_type(
                pending,
                owner,
                origin,
                DefaultBodyProviderTypeSiteV1::CallableOwner,
            ),
            OptionalSignatureType::Absent => Ok(()),
        }
    }

    pub(super) fn process_bound_callable_use<'body>(
        &mut self,
        callable: &'body DefaultBoundCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(
            pending,
            depth,
            BodyNode::BoundCallableShape { callable, origin },
        )?;
        self.push_callable(
            pending,
            CallableTargetView::Bound(callable),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    pub(super) fn process_bound_callable_shape<'body>(
        &mut self,
        callable: &'body DefaultBoundCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_type(
            pending,
            callable.instantiated_signature(),
            origin,
            DefaultBodyProviderTypeSiteV1::BoundCallableInstantiatedSignature,
        )?;
        match callable.source() {
            DefaultBoundCallableSourceV1::Class { bound, callable } => {
                self.push_child(pending, depth, BodyNode::CallableShape { callable, origin })?;
                self.push_type(
                    pending,
                    bound,
                    origin,
                    DefaultBodyProviderTypeSiteV1::BoundCallableBound,
                )
            }
            DefaultBoundCallableSourceV1::Interface { bound, .. } => self.push_type(
                pending,
                bound,
                origin,
                DefaultBodyProviderTypeSiteV1::BoundCallableBound,
            ),
        }
    }

    pub(super) fn process_method_callee<'body>(
        &mut self,
        callee: &'body DefaultMethodCalleeV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match callee {
            DefaultMethodCalleeV1::Callable(callable) => {
                self.push_child(pending, depth, BodyNode::CallableUse { callable, origin })
            }
            DefaultMethodCalleeV1::Bound(callable) => self.push_child(
                pending,
                depth,
                BodyNode::BoundCallableUse { callable, origin },
            ),
            DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::DerivedEqualityOwner,
                )?;
                self.push_callable(
                    pending,
                    CallableTargetView::DerivedEquality(owner_type),
                    origin,
                    ExportDefaultReferenceOccurrenceSiteV1::Expression,
                )
            }
        }
    }

    pub(super) fn process_method_callee_shape<'body>(
        &mut self,
        callee: &'body DefaultMethodCalleeV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match callee {
            DefaultMethodCalleeV1::Callable(callable) => {
                self.push_child(pending, depth, BodyNode::CallableShape { callable, origin })
            }
            DefaultMethodCalleeV1::Bound(callable) => self.push_child(
                pending,
                depth,
                BodyNode::BoundCallableShape { callable, origin },
            ),
            DefaultMethodCalleeV1::DerivedEquality { owner_type } => self.push_type(
                pending,
                owner_type,
                origin,
                DefaultBodyProviderTypeSiteV1::DerivedEqualityOwner,
            ),
        }
    }

    pub(super) fn process_constructor_use<'body>(
        &mut self,
        target: ConstructorTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let owner_type = match target {
            ConstructorTargetView::Constructor(constructor) => constructor.owner_type(),
            ConstructorTargetView::Variant(variant) => variant.owner_type(),
        };
        self.push_type(
            pending,
            owner_type,
            origin,
            DefaultBodyProviderTypeSiteV1::ConstructorOwner,
        )?;
        self.push_constructor(pending, target, origin, site)
    }

    pub(super) fn process_field_use<'body>(
        &mut self,
        target: FieldTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let owner_type = match target {
            FieldTargetView::Field(DefaultFieldRefV1::Struct { owner_type, .. })
            | FieldTargetView::Field(DefaultFieldRefV1::Class { owner_type, .. })
            | FieldTargetView::Struct { owner_type, .. } => Some(owner_type),
            FieldTargetView::Field(DefaultFieldRefV1::Tuple { .. }) => None,
        };
        if let Some(owner_type) = owner_type {
            self.push_type(
                pending,
                owner_type,
                origin,
                DefaultBodyProviderTypeSiteV1::FieldOwner,
            )?;
        }
        self.push_field(pending, target, origin, site)
    }

    pub(super) fn process_literal_equality<'body>(
        &mut self,
        equality: &'body DefaultLiteralEqualityV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let callable = match equality {
            DefaultLiteralEqualityV1::Integer { target, .. }
            | DefaultLiteralEqualityV1::Ordinary { target } => target,
        };
        self.push_child(pending, depth, BodyNode::CallableUse { callable, origin })
    }
}
