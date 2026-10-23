use super::*;
use crate::call_resolution::candidates::{
    NominalConstructorSource, ValueParameterCalling, VarargOmission,
};

mod imported;
mod selection;

impl DeclarationAnalysis {
    pub fn constructor_signature(
        &mut self,
        callee: C,
    ) -> Result<Option<hir::EditingConstructorSignature>, ast::Diagnostic> {
        if !self.declarations_ready {
            return Ok(None);
        }
        let Some(origin) = self.lowerer.editing_callee_origin(callee) else {
            return Ok(None);
        };
        let target = match callee {
            C::Callable(_) => return Ok(None),
            C::Constructor(Constructor::CurrentClass(id)) => NominalConstructorSource::Class(id),
            C::Constructor(Constructor::CurrentStruct(id)) => NominalConstructorSource::Struct(id),
            C::CurrentVariant(id) => NominalConstructorSource::Variant(id),
            C::Constructor(Constructor::Imported(id)) => {
                return self.lowerer.editing_imported_constructor_signature(
                    callee,
                    CallableTemplateOrigin::Constructor(id),
                    origin,
                );
            }
            C::ImportedVariant(id) => {
                return self.lowerer.editing_imported_constructor_signature(
                    callee,
                    CallableTemplateOrigin::VariantConstructor(id),
                    origin,
                );
            }
        };
        if !self.declaration_scope_available(&origin) {
            return Ok(None);
        }
        let signature = self
            .lowerer
            .nominal_constructor_view(target, origin.span)
            .signature;
        let form = match callee {
            C::CurrentVariant(variant) => {
                let variant = &self.lowerer.enums[variant.enumeration()].variants
                    [variant.local_index() as usize];
                hir::EditingConstructorForm::Variant {
                    name: variant.name.clone(),
                    style: variant.style,
                }
            }
            _ => hir::EditingConstructorForm::Nominal,
        };
        let mut parameters = Vec::new();
        for (index, parameter) in signature.value_parameters.into_iter().enumerate() {
            let (vararg_element_type, default) = match parameter.calling {
                ValueParameterCalling::Required => (None, false),
                ValueParameterCalling::Default(_) => (None, true),
                ValueParameterCalling::Vararg {
                    element_type,
                    omission,
                    ..
                } => (
                    Some(element_type),
                    matches!(omission, VarargOmission::Default(_)),
                ),
            };
            let declared_default = if default {
                let owner = match callee {
                    C::Constructor(Constructor::CurrentClass(id)) => {
                        hir::SourceParameterOwner::ClassConstructor(id)
                    }
                    C::Constructor(Constructor::CurrentStruct(id)) => {
                        hir::SourceParameterOwner::StructConstructor(id)
                    }
                    C::CurrentVariant(id) => hir::SourceParameterOwner::VariantConstructor(id),
                    _ => unreachable!("source constructor parameters retain their original owner"),
                };
                Some(
                    self.lowerer
                        .editing_default_source(hir::SourceDefaultKey::new(owner, index))
                        .expect("a source default retains its original expression")
                        .origin,
                )
            } else {
                None
            };
            parameters.push(hir::EditingParameter {
                name: parameter.name,
                value_type: parameter.ty,
                vararg_element_type,
                declared_default,
                origin: self
                    .lowerer
                    .editing_callee_parameter_origin(callee, index)
                    .expect("a constructor parameter retains its source"),
            });
        }
        Ok(Some(hir::EditingConstructorSignature {
            callee,
            form,
            binders: hir::SemanticDeclarationBinders {
                owner: signature.owner_parameters,
                callable: signature.callable_parameters,
            },
            parameters,
            result: signature.return_type,
            origin,
        }))
    }
}
