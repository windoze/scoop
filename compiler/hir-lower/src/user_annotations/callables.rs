use super::*;
use hir::{SourceAnnotationCallable as Callable, SourceAnnotationTarget as Target};

impl Lowerer {
    pub(crate) fn annotate_function(
        &mut self,
        function: hir::FunctionId,
        source: &ast::FunctionDecl,
    ) {
        self.bind_source_annotations(
            Target::Callable(Callable::Function(function)),
            &source.annotations,
        );
        self.annotate_parameters(Callable::Function(function), &source.params);
    }

    pub(crate) fn annotate_parameters(&mut self, callable: Callable, parameters: &[ast::Param]) {
        for (index, parameter) in parameters.iter().enumerate() {
            self.bind_source_annotations(
                Target::Parameter {
                    callable,
                    index: index as u32,
                },
                &parameter.annotations,
            );
        }
    }

    pub(super) fn annotate_class_constructors(
        &mut self,
        owner: hir::ClassId,
        source: &ast::ClassDecl,
    ) {
        if let ast::ClassConstructorDecl::Declared(primary) = &source.constructor {
            let id = self.classes[owner].constructors.iter().copied().find(|id| {
                matches!(
                    self.class_constructors[*id].kind,
                    hir::ClassConstructorKind::Primary { .. }
                )
            });
            if let Some(id) = id {
                let callable = Callable::ClassConstructor(id);
                self.bind_source_annotations(Target::Callable(callable), &primary.annotations);
                for (index, parameter) in primary.parameters.iter().enumerate() {
                    if !parameter.property.is_property() {
                        self.bind_source_annotations(
                            Target::Parameter {
                                callable,
                                index: index as u32,
                            },
                            &parameter.annotations,
                        );
                    }
                }
            }
        }
        for source in source.secondary_constructors() {
            let id = self.classes[owner]
                .constructors
                .iter()
                .copied()
                .find(|id| self.class_constructors[*id].span == source.span);
            if let Some(id) = id {
                let callable = Callable::ClassConstructor(id);
                self.bind_source_annotations(Target::Callable(callable), &source.annotations);
                self.annotate_parameters(callable, &source.params);
            }
        }
    }

    pub(super) fn annotate_struct_constructors(
        &mut self,
        owner: hir::StructId,
        source: &ast::StructDecl,
    ) {
        for source in source.secondary_constructors() {
            let id = self.structs[owner]
                .constructors
                .iter()
                .copied()
                .find(|id| self.struct_constructors[*id].span == source.span);
            if let Some(id) = id {
                let callable = Callable::StructConstructor(id);
                self.bind_source_annotations(Target::Callable(callable), &source.annotations);
                self.annotate_parameters(callable, &source.params);
            }
        }
    }
}
