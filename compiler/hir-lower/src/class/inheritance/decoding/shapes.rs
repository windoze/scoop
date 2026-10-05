use super::*;
mod primary;

impl Lowerer {
    pub(super) fn decode_record_shape(
        &mut self,
        result: TypeId,
        span: Span,
    ) -> Option<DecodeRecord> {
        let application = self
            .nominal_application(result)
            .expect("a record has a nominal type");
        if self.nominal_intrinsic_kind(application.template).is_some() {
            self.error(
                span,
                format!(
                    "intrinsic type {} requires an explicit decode implementation",
                    self.type_name(result)
                ),
            );
            return None;
        }
        if matches!(self.types[result], Type::Class(_)) {
            return self.decode_class_shape(result, span);
        }
        let mut record = self.decoding_primary(result, span)?;
        if let Some(structure) = self.source_struct_id(application.template) {
            for (index, parameter) in record.parameters.iter_mut().enumerate() {
                let field = hir::StructFieldRef::checked(&self.structs, structure, index as u32)
                    .expect("a primary parameter maps to its field");
                parameter.wire = self.serialization_wire_name(
                    hir::SourceAnnotationTarget::Field(field),
                    &parameter.name,
                );
            }
        } else {
            let declaration = &self.loaded_struct_definitions[&application.template].declaration;
            for (index, parameter) in record.parameters.iter_mut().enumerate() {
                let field = declaration.interface.source_shape().declared_fields()[index].field();
                parameter.wire = self.dependency_decoding_wire_name(
                    hir::AnnotationTargetV1::Field(field),
                    &parameter.name,
                );
            }
        }
        Some(record)
    }

    pub(super) fn current_decoding_record(
        &mut self,
        result: TypeId,
        source: NominalConstructorSource,
        keyed: bool,
        span: Span,
    ) -> DecodeRecord {
        let application = self
            .nominal_application(result)
            .expect("a source constructor has a nominal result");
        let signature = self.nominal_constructor_view(source, span).signature;
        let bindings = signature
            .owner_parameters
            .iter()
            .map(|parameter| parameter.id)
            .zip(application.arguments.iter().copied())
            .collect();
        let parameters = signature
            .value_parameters
            .into_iter()
            .map(|parameter| {
                let default = match parameter.calling {
                    ValueParameterCalling::Default(source)
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::Default(source),
                        ..
                    } => Some(DecodeDefault::Current(source)),
                    ValueParameterCalling::Required
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::EmptyArray,
                        ..
                    } => None,
                };
                DecodeParameter {
                    wire: Some(parameter.name.clone()),
                    name: parameter.name,
                    ty: self.instantiate_ty(parameter.ty, &application.arguments),
                    default,
                }
            })
            .collect();
        DecodeRecord {
            constructor: DecodeConstructor::Current { source, bindings },
            parameters,
            keyed,
        }
    }

    pub(super) fn dependency_decoding_record(
        &mut self,
        result: TypeId,
        origin: scoop_identity::CallableTemplateOrigin,
        keyed: bool,
        span: Span,
    ) -> Option<DecodeRecord> {
        let application = self
            .nominal_application(result)
            .expect("a dependency constructor has a nominal result");
        let declaration = self
            .dependencies
            .as_ref()
            .expect("a dependency constructor has a catalog")
            .callable_declaration(origin)
            .expect("the selected source constructor is declared")
            .clone();
        let bindings: ImportedTypeBindings = application
            .arguments
            .iter()
            .enumerate()
            .map(|(index, &ty)| {
                (
                    scoop_identity::SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    ty,
                )
            })
            .collect();
        let types = declaration
            .interface()
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                self.imported_signature_type_with_bindings(parameter.value_type(), &bindings)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                self.error(
                    span,
                    format!("cannot resolve decoding constructor parameters: {error:?}"),
                )
            })
            .ok()?;
        let parameters = self
            .imported_parameter_views(&declaration, types)
            .into_iter()
            .map(|parameter| {
                let default = match parameter.calling {
                    ValueParameterCalling::Default(source)
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::Default(source),
                        ..
                    } => Some(DecodeDefault::Dependency(source)),
                    ValueParameterCalling::Required
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::EmptyArray,
                        ..
                    } => None,
                };
                DecodeParameter {
                    wire: Some(parameter.name.clone()),
                    name: parameter.name,
                    ty: parameter.ty,
                    default,
                }
            })
            .collect();
        Some(DecodeRecord {
            constructor: DecodeConstructor::Dependency {
                declaration: Box::new(declaration),
                bindings,
            },
            parameters,
            keyed,
        })
    }
}
