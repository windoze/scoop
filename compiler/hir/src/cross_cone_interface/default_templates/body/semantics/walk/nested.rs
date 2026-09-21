use scoop_identity::{OptionalSignatureType, SignatureTypeKey};

use crate::{
    DefaultAnonymousFunctionV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableBodyTypeArgumentsV1, DefaultCallableRefV1, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1, DefaultCaptureV1, DefaultFieldRefV1, DefaultLambdaV1,
    DefaultLiteralEqualityV1, DefaultLocalFunctionV1, DefaultMethodCalleeV1,
    ExportDefinitionSourceV1,
};

use super::{BodyNode, BodyWalkMode, Validator, WorkItem};
use crate::{DefaultBodyOriginSiteV1, DefaultBodyProviderTypeSiteV1};

impl<M> Validator<'_, M>
where
    M: BodyWalkMode,
{
    pub(super) fn process_local_function<'body>(
        &mut self,
        function: &'body DefaultLocalFunctionV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_captures(pending, depth, function.captures())?;
        self.meter.charge_edges(1, self.path).map_err(M::resource)?;
        self.meter.charge_work(1, self.path).map_err(M::resource)?;
        self.mode.validate_local_function_signature(
            function,
            definition_origin,
            self.meter,
            self.path,
        )
    }

    pub(super) fn process_lambda<'body>(
        &mut self,
        lambda: &'body DefaultLambdaV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.process_lexical_callable(
            lambda.function_type(),
            lambda.body_type_arguments(),
            lambda.captures(),
            definition_origin,
            depth,
            pending,
        )
    }

    pub(super) fn process_anonymous<'body>(
        &mut self,
        function: &'body DefaultAnonymousFunctionV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.process_lexical_callable(
            function.function_type(),
            function.body_type_arguments(),
            function.captures(),
            definition_origin,
            depth,
            pending,
        )
    }

    fn process_lexical_callable<'body>(
        &mut self,
        function_type: &'body SignatureTypeKey,
        body_type_arguments: &'body DefaultCallableBodyTypeArgumentsV1,
        captures: &'body [DefaultCaptureV1],
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_captures(pending, depth, captures)?;
        if let Some(arguments) = body_type_arguments.explicit_arguments() {
            for (index, argument) in arguments.iter().enumerate().rev() {
                self.push_type(
                    pending,
                    argument,
                    DefaultBodyProviderTypeSiteV1::NestedCallableBodyTypeArgument { index },
                    definition_origin,
                )?;
            }
        }
        self.push_type(
            pending,
            function_type,
            DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
            definition_origin,
        )
    }

    pub(super) fn process_callable_reference<'body>(
        &mut self,
        reference: &'body DefaultCallableReferenceV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_captures(pending, depth, reference.captures())?;
        self.push_callable_reference_target(pending, depth, reference.target(), definition_origin)?;
        self.push_type(
            pending,
            reference.function_type(),
            DefaultBodyProviderTypeSiteV1::NestedCallableFunction,
            definition_origin,
        )
    }

    fn push_callable_reference_target<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        target: &'body DefaultCallableReferenceTargetV1,
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), M::Error> {
        match target {
            DefaultCallableReferenceTargetV1::Named(callable)
            | DefaultCallableReferenceTargetV1::Local {
                callee: callable, ..
            } => self.push_child(
                pending,
                depth,
                BodyNode::CallableRef {
                    callable,
                    definition_origin,
                },
            ),
            DefaultCallableReferenceTargetV1::BoundMember { receiver, callee } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::MethodCallee {
                        callee,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
            DefaultCallableReferenceTargetV1::BoundExtension { receiver, callee } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableRef {
                        callable: callee,
                        definition_origin,
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
    ) -> Result<(), M::Error> {
        for capture in captures.iter().rev() {
            self.push_child(pending, depth, BodyNode::Capture(capture))?;
        }
        Ok(())
    }

    pub(super) fn process_capture<'body>(
        &mut self,
        capture: &'body DefaultCaptureV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_type(
            pending,
            capture.value_type(),
            DefaultBodyProviderTypeSiteV1::CaptureValue,
            capture.first_use_origin(),
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: capture.first_use_origin(),
                site: DefaultBodyOriginSiteV1::CaptureFirstUse,
            },
        )
    }

    pub(super) fn process_callable_ref<'body>(
        &mut self,
        callable: &'body DefaultCallableRefV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        for (index, argument) in callable.type_arguments().iter().enumerate().rev() {
            self.push_type(
                pending,
                argument,
                DefaultBodyProviderTypeSiteV1::CallableTypeArgument { index },
                definition_origin,
            )?;
        }
        match callable.owner() {
            OptionalSignatureType::Present(owner) => self.push_type(
                pending,
                owner,
                DefaultBodyProviderTypeSiteV1::CallableOwner,
                definition_origin,
            ),
            OptionalSignatureType::Absent => Ok(()),
        }
    }

    pub(super) fn process_bound_callable_ref<'body>(
        &mut self,
        callable: &'body DefaultBoundCallableRefV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        self.push_type(
            pending,
            callable.instantiated_signature(),
            DefaultBodyProviderTypeSiteV1::BoundCallableInstantiatedSignature,
            definition_origin,
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::BoundCallableSource {
                source: callable.source(),
                definition_origin,
            },
        )?;
        let receiver = callable.receiver_parameter();
        self.push_binder(
            pending,
            receiver.depth(),
            receiver.index(),
            DefaultBodyProviderTypeSiteV1::BoundCallableReceiverParameter,
            definition_origin,
        )
    }

    pub(super) fn process_bound_callable_source<'body>(
        &mut self,
        source: &'body DefaultBoundCallableSourceV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match source {
            DefaultBoundCallableSourceV1::Class { bound, callable } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::CallableRef {
                        callable,
                        definition_origin,
                    },
                )?;
                self.push_type(
                    pending,
                    bound,
                    DefaultBodyProviderTypeSiteV1::BoundCallableBound,
                    definition_origin,
                )
            }
            DefaultBoundCallableSourceV1::Interface { bound, .. } => self.push_type(
                pending,
                bound,
                DefaultBodyProviderTypeSiteV1::BoundCallableBound,
                definition_origin,
            ),
        }
    }

    pub(super) fn process_method_callee<'body>(
        &mut self,
        callee: &'body DefaultMethodCalleeV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match callee {
            DefaultMethodCalleeV1::Callable(callable) => self.push_child(
                pending,
                depth,
                BodyNode::CallableRef {
                    callable,
                    definition_origin,
                },
            ),
            DefaultMethodCalleeV1::Bound(callable) => self.push_child(
                pending,
                depth,
                BodyNode::BoundCallableRef {
                    callable,
                    definition_origin,
                },
            ),
            DefaultMethodCalleeV1::DerivedEquality { owner_type } => self.push_type(
                pending,
                owner_type,
                DefaultBodyProviderTypeSiteV1::DerivedEqualityOwner,
                definition_origin,
            ),
        }
    }

    pub(super) fn process_field_ref<'body>(
        &mut self,
        field: &'body DefaultFieldRefV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        match field {
            DefaultFieldRefV1::Struct { owner_type, .. }
            | DefaultFieldRefV1::Class { owner_type, .. } => self.push_type(
                pending,
                owner_type,
                DefaultBodyProviderTypeSiteV1::FieldOwner,
                definition_origin,
            ),
            DefaultFieldRefV1::Tuple { .. } => Ok(()),
        }
    }

    pub(super) fn process_literal_equality<'body>(
        &mut self,
        equality: &'body DefaultLiteralEqualityV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), M::Error> {
        let target = match equality {
            DefaultLiteralEqualityV1::Integer { target, .. }
            | DefaultLiteralEqualityV1::Ordinary { target } => target,
        };
        self.push_child(
            pending,
            depth,
            BodyNode::CallableRef {
                callable: target,
                definition_origin,
            },
        )
    }
}
