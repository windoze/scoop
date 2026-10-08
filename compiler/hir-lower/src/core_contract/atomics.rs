use super::*;

const MEMORY_ORDERS: [(&str, hir::AtomicMemoryOrder); 5] = [
    ("Relaxed", hir::AtomicMemoryOrder::Relaxed),
    ("Acquire", hir::AtomicMemoryOrder::Acquire),
    ("Release", hir::AtomicMemoryOrder::Release),
    ("AcqRel", hir::AtomicMemoryOrder::AcqRel),
    ("SeqCst", hir::AtomicMemoryOrder::SeqCst),
];

impl Lowerer {
    pub(crate) fn validate_atomic_intrinsics(&mut self, files: &[ast::SourceFile]) {
        let functions = hir::AtomicIntrinsic::all()
            .filter_map(|kind| {
                self.intrinsic_functions
                    .get(&hir::IntrinsicFunctionKind::Atomic(kind))
                    .map(|(function, _)| (kind, *function))
            })
            .collect::<Vec<_>>();
        if functions.is_empty() {
            return;
        }
        let Some(order) = self.require_core_enum("MemoryOrder", files) else {
            return;
        };
        self.validate_unit_enum(order, &MEMORY_ORDERS.map(|(name, _)| name));
        let order_type = self.interned_enum_type(order);
        for (kind, function) in functions {
            if !self.valid_atomic_signature(function, kind, order_type) {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core atomic intrinsic `{}`", kind.name()),
                );
            }
        }
    }

    fn valid_atomic_signature(
        &mut self,
        function: FunctionId,
        kind: hir::AtomicIntrinsic,
        order: TypeId,
    ) -> bool {
        if !self
            .function_has_intrinsic_owner(function, hir::IntrinsicTypeKind::Atomic(kind.family()))
        {
            return false;
        }
        let Some(Owner::Class(owner)) = self.function_owner.get(&function).copied() else {
            return false;
        };
        let receiver = self.class_applications[self.classes[owner].self_application].canonical_type;
        let Some((_, value)) = self.atomic_value_type(receiver) else {
            return false;
        };
        let signature = &self.signatures[&function];
        let type_parameters = usize::from(kind.family() == hir::AtomicValueKind::Reference);
        let values = kind.method().value_parameter_count();
        let orders = kind.method().order_parameter_count();
        let result = match kind.method() {
            hir::AtomicMethod::Store => self.unit,
            hir::AtomicMethod::CompareAndSet => self.boolean,
            _ => value,
        };
        if signature.is_suspend
            || !signature.context_parameters.is_empty()
            || signature.owner_type_param_count != type_parameters
            || signature.type_params.len() != type_parameters
            || signature.modifiers != hir::CallableModifiers::default()
            || signature.return_ty != result
            || signature.params.len() != values + orders
            || self.functions[function].name.rsplit('.').next() != Some(kind.method().source_name())
        {
            return false;
        }
        signature
            .params
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                let name = if index < values {
                    if values == 2 && index == 0 {
                        "expected"
                    } else {
                        "value"
                    }
                } else if orders == 1 {
                    "order"
                } else if index == values {
                    "successOrder"
                } else {
                    "failureOrder"
                };
                parameter.name.text == name
                    && if index < values {
                        parameter.ty == value
                            && matches!(parameter.calling, crate::FnParamCalling::Required)
                    } else {
                        parameter.ty == order
                            && matches!(parameter.calling, crate::FnParamCalling::Default { .. })
                    }
            })
    }

    /// The argument has already been checked against the selected method's
    /// actual MemoryOrder parameter. Resolve variants by that nominal identity.
    pub(crate) fn atomic_memory_order(
        &self,
        variant: hir::EnumVariantApplication,
    ) -> Option<hir::AtomicMemoryOrder> {
        MEMORY_ORDERS.into_iter().find_map(|(name, order)| {
            (self.named_enum_variant(variant.owner, name) == Some(variant)).then_some(order)
        })
    }

    pub(crate) fn atomic_order_type_matches(&self, ty: TypeId) -> bool {
        let Type::Enum(application) = self.types[ty] else {
            return false;
        };
        let application = &self.enum_applications[application];
        let definition = self.enum_definition(application.template);
        application.arguments.is_empty()
            && definition.variants.len() == MEMORY_ORDERS.len()
            && definition
                .variants
                .iter()
                .zip(MEMORY_ORDERS)
                .all(|(variant, (name, _))| {
                    variant.name == name
                        && variant.fields.is_empty()
                        && variant.style == hir::VariantStyle::Unit
                })
    }
}
