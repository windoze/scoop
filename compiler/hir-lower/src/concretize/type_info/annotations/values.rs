use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize::type_info) fn metadata_annotation_value(
        &mut self,
        annotation: &export::CanonicalAnnotationValueV1,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        use export::CanonicalAnnotationValueV1 as V;
        let (name, values) = match annotation {
            V::Scalar(scalar) => return self.metadata_scalar(scalar, result, origin),
            V::IntegralEnum {
                value_type,
                variant,
                value,
            } => {
                let fields = self.metadata_variant(result, "IntegerEnum").1;
                let ty = self.metadata_signature_type(value_type, &[]);
                let ty = self.type_reference_value(ty, fields[0], origin);
                let variant =
                    self.metadata_identity(fields[1], format!("variant:{variant}"), origin);
                let value = self.metadata_integer((*value).into(), fields[2], origin);
                ("IntegerEnum", vec![ty, variant, value])
            }
            V::Annotation {
                annotation,
                arguments,
            } => {
                let record = self.metadata_variant(result, "Annotation").1[0];
                let value = self.metadata_annotation(
                    export::StaticAnnotation {
                        annotation: *annotation,
                        arguments,
                    },
                    record,
                    origin,
                );
                ("Annotation", vec![value])
            }
            V::Array {
                element_type,
                elements,
            } => {
                let fields = self.metadata_variant(result, "Array").1;
                let atom = match element_type {
                    export::AnnotationElementTypeV1::Scalar(kind) => {
                        AnnotationType::Value(self.metadata_scalar_type(*kind))
                    }
                    export::AnnotationElementTypeV1::IntegralEnum(ty) => {
                        AnnotationType::Value(self.metadata_signature_type(ty, &[]))
                    }
                    export::AnnotationElementTypeV1::Annotation(id) => {
                        AnnotationType::Annotation(*id)
                    }
                };
                let atom = self.metadata_annotation_type(atom, fields[0], origin);
                let item = self.metadata_element(fields[1]);
                let elements = elements
                    .iter()
                    .map(|element| self.metadata_annotation_value(&element.value, item, origin))
                    .collect();
                let elements = self.metadata_array(fields[1], elements, origin);
                ("Array", vec![atom, elements])
            }
        };
        self.metadata_case(result, name, values, origin)
    }

    fn metadata_scalar(
        &mut self,
        scalar: &export::CanonicalConstValueV1,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        use export::CanonicalConstValueV1 as V;
        let name = match scalar {
            V::Boolean(_) => "Boolean",
            V::String(_) => "String",
            V::Char(_) => "Char",
            V::Integer(_) => "Integer",
            V::Float(value) => match value.kind() {
                export::FloatKind::F32 => "FloatBits",
                export::FloatKind::F64 => "DoubleBits",
            },
        };
        let payload = self.metadata_variant(result, name).1[0];
        let expression = match scalar {
            V::Integer(integer) => self.metadata_integer((*integer).into(), payload, origin),
            V::Boolean(boolean) => self.metadata_bool((*boolean).into(), origin),
            V::String(string) => self.metadata_string(string.clone(), origin),
            V::Char(character) => value(
                concrete::ExprKind::CharLiteral(character.value()),
                payload,
                origin,
            ),
            V::Float(float) => {
                let integer = match float.kind() {
                    export::FloatKind::F32 => {
                        export::HirIntegerConstant::Unsigned32(float.raw_bits() as u32)
                    }
                    export::FloatKind::F64 => {
                        export::HirIntegerConstant::Unsigned64(float.raw_bits())
                    }
                };
                value(concrete::ExprKind::IntegerLiteral(integer), payload, origin)
            }
        };
        self.metadata_case(result, name, vec![expression], origin)
    }

    pub(in crate::concretize::type_info) fn metadata_integer(
        &self,
        integer: export::HirIntegerConstant,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let name = integer.kind().canonical_name();
        let ty = self.metadata_variant(result, name).1[0];
        let integer = value(concrete::ExprKind::IntegerLiteral(integer), ty, origin);
        self.metadata_case(result, name, vec![integer], origin)
    }

    fn metadata_scalar_type(
        &mut self,
        kind: export::CanonicalConstValueKindV1,
    ) -> concrete::TypeId {
        use export::CanonicalConstValueKindV1 as K;
        let source = match kind {
            K::Integer(kind) => return self.lower_integer_type(kind, &[]),
            K::Boolean => self.source.boolean,
            K::String => self.source.string,
            K::Char | K::Float(_) => {
                let nominal = match self.core {
                    export::CoreProtocols::Defined(core) => {
                        let id = match kind {
                            K::Char => core.fundamental_types.character,
                            K::Float(export::FloatKind::F32) => core.fundamental_types.float,
                            K::Float(export::FloatKind::F64) => core.fundamental_types.double,
                            _ => unreachable!(),
                        };
                        self.source.nominal_identities[id].declaration_id()
                    }
                    export::CoreProtocols::Imported(core) => {
                        export::SourceNominalId::Concrete(match kind {
                            K::Char => core.fundamental_types().character().persistent(),
                            K::Float(kind) => core.fundamental_types().floating(kind).persistent(),
                            _ => unreachable!(),
                        })
                    }
                };
                self.metadata_nominal_source(nominal)
            }
        };
        self.lower_type(source, &[])
    }
}
