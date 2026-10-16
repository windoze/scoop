use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_type_use(
        &mut self,
        target: concrete::TypeId,
        nodes: &[export::TypeAnnotationNode],
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        self.metadata_type_use_at(target, nodes, &[], result, origin)
    }

    fn metadata_type_use_at(
        &mut self,
        target: concrete::TypeId,
        nodes: &[export::TypeAnnotationNode],
        path: &[export::TypeUseStep],
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let reference = self.type_reference_value(target, fields[0], origin);
        let annotations = nodes
            .iter()
            .filter(|node| node.path == path)
            .flat_map(|node| &node.annotations)
            .map(|annotation| export::StaticAnnotation {
                annotation: annotation.annotation,
                arguments: &annotation.arguments,
            });
        let annotations = self.metadata_annotations(annotations, fields[1], origin);
        let child = self.metadata_element(fields[2]);
        let child_fields = self.metadata_fields(child);
        let children = self
            .metadata_type_children(target)
            .into_iter()
            .map(|(step, target)| {
                let (role, index) = match step {
                    export::TypeUseStep::Argument(index) => ("Argument", Some(index)),
                    export::TypeUseStep::TupleElement(index) => ("TupleElement", Some(index)),
                    export::TypeUseStep::FunctionReceiver => ("Receiver", None),
                    export::TypeUseStep::FunctionParameter(index) => ("Parameter", Some(index)),
                    export::TypeUseStep::FunctionResult => ("Result", None),
                    export::TypeUseStep::PointerPointee | export::TypeUseStep::FunctionPointee => {
                        ("Pointee", None)
                    }
                };
                let index = index
                    .map(|index| self.metadata_index(index as usize, origin))
                    .into_iter()
                    .collect();
                let role = self.metadata_case(child_fields[0], role, index, origin);
                let mut nested = path.to_vec();
                nested.push(step);
                let ty = self.metadata_type_use_at(target, nodes, &nested, child_fields[1], origin);
                self.metadata_record(child, vec![role, ty], origin)
            })
            .collect();
        let children = self.metadata_array(fields[2], children, origin);
        self.metadata_record(result, vec![reference, annotations, children], origin)
    }

    fn metadata_type_children(
        &self,
        target: concrete::TypeId,
    ) -> Vec<(export::TypeUseStep, concrete::TypeId)> {
        use export::TypeUseStep as Step;
        match &self.types[target].kind {
            concrete::TypeKind::Tuple(elements) => elements
                .iter()
                .enumerate()
                .map(|(i, ty)| (Step::TupleElement(i as u32), *ty))
                .collect(),
            concrete::TypeKind::Function(id) => {
                let signature = &self.function_types[*id];
                signature
                    .receiver
                    .map(|ty| (Step::FunctionReceiver, ty))
                    .into_iter()
                    .chain(
                        signature
                            .parameter_types
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| (Step::FunctionParameter(i as u32), *ty)),
                    )
                    .chain([(Step::FunctionResult, signature.return_type)])
                    .collect()
            }
            concrete::TypeKind::Ptr(pointee) => vec![(Step::PointerPointee, *pointee)],
            concrete::TypeKind::FunPtr(signature) => vec![(
                Step::FunctionPointee,
                self.function_types[*signature].canonical_type,
            )],
            _ => self
                .type_arguments(target)
                .into_iter()
                .enumerate()
                .map(|(i, ty)| (Step::Argument(i as u32), ty))
                .collect(),
        }
    }
}
