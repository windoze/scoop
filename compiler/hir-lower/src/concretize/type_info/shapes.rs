use super::*;

mod enumerations;
mod nominals;
mod structural;

impl Concretizer<'_> {
    pub(in crate::concretize) fn type_description_data(
        &mut self,
        target: concrete::TypeId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let reference = self.type_reference_value(target, fields[0], origin);
        let annotations = self.type_nominal(target).map(|nominal| {
            self.source
                .static_nominal_annotations(nominal, self.dependencies)
        });
        let annotations = self.metadata_annotations(
            annotations
                .into_iter()
                .flat_map(|annotations| annotations.iter()),
            fields[1],
            origin,
        );
        let shape = self.metadata_type_shape(target, fields[2], origin);
        self.metadata_record(result, vec![reference, annotations, shape], origin)
    }

    fn metadata_type_shape(
        &mut self,
        target: concrete::TypeId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        use concrete::TypeKind as T;
        let intrinsic = match &self.types[target].kind {
            T::Struct(id) => match self.structs[*id].representation {
                concrete::StructRepresentation::Intrinsic { declaration, .. } => Some(declaration),
                _ => None,
            },
            T::Class(id) => match self.classes[*id].representation {
                concrete::ClassRepresentation::Intrinsic { declaration, .. } => Some(declaration),
                _ => None,
            },
            T::Integer(kind) => Some(export::IntrinsicTypeKind::Integer(*kind)),
            T::Boolean => Some(export::IntrinsicTypeKind::Boolean),
            T::String => Some(export::IntrinsicTypeKind::String),
            T::Any => Some(export::IntrinsicTypeKind::Any),
            _ => None,
        };
        let case = if intrinsic.is_some() {
            "Intrinsic"
        } else {
            match self.types[target].kind {
                T::Unit => "Unit",
                T::Struct(_) => "Struct",
                T::Class(_) => {
                    if self.metadata_shape(target).unwrap().kind()
                        == export::StaticNominalKind::Object
                    {
                        "Object"
                    } else {
                        "Class"
                    }
                }
                T::Enum(id) => {
                    if self.enums[id].representation.integer_kind().is_some() {
                        "IntegerEnum"
                    } else {
                        "Enum"
                    }
                }
                T::Interface(_) => "Interface",
                T::Tuple(_) => "Tuple",
                T::Function(_) => "Function",
                T::Ptr(_) | T::FunPtr(_) => "Pointer",
                T::Integer(_) | T::Boolean | T::String | T::Any => {
                    unreachable!("primitive types selected their intrinsic kind")
                }
            }
        };
        let fields = self.metadata_variant(result, case).1;
        let payload = if let Some(intrinsic) = intrinsic {
            Some(self.metadata_intrinsic(intrinsic, fields[0], origin))
        } else {
            match self.types[target].kind.clone() {
                T::Unit => None,
                T::Struct(_) | T::Class(_) | T::Interface(_) => Some(self.metadata_nominal_shape(
                    target,
                    self.metadata_shape(target).unwrap(),
                    fields[0],
                    origin,
                )),
                T::Enum(id) => Some(self.metadata_enum(target, id, fields[0], origin)),
                T::Tuple(elements) => Some(self.metadata_tuple(&elements, fields[0], origin)),
                T::Function(signature) => {
                    Some(self.metadata_function_type(signature, fields[0], origin))
                }
                T::Ptr(pointee) => {
                    let payload = self.metadata_variant(fields[0], "Ptr").1[0];
                    let pointee = self.metadata_type_use(pointee, &[], payload, origin);
                    Some(self.metadata_case(fields[0], "Ptr", vec![pointee], origin))
                }
                T::FunPtr(signature) => {
                    let payload = self.metadata_variant(fields[0], "FunPtr").1[0];
                    let function = self.metadata_function_type(signature, payload, origin);
                    Some(self.metadata_case(fields[0], "FunPtr", vec![function], origin))
                }
                _ => unreachable!("intrinsic metadata has been generated above"),
            }
        };
        self.metadata_case(result, case, payload.into_iter().collect(), origin)
    }
}
