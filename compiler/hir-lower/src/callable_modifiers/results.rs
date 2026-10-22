use super::*;

impl Lowerer {
    pub(crate) fn validate_inferred_operator_result(
        &mut self,
        id: hir::FunctionId,
        decl: &ast::FunctionSignature,
    ) {
        let function = &self.functions[id];
        let result = function.return_ty;
        let modifiers = function.modifiers;
        let receiver = function
            .method
            .map(|method| method.owner)
            .or_else(|| self.extension_receivers.get(&id).copied());
        if let Some(kind) = modifiers.operator {
            self.validate_operator_result(decl, kind, receiver, result);
        }
        if modifiers.property_delegate_operator == Some(hir::PropertyDelegateOperatorKind::SetValue)
            && !self.types_equal(result, self.unit)
        {
            self.error(
                decl.name.span,
                format!(
                    "property delegate operator `setValue` must return Unit, found {}",
                    self.type_name(result)
                ),
            );
        }
    }

    pub(super) fn validate_operator_result(
        &mut self,
        decl: &ast::FunctionSignature,
        kind: hir::OperatorKind,
        receiver: Option<TypeId>,
        result: TypeId,
    ) {
        use hir::OperatorKind as K;
        match kind {
            K::Set
            | K::PlusAssign
            | K::MinusAssign
            | K::TimesAssign
            | K::DivAssign
            | K::RemAssign => self.require_operator_return(decl, result, self.unit, "Unit"),
            K::Contains | K::Equals => {
                self.require_operator_return(decl, result, self.boolean, "Boolean")
            }
            K::CompareTo => self.require_operator_return(
                decl,
                result,
                self.integer_type(hir::IntegerKind::SIGNED_64),
                "Long",
            ),
            K::Inc | K::Dec => {
                if let Some(receiver) = receiver
                    && !self.is_subtype(result, receiver)
                {
                    self.error(
                        decl.name.span,
                        format!(
                            "operator `{}` must return a subtype of receiver type {}, found {}",
                            decl.name.text,
                            self.type_name(receiver),
                            self.type_name(result)
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}
