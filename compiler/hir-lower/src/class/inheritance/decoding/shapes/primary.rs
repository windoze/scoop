use super::*;

impl Lowerer {
    pub(in crate::class::inheritance::decoding) fn decoding_primary(
        &mut self,
        result: TypeId,
        span: Span,
    ) -> Option<DecodeRecord> {
        let application = self
            .nominal_application(result)
            .expect("a primary constructor has a nominal owner");
        let local = match self.types[result] {
            Type::Struct(_) => self.source_struct_id(application.template).map(|owner| {
                self.struct_primary_constructor(owner)
                    .map(NominalConstructorSource::Struct)
            }),
            Type::Class(_) => self.source_class_id(application.template).map(|owner| {
                self.classes[owner]
                    .constructors
                    .iter()
                    .copied()
                    .find(|id| {
                        let value = &self.class_constructors[*id];
                        value.identity_kind == hir::ClassConstructorIdentityKind::Source
                            && matches!(value.kind, hir::ClassConstructorKind::Primary { .. })
                    })
                    .map(NominalConstructorSource::Class)
            }),
            _ => unreachable!("only structs and classes have primary constructors"),
        };
        if let Some(local) = local {
            let Some(source) = local else {
                self.error(
                    span,
                    format!(
                        "automatic decode requires a primary constructor for {}",
                        self.type_name(result)
                    ),
                );
                return None;
            };
            return Some(self.current_decoding_record(result, source, true, span));
        }
        let declaration = self
            .dependencies
            .as_ref()?
            .nominal_declaration(application.template)?;
        let details = declaration.interface.declaration_details();
        let constructor = match self.types[result] {
            Type::Struct(_) => details.primary_value_constructor(),
            Type::Class(_) => details
                .class_primary_constructor()
                .map(|primary| primary.constructor()),
            _ => unreachable!("only structs and classes have primary constructors"),
        };
        let Some(constructor) = constructor else {
            self.error(
                span,
                format!(
                    "automatic decode requires a primary constructor for {}",
                    self.type_name(result)
                ),
            );
            return None;
        };
        self.dependency_decoding_record(
            result,
            scoop_identity::CallableTemplateOrigin::Constructor(constructor),
            true,
            span,
        )
    }
}
