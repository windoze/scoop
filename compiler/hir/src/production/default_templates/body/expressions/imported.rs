use scoop_identity::{CallableTemplateOrigin, OptionalSignatureType};

use super::BodyProjection;
use crate::production::default_templates::DefaultBodyProjectionError;
use crate::{
    DefaultCallableDeclarationV1, DefaultCallableRefV1, DefaultConstructorRefV1,
    DefaultExpressionKindV1, Expr, ImportedDependencyCallableUseId, SourceCallReceiver, TypeId,
};

impl BodyProjection<'_, '_> {
    pub(super) fn imported_call(
        &mut self,
        callee: ImportedDependencyCallableUseId,
        args: &[Expr],
        receiver: SourceCallReceiver<TypeId>,
        result_type: TypeId,
    ) -> Result<DefaultExpressionKindV1, DefaultBodyProjectionError> {
        let selected = self.entities.imported_dependency_source(callee)?;
        let declaration = selected.interface().declaration();
        let owner_type = selected.interface().result().clone();
        let callable = match declaration {
            CallableTemplateOrigin::Function(id) => DefaultCallableDeclarationV1::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                DefaultCallableDeclarationV1::GenericFunction(id)
            }
            CallableTemplateOrigin::Accessor(id) => {
                DefaultCallableDeclarationV1::PropertyAccessor(id)
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
        let callee =
            DefaultCallableRefV1::try_new(callable, OptionalSignatureType::Absent, Vec::new())
                .map_err(
                    crate::production::default_templates::DefaultEntityProjectionError::Callable,
                )?;
        Ok(DefaultExpressionKindV1::Call {
            callee,
            receiver: receiver.try_map(|ty| self.type_key(ty))?,
            arguments: self.expressions(args)?,
        })
    }
}
