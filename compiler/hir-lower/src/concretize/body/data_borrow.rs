use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_data_borrow_call(
        &mut self,
        callee: concrete::CallableTarget,
        args: &[export::Expr],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Option<concrete::DataBorrow> {
        let concrete::CallableTarget::Local(concrete::Callable::Function(function)) = callee else {
            return None;
        };
        let index = function.into_raw().into_u32() as usize;
        let (kind, [object_type, block_type]) =
            self.data_borrow_signature(self.function_sources[index])?;
        let arguments = self.function_key_arguments(&self.function_keys[index]);
        let object_type = self.lower_type(object_type, &arguments);
        let block_type = self.lower_type(block_type, &arguments);
        let concrete::TypeKind::Function(function_type) = self.types[block_type].kind else {
            unreachable!("the data borrow declaration has an ordinary function parameter")
        };
        let source = match kind {
            concrete::DataBorrowIntrinsic::String => concrete::BorrowDataSource::String,
            concrete::DataBorrowIntrinsic::Array | concrete::DataBorrowIntrinsic::MutableArray => {
                let concrete::TypeKind::Class(class) = self.types[object_type].kind else {
                    unreachable!("the resolved array borrow parameter has an exact class")
                };
                concrete::BorrowDataSource::Array(class)
            }
        };
        let [object, block] = args else {
            unreachable!("resolved arguments cover the two data borrow parameters")
        };
        Some(concrete::DataBorrow {
            source,
            function_type,
            object: Box::new(self.lower_expr(object, substitution, locals)),
            block: Box::new(self.lower_expr(block, substitution, locals)),
        })
    }

    fn data_borrow_signature(
        &self,
        source: FunctionSource,
    ) -> Option<(export::DataBorrowIntrinsic, [export::TypeId; 2])> {
        match source {
            FunctionSource::Local(function) => {
                let kind = borrow_kind(&self.source.functions[function].kind)?;
                // Bodyless source intrinsics retain their logical parameters in
                // the source interface; they do not have body-local parameters.
                let interface = self
                    .source
                    .source_parameter_interfaces
                    .iter()
                    .find(|interface| {
                        interface.owner == export::ExportParameterOwner::Function(function)
                    })
                    .expect("a source intrinsic retains its resolved parameter interface");
                let [object, block] = interface.parameters.as_slice() else {
                    unreachable!("a data borrow source interface has two parameters")
                };
                let value_type = |parameter: &export::ExportValueParameter| match parameter.calling
                {
                    export::ExportParameterCalling::Required { value_type }
                    | export::ExportParameterCalling::Default { value_type, .. } => value_type,
                    export::ExportParameterCalling::Vararg { parameter_type, .. } => {
                        self.source.export_vararg_parameter_types[parameter_type].array_type
                    }
                };
                Some((kind, [value_type(object), value_type(block)]))
            }
            FunctionSource::Imported(template) => {
                let template = &self.source.imported_generic_templates[template];
                let kind = borrow_kind(&template.implementation)?;
                let [object, block] = template.params.as_slice() else {
                    unreachable!("an imported data borrow signature has two parameters")
                };
                Some((kind, [object.ty, block.ty]))
            }
            FunctionSource::Companion(..) => None,
        }
    }
}

fn borrow_kind(implementation: &export::FunctionKind) -> Option<export::DataBorrowIntrinsic> {
    match implementation {
        export::FunctionKind::Intrinsic(export::IntrinsicFunction {
            kind: export::IntrinsicFunctionKind::DataBorrow(kind),
            ..
        }) => Some(*kind),
        _ => None,
    }
}
