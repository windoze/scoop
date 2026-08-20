use scoop_identity::CallableTemplateOrigin;

use super::BodyProjection;
use crate::production::default_templates::DefaultBodyProjectionError;
use crate::{
    DefaultConstructorRefV1, DefaultExpressionKindV1, DefaultMethodCalleeV1, Expr,
    ImportedDependencyCallableUseId, ImportedDependencyDispatch, SourceCallReceiver, TypeId,
};

impl BodyProjection<'_, '_> {
    pub(super) fn source_call(
        &mut self,
        target: crate::Callable,
        args: &[Expr],
        receiver: SourceCallReceiver<TypeId>,
    ) -> Result<DefaultExpressionKindV1, DefaultBodyProjectionError> {
        let export = self.entities.export();
        let function = export.callable_function(target);
        let local = export.local_functions.iter().find_map(|(id, local)| {
            local
                .source()
                .is_some_and(|(source, _)| source == function)
                .then_some((id, local.captures.len()))
        });
        let callee = self.entities.callable(target, self.binders)?;
        if let Some((local, count)) = local {
            let declaration = self.local_function_declaration(local)?;
            return self.local_call(declaration, callee, args, count);
        }
        Ok(DefaultExpressionKindV1::Call {
            callee,
            receiver: receiver.try_map(|ty| self.type_key(ty))?,
            arguments: self.expressions(args)?,
        })
    }

    fn local_call(
        &mut self,
        declaration: CallableTemplateOrigin,
        callee: crate::DefaultCallableRefV1,
        args: &[Expr],
        capture_count: usize,
    ) -> Result<DefaultExpressionKindV1, DefaultBodyProjectionError> {
        let (captures, arguments) = args.split_at(capture_count);
        Ok(DefaultExpressionKindV1::LocalFunctionCall {
            declaration,
            callee,
            captures: self.expressions(captures)?,
            arguments: self.expressions(arguments)?,
        })
    }

    pub(super) fn imported_generic_call(
        &mut self,
        application: crate::ImportedGenericCallableApplicationId,
        args: &[Expr],
        receiver: SourceCallReceiver<TypeId>,
        kind: crate::MemberCallKind,
    ) -> Result<DefaultExpressionKindV1, DefaultBodyProjectionError> {
        let callee = self
            .entities
            .imported_generic_callable(application, self.binders)?;
        let export = self.entities.export();
        let application = &export.imported_generic_applications[application];
        let template = &export.imported_generic_templates[application.template];
        if matches!(
            application.arguments,
            crate::ImportedCallableArguments::Method { .. }
        ) {
            let (receiver, arguments) = args
                .split_first()
                .ok_or(DefaultBodyProjectionError::MissingMethodReceiver)?;
            let receiver = Box::new(self.expression(receiver)?);
            let callee = DefaultMethodCalleeV1::Callable(callee);
            let arguments = self.expressions(arguments)?;
            return Ok(match kind {
                crate::MemberCallKind::Ordinary => DefaultExpressionKindV1::MethodCall {
                    receiver,
                    callee,
                    arguments,
                },
                crate::MemberCallKind::DirectSuper => {
                    DefaultExpressionKindV1::DirectSuperMethodCall {
                        receiver,
                        callee,
                        arguments,
                    }
                }
            });
        }
        match &template.declaration {
            crate::ImportedCallableTemplateOrigin::Generic(_)
            | crate::ImportedCallableTemplateOrigin::Intrinsic(_)
            | crate::ImportedCallableTemplateOrigin::Closure { .. }
            | crate::ImportedCallableTemplateOrigin::Initialization { .. }
            | crate::ImportedCallableTemplateOrigin::ExtensionAccessor(_)
            | crate::ImportedCallableTemplateOrigin::Nominal { .. } => {
                Ok(DefaultExpressionKindV1::Call {
                    callee,
                    receiver: receiver.try_map(|ty| self.type_key(ty))?,
                    arguments: self.expressions(args)?,
                })
            }
            crate::ImportedCallableTemplateOrigin::Local { descriptor, .. } => self.local_call(
                descriptor.declaration(),
                callee,
                args,
                descriptor.capture_count() as usize,
            ),
        }
    }

    pub(super) fn imported_call(
        &mut self,
        callee: ImportedDependencyCallableUseId,
        args: &[Expr],
        receiver: SourceCallReceiver<TypeId>,
        result_type: TypeId,
    ) -> Result<DefaultExpressionKindV1, DefaultBodyProjectionError> {
        let selected = self.entities.imported_dependency_source(callee)?;
        let direct_super = self.entities.export().imported_dependency_callables[callee].dispatch()
            == ImportedDependencyDispatch::Direct
            && selected.interface().modality() != crate::CallableModalityV1::Final;
        let declaration = selected.interface().declaration();
        let owner_type = selected.interface().result().clone();
        let callable = match declaration {
            CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::GenericFunction(_)
            | CallableTemplateOrigin::Accessor(_) => {
                self.entities.imported_dependency_callable(callee)?
            }
            CallableTemplateOrigin::Constructor(declaration) => {
                let constructor =
                    self.entities
                        .imported_constructor(declaration, result_type, self.binders)?;
                let arguments = self.expressions(args)?;
                return Ok(match constructor {
                    constructor @ DefaultConstructorRefV1::Struct { .. } => {
                        DefaultExpressionKindV1::StructInit {
                            constructor,
                            arguments,
                        }
                    }
                    constructor @ DefaultConstructorRefV1::Class { .. } => {
                        DefaultExpressionKindV1::ClassInit {
                            constructor,
                            arguments,
                        }
                    }
                    DefaultConstructorRefV1::Variant {
                        declaration,
                        owner_type,
                    } => DefaultExpressionKindV1::VariantConstruct {
                        variant: crate::DefaultEnumVariantRefV1::new(declaration, owner_type),
                        arguments,
                    },
                });
            }
            CallableTemplateOrigin::VariantConstructor(declaration) => {
                return Ok(DefaultExpressionKindV1::VariantConstruct {
                    variant: crate::DefaultEnumVariantRefV1::new(declaration, owner_type),
                    arguments: self.expressions(args)?,
                });
            }
        };
        if direct_super {
            let Some((receiver, arguments)) = args.split_first() else {
                return Err(DefaultBodyProjectionError::MissingMethodReceiver);
            };
            return Ok(DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver: Box::new(self.expression(receiver)?),
                callee: DefaultMethodCalleeV1::Callable(callable),
                arguments: self.expressions(arguments)?,
            });
        }
        Ok(DefaultExpressionKindV1::Call {
            callee: callable,
            receiver: receiver.try_map(|ty| self.type_key(ty))?,
            arguments: self.expressions(args)?,
        })
    }
}
