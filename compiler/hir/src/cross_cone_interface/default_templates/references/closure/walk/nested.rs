use scoop_identity::OptionalSignatureType;

use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork};
use crate::{
    DefaultAnonymousFunctionV1, DefaultBodyProviderTypeSiteV1, DefaultBoundCallableRefV1,
    DefaultBoundCallableSourceV1, DefaultCallableBodyTypeArgumentsV1, DefaultCallableRefV1,
    DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1, DefaultCaptureV1,
    DefaultFieldRefV1, DefaultLambdaV1, DefaultLiteralEqualityV1, DefaultLocalFunctionV1,
    DefaultMethodCalleeV1, ExportDefinitionSourceV1,
};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn process_local_function(
        &mut self,
        function: &'body DefaultLocalFunctionV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_captures(pending, function.captures())?;
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

    pub(super) fn process_lambda(
        &mut self,
        lambda: &'body DefaultLambdaV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_lexical_callable_shape(
            pending,
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

    pub(super) fn process_anonymous(
        &mut self,
        function: &'body DefaultAnonymousFunctionV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_lexical_callable_shape(
            pending,
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

    fn push_lexical_callable_shape(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        function_type: &'body scoop_identity::SignatureTypeKey,
        body_type_arguments: &'body DefaultCallableBodyTypeArgumentsV1,
        captures: &'body [DefaultCaptureV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        self.push_captures(pending, captures)?;
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

    pub(super) fn process_callable_reference(
        &mut self,
        reference: &'body DefaultCallableReferenceV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_captures(pending, reference.captures())?;
        self.push_callable_reference_target_shape(pending, reference.target(), origin)?;
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

    fn push_callable_reference_target_shape(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: &'body DefaultCallableReferenceTargetV1,
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        match target {
            DefaultCallableReferenceTargetV1::Named(callable)
            | DefaultCallableReferenceTargetV1::Local {
                callee: callable, ..
            } => self.push_child(pending, BodyNode::CallableShape { callable, origin }),
            DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                self.push_child(pending, BodyNode::MethodCalleeShape { callee, origin })?;
                self.push_child(pending, BodyNode::Expression(receiver))
            }
            DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                self.push_child(
                    pending,
                    BodyNode::CallableShape {
                        callable: callee,
                        origin,
                    },
                )?;
                self.push_child(pending, BodyNode::Expression(receiver))
            }
        }
    }

    fn push_captures(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        captures: &'body [DefaultCaptureV1],
    ) -> Result<(), V::Error> {
        for capture in captures.iter().rev() {
            self.push_child(pending, BodyNode::Capture(capture))?;
        }
        Ok(())
    }

    pub(super) fn process_capture(
        &mut self,
        capture: &'body DefaultCaptureV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_type(
            pending,
            capture.value_type(),
            capture.first_use_origin(),
            DefaultBodyProviderTypeSiteV1::CaptureValue,
        )
    }

    pub(super) fn process_callable_use(
        &mut self,
        callable: &'body DefaultCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(pending, BodyNode::CallableShape { callable, origin })?;
        self.push_callable(
            pending,
            CallableTargetView::Callable(callable),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    pub(super) fn process_callable_shape(
        &mut self,
        callable: &'body DefaultCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
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

    pub(super) fn process_bound_callable_use(
        &mut self,
        callable: &'body DefaultBoundCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(pending, BodyNode::BoundCallableShape { callable, origin })?;
        self.push_callable(
            pending,
            CallableTargetView::Bound(callable),
            origin,
            ExportDefaultReferenceOccurrenceSiteV1::Expression,
        )
    }

    pub(super) fn process_bound_callable_shape(
        &mut self,
        callable: &'body DefaultBoundCallableRefV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_type(
            pending,
            callable.receiver_type(),
            origin,
            DefaultBodyProviderTypeSiteV1::BoundCallableReceiverType,
        )?;
        self.push_type(
            pending,
            callable.instantiated_signature(),
            origin,
            DefaultBodyProviderTypeSiteV1::BoundCallableInstantiatedSignature,
        )?;
        match callable.source() {
            DefaultBoundCallableSourceV1::Class { bound, callable } => {
                self.push_child(pending, BodyNode::CallableShape { callable, origin })?;
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

    pub(super) fn process_method_callee(
        &mut self,
        callee: &'body DefaultMethodCalleeV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match callee {
            DefaultMethodCalleeV1::Callable(callable) => {
                self.push_child(pending, BodyNode::CallableUse { callable, origin })
            }
            DefaultMethodCalleeV1::Bound(callable) => {
                self.push_child(pending, BodyNode::BoundCallableUse { callable, origin })
            }
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

    pub(super) fn process_method_callee_shape(
        &mut self,
        callee: &'body DefaultMethodCalleeV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match callee {
            DefaultMethodCalleeV1::Callable(callable) => {
                self.push_child(pending, BodyNode::CallableShape { callable, origin })
            }
            DefaultMethodCalleeV1::Bound(callable) => {
                self.push_child(pending, BodyNode::BoundCallableShape { callable, origin })
            }
            DefaultMethodCalleeV1::DerivedEquality { owner_type } => self.push_type(
                pending,
                owner_type,
                origin,
                DefaultBodyProviderTypeSiteV1::DerivedEqualityOwner,
            ),
        }
    }

    pub(super) fn process_constructor_use(
        &mut self,
        target: ConstructorTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
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

    pub(super) fn process_field_use(
        &mut self,
        target: FieldTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        let owner_type = match target {
            FieldTargetView::Field(DefaultFieldRefV1::Struct { owner_type, .. })
            | FieldTargetView::Field(DefaultFieldRefV1::Class { owner_type, .. })
            | FieldTargetView::Struct { owner_type, .. }
            | FieldTargetView::Class { owner_type, .. } => Some(owner_type),
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

    pub(super) fn process_literal_equality(
        &mut self,
        equality: &'body DefaultLiteralEqualityV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        let callable = match equality {
            DefaultLiteralEqualityV1::Char
            | DefaultLiteralEqualityV1::Integer { .. }
            | DefaultLiteralEqualityV1::Float { .. } => {
                return Ok(());
            }
            DefaultLiteralEqualityV1::Ordinary { target } => target,
        };
        self.push_child(pending, BodyNode::CallableUse { callable, origin })
    }
}
