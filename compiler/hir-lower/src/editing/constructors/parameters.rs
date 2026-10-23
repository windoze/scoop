use super::*;

impl Lowerer {
    pub(in crate::editing) fn editing_callee_parameter_origin(
        &self,
        callee: C,
        index: usize,
    ) -> Option<hir::SemanticSourceLocation> {
        let origin = match callee {
            C::Callable(callable) => return self.editing_parameter_origin(callable, index),
            C::Constructor(Constructor::CurrentClass(constructor)) => {
                self.class_constructors[constructor]
                    .parameters
                    .get(index)?
                    .definition
            }
            C::Constructor(Constructor::CurrentStruct(constructor)) => {
                self.struct_constructors[constructor]
                    .parameters
                    .get(index)?
                    .definition
            }
            C::CurrentVariant(variant) => {
                if self.enums[variant.enumeration()].variants[variant.local_index() as usize].style
                    != hir::VariantStyle::Positional
                {
                    return self.editing_source_parameter_origin(
                        hir::SourceParameterOwner::VariantConstructor(variant),
                        index,
                    );
                }
                let field = hir::EnumVariantFieldRef::checked(&self.enums, variant, index as u32)?;
                return Some(hir::SemanticSourceLocation {
                    source: self
                        .source_identity(*self.enum_files.get(&variant.enumeration())?)
                        .clone(),
                    span: *self.enum_variant_field_spans.get(&field)?,
                });
            }
            C::Constructor(Constructor::Imported(constructor)) => {
                return self.editing_imported_parameter_origin(
                    CallableTemplateOrigin::Constructor(constructor),
                    index,
                );
            }
            C::ImportedVariant(variant) => {
                return self.editing_imported_parameter_origin(
                    CallableTemplateOrigin::VariantConstructor(variant),
                    index,
                );
            }
        };
        Some(hir::SemanticSourceLocation {
            source: self.source_identity(origin.file as usize).clone(),
            span: origin.span,
        })
    }

    pub(in crate::editing) fn editing_imported_parameter_origin(
        &self,
        callable: CallableTemplateOrigin,
        index: usize,
    ) -> Option<hir::SemanticSourceLocation> {
        let declaration = self
            .dependencies
            .as_ref()?
            .callable_declaration(callable)
            .ok()?;
        let parameter = declaration
            .source_interface()?
            .parameters()
            .parameters()
            .get(index)?;
        crate::semantic_queries::imported_location(parameter.definition_origin().origin()).ok()
    }
}
