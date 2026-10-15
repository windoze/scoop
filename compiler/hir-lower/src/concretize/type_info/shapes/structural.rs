use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_intrinsic(
        &mut self,
        kind: export::IntrinsicTypeKind,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        use export::IntrinsicTypeKind as K;
        let enumeration = self.metadata_fields(result)[0];
        let (name, values) = match kind {
            K::Integer(kind) => {
                let signed = self.metadata_bool(
                    kind.signedness() == export::IntegerSignedness::Signed,
                    origin,
                );
                let width = self.metadata_index(kind.width().bits() as usize, origin);
                ("Integer", vec![signed, width])
            }
            K::Atomic(kind) => {
                let ty = self.metadata_variant(enumeration, "Atomic").1[0];
                let name = match kind {
                    export::AtomicValueKind::Int => "Int",
                    export::AtomicValueKind::Long => "Long",
                    export::AtomicValueKind::Boolean => "Boolean",
                    export::AtomicValueKind::Reference => "Reference",
                };
                (
                    "Atomic",
                    vec![self.metadata_case(ty, name, Vec::new(), origin)],
                )
            }
            K::Boolean => ("Boolean", Vec::new()),
            K::Char => ("Char", Vec::new()),
            K::Float(export::FloatKind::F32) => ("Float", Vec::new()),
            K::Float(export::FloatKind::F64) => ("Double", Vec::new()),
            K::String => ("String", Vec::new()),
            K::Array => ("Array", Vec::new()),
            K::MutableArray => ("MutableArray", Vec::new()),
            K::Any => ("Any", Vec::new()),
            K::Nothing => ("Nothing", Vec::new()),
            K::MaybeUninit => ("MaybeUninit", Vec::new()),
            K::Unit | K::Ptr | K::FunPtr => {
                unreachable!("Unit and pointer representations have their own shapes")
            }
        };
        let kind = self.metadata_case(enumeration, name, values, origin);
        self.metadata_record(result, vec![kind], origin)
    }

    pub(super) fn metadata_tuple(
        &mut self,
        elements: &[concrete::TypeId],
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let array = self.metadata_fields(result)[0];
        let record = self.metadata_element(array);
        let fields = self.metadata_fields(record);
        let elements = elements
            .iter()
            .enumerate()
            .map(|(index, &ty)| {
                let name = self.metadata_string(format!("_{}", index + 1), origin);
                let index = self.metadata_index(index, origin);
                let ty = self.metadata_type_use(ty, &[], fields[2], origin);
                self.metadata_record(record, vec![index, name, ty], origin)
            })
            .collect();
        let elements = self.metadata_array(array, elements, origin);
        self.metadata_record(result, vec![elements], origin)
    }

    pub(super) fn metadata_function_type(
        &mut self,
        signature: concrete::FunctionTypeId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let signature = self.function_types[signature].clone();
        let suspend = self.metadata_bool(signature.is_suspend, origin);
        let receiver = signature.receiver.map(|ty| {
            self.metadata_type_use(ty, &[], self.metadata_optional_element(fields[1]), origin)
        });
        let receiver = self.metadata_option(fields[1], receiver, origin);
        let element = self.metadata_element(fields[2]);
        let parameters = signature
            .parameter_types
            .into_iter()
            .map(|ty| self.metadata_type_use(ty, &[], element, origin))
            .collect();
        let parameters = self.metadata_array(fields[2], parameters, origin);
        let return_type = self.metadata_type_use(signature.return_type, &[], fields[3], origin);
        self.metadata_record(
            result,
            vec![suspend, receiver, parameters, return_type],
            origin,
        )
    }
}
