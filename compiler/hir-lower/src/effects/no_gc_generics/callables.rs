use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::effects) enum GenericCallable {
    Function(hir::FunctionId),
    ClassConstructor(hir::ClassConstructorId),
    StructConstructor(hir::StructConstructorId),
}

impl GenericCallable {
    pub(in crate::effects) fn kind(self) -> &'static str {
        match self {
            Self::Function(_) => "function",
            Self::ClassConstructor(_) | Self::StructConstructor(_) => "constructor",
        }
    }
}

impl Lowerer {
    pub(in crate::effects) fn effect_callable_ids(&self) -> Vec<GenericCallable> {
        self.functions
            .iter()
            .map(|(id, _)| GenericCallable::Function(id))
            .chain(
                self.class_constructors
                    .iter()
                    .map(|(id, _)| GenericCallable::ClassConstructor(id)),
            )
            .chain(
                self.struct_constructors
                    .iter()
                    .map(|(id, _)| GenericCallable::StructConstructor(id)),
            )
            .collect()
    }

    pub(in crate::effects) fn effect_callable_parameters(
        &self,
        callable: GenericCallable,
    ) -> Vec<&hir::TypeParamDecl> {
        match callable {
            GenericCallable::Function(id) => self.functions[id].type_params(),
            GenericCallable::ClassConstructor(id) => self.classes
                [self.class_constructors[id].owner]
                .type_params
                .iter()
                .collect(),
            GenericCallable::StructConstructor(id) => self.structs
                [self.struct_constructors[id].owner]
                .type_params
                .iter()
                .collect(),
        }
    }

    pub(in crate::effects) fn effect_callable_parameter(
        &self,
        callable: GenericCallable,
        parameter: hir::TypeParamId,
    ) -> &hir::TypeParamDecl {
        self.effect_callable_parameters(callable)
            .into_iter()
            .find(|p| p.id == parameter)
            .expect("a callable requirement names one of its complete declaration parameters")
    }

    pub(in crate::effects) fn effect_callable_name(&self, callable: GenericCallable) -> &str {
        match callable {
            GenericCallable::Function(id) => &self.functions[id].name,
            GenericCallable::ClassConstructor(id) => {
                &self.classes[self.class_constructors[id].owner].name
            }
            GenericCallable::StructConstructor(id) => {
                &self.structs[self.struct_constructors[id].owner].name
            }
        }
    }

    pub(in crate::effects) fn effect_callable_span(&self, callable: GenericCallable) -> Span {
        match callable {
            GenericCallable::Function(id) => self.functions[id].span,
            GenericCallable::ClassConstructor(id) => self.class_constructors[id].span,
            GenericCallable::StructConstructor(id) => self.struct_constructors[id].span,
        }
    }

    pub(in crate::effects) fn effect_callable_file(&self, callable: GenericCallable) -> usize {
        match callable {
            GenericCallable::Function(id) => self
                .function_files
                .get(&id)
                .copied()
                .unwrap_or_else(|| self.primary_output_file()),
            GenericCallable::ClassConstructor(id) => {
                self.class_constructors[id].origin.file as usize
            }
            GenericCallable::StructConstructor(id) => {
                self.struct_constructors[id].origin.file as usize
            }
        }
    }
}
