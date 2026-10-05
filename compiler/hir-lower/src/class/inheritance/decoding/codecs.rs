use super::*;
mod dependencies;
mod values;

pub(super) struct SelectedDecoder {
    ty: TypeId,
    value: DecoderValue,
}

enum DecoderValue {
    Dependency(DecoderDependency),
    This,
    Singleton(TypeId),
    Container {
        companion: TypeId,
        element: Box<SelectedDecoder>,
    },
    Tuple(Vec<SelectedDecoder>),
}

enum DecoderDependency {
    Field(hir::FieldRef, TypeId),
    Property(hir::PropertyId, TypeId),
}

impl Lowerer {
    pub(super) fn select_field_decoder(
        &mut self,
        context: DecodeContext,
        ty: TypeId,
        span: Span,
    ) -> Option<SelectedDecoder> {
        let interface = self
            .apply_nominal_type(context.decodable, vec![ty])
            .expect("Decodable has one unconstrained parameter");
        let mut dependencies = self.decoding_dependencies(context.owner, interface);
        if dependencies.len() > 1 {
            self.error(
                span,
                format!(
                    "automatic decode has ambiguous primary val dependencies for {}: {}",
                    self.type_name(ty),
                    dependencies
                        .iter()
                        .map(|(name, _)| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            return None;
        }
        let value = if let Some((_, dependency)) = dependencies.pop() {
            DecoderValue::Dependency(dependency)
        } else if self.types_equal(ty, context.result) {
            DecoderValue::This
        } else {
            return self.select_default_decoder(context, ty, interface, span);
        };
        Some(SelectedDecoder { ty, value })
    }

    fn select_default_decoder(
        &mut self,
        context: DecodeContext,
        ty: TypeId,
        interface: TypeId,
        span: Span,
    ) -> Option<SelectedDecoder> {
        let companion = self.decoding_companion(ty, span)?;
        if let Some(companion) = companion
            && self.is_subtype(companion, interface)
        {
            return Some(SelectedDecoder {
                ty,
                value: DecoderValue::Singleton(companion),
            });
        }
        if let Some(application) = self.nominal_application(ty) {
            let container = ["Option", "Array", "MutableArray", "ArrayList"]
                .iter()
                .any(|name| self.core_coding_nominal(name) == Some(application.template));
            if container {
                let element = self.select_field_decoder(context, application.arguments[0], span)?;
                let Some(companion) = companion else {
                    self.error(
                        span,
                        format!(
                            "core container {} requires its predefined decoder companion",
                            self.type_name(ty)
                        ),
                    );
                    return None;
                };
                return Some(SelectedDecoder {
                    ty,
                    value: DecoderValue::Container {
                        companion,
                        element: Box::new(element),
                    },
                });
            }
        }
        if ty == self.unit {
            let unit_decoder = self.core_coding_nominal("UnitDecoder")?;
            let singleton = self
                .apply_nominal_type(unit_decoder, Vec::new())
                .expect("UnitDecoder is an ordinary object");
            return Some(SelectedDecoder {
                ty,
                value: DecoderValue::Singleton(singleton),
            });
        }
        if let Type::Tuple(elements) = self.types[ty].clone() {
            let elements = elements
                .into_iter()
                .map(|ty| self.select_field_decoder(context, ty, span))
                .collect::<Option<Vec<_>>>()?;
            return Some(SelectedDecoder {
                ty,
                value: DecoderValue::Tuple(elements),
            });
        }
        self.error(span, format!("automatic decode has no Decodable<{}> field decoder; provide a primary val dependency, a conforming companion, or an explicit decode implementation", self.type_name(ty)));
        None
    }

    fn decoding_companion(&mut self, ty: TypeId, span: Span) -> Option<Option<TypeId>> {
        let owner = self
            .nominal_target_for_type(ty)
            .map(|target| self.nominal_identity(target.owner()).declaration_id())
            .or_else(|| self.imported_nominal_owner(ty));
        let Some(owner) = owner else {
            return Some(None);
        };
        let companion = if let Some(&owner) = self.nominal_owners.get(&owner) {
            self.companion_object(owner).map(|object| {
                self.nominal_identity(Owner::Object(object))
                    .declaration_id()
            })
        } else {
            let Some(declaration) = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(owner))
            else {
                return Some(None);
            };
            declaration
                .interface
                .declaration_details()
                .children()
                .values()
                .iter()
                .copied()
                .find(|child| self.nominal_is_companion(*child))
        };
        match companion {
            Some(companion) => self.apply_companion_type(ty, companion, span).map(Some),
            None => Some(None),
        }
    }
}
