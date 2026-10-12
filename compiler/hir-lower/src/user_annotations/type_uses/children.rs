use super::*;
use ast::TypeRefKind as Shape;
use hir::TypeUseStep as Step;

impl Lowerer {
    pub(super) fn type_annotation_children(
        &self,
        source: &ast::TypeRef,
        ty: hir::TypeId,
    ) -> Vec<TypeUsePart> {
        let mut children = Vec::new();
        let mut push = |source: &ast::TypeRef, to: Vec<Step>, from: Vec<Step>| {
            let key = TypeUseKey::new(self.current_file, source.span);
            if self.type_annotations.uses.contains_key(&key) {
                children.push(TypeUsePart { key, from, to });
            }
        };
        match &source.kind {
            Shape::Nullable(inner) => push(inner, vec![Step::Argument(0)], Vec::new()),
            Shape::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    push(element, vec![Step::TupleElement(index as u32)], Vec::new());
                }
            }
            Shape::Function(signature) => {
                if let Some(receiver) = &signature.receiver {
                    push(receiver, vec![Step::FunctionReceiver], Vec::new());
                }
                for (index, parameter) in signature.parameters.iter().enumerate() {
                    push(
                        parameter,
                        vec![Step::FunctionParameter(index as u32)],
                        Vec::new(),
                    );
                }
                push(
                    &signature.return_type,
                    vec![Step::FunctionResult],
                    Vec::new(),
                );
            }
            Shape::Generic(_, arguments)
            | Shape::Qualified { arguments, .. }
            | Shape::AppliedMember { arguments, .. } => {
                let count = self
                    .nominal_application(ty)
                    .map_or(arguments.len(), |application| application.arguments.len());
                let offset = count.saturating_sub(arguments.len());
                for (index, argument) in arguments.iter().enumerate() {
                    let step = match self.types[ty] {
                        hir::Type::Ptr(_) => Step::PointerPointee,
                        hir::Type::FunPtr(_) => Step::FunctionPointee,
                        _ => Step::Argument((offset + index) as u32),
                    };
                    push(argument, vec![step], Vec::new());
                }
                if let Shape::AppliedMember { owner, .. } = &source.kind {
                    for index in 0..offset {
                        push(
                            owner,
                            vec![Step::Argument(index as u32)],
                            vec![Step::Argument(index as u32)],
                        );
                    }
                }
            }
            Shape::Named(_) | Shape::Unit => {}
        }
        children
    }
}
