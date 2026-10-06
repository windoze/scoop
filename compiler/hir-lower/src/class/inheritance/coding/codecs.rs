use super::*;
mod dependencies;
mod values;

pub(in crate::class::inheritance) struct SelectedCodec {
    pub(in crate::class::inheritance) ty: TypeId,
    value: CodecValue,
}

enum CodecValue {
    Dependency(CodecDependency),
    This,
    Singleton(TypeId),
    Container {
        companion: TypeId,
        element: Box<SelectedCodec>,
    },
    Tuple(Vec<SelectedCodec>),
}

enum CodecDependency {
    Field(hir::FieldRef, TypeId),
    Property(hir::PropertyId, TypeId),
}

impl Lowerer {
    pub(in crate::class::inheritance) fn select_field_codec(
        &mut self,
        context: CodingContext,
        ty: TypeId,
        span: Span,
    ) -> Option<SelectedCodec> {
        let interface = self
            .apply_nominal_type(context.interface, vec![ty])
            .expect("a coding protocol has one unconstrained parameter");
        let mut dependencies = self.coding_dependencies(context.owner, interface);
        if dependencies.len() > 1 {
            self.error(
                span,
                format!(
                    "automatic {} has ambiguous primary val dependencies for {}: {}",
                    context.direction.method(),
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
            CodecValue::Dependency(dependency)
        } else if self.types_equal(ty, context.target) {
            CodecValue::This
        } else {
            return self.select_default_codec(context, ty, interface, span);
        };
        Some(SelectedCodec { ty, value })
    }

    fn select_default_codec(
        &mut self,
        context: CodingContext,
        ty: TypeId,
        interface: TypeId,
        span: Span,
    ) -> Option<SelectedCodec> {
        let companion = self.coding_companion(ty, span)?;
        if let Some(companion) = companion
            && self.is_subtype(companion, interface)
        {
            return Some(SelectedCodec {
                ty,
                value: CodecValue::Singleton(companion),
            });
        }
        if let Some(application) = self.nominal_application(ty) {
            let container = ["Option", "Array", "MutableArray", "ArrayList"]
                .iter()
                .any(|name| self.core_coding_nominal(name) == Some(application.template));
            if container {
                let element = self.select_field_codec(context, application.arguments[0], span)?;
                let Some(companion) = companion else {
                    self.error(
                        span,
                        format!(
                            "core container {} requires its predefined {} companion",
                            self.type_name(ty),
                            context.direction.factory()
                        ),
                    );
                    return None;
                };
                return Some(SelectedCodec {
                    ty,
                    value: CodecValue::Container {
                        companion,
                        element: Box::new(element),
                    },
                });
            }
        }
        if ty == self.unit {
            let unit_codec = self.core_coding_nominal(context.direction.unit_codec())?;
            let singleton = self
                .apply_nominal_type(unit_codec, Vec::new())
                .expect("a Unit codec is an ordinary object");
            return Some(SelectedCodec {
                ty,
                value: CodecValue::Singleton(singleton),
            });
        }
        if let Type::Tuple(elements) = self.types[ty].clone() {
            let elements = elements
                .into_iter()
                .map(|ty| self.select_field_codec(context, ty, span))
                .collect::<Option<Vec<_>>>()?;
            return Some(SelectedCodec {
                ty,
                value: CodecValue::Tuple(elements),
            });
        }
        self.error(span, format!("automatic {} has no {}<{}> field codec; provide a primary val dependency, a conforming companion, or an explicit {} implementation", context.direction.method(), context.direction.protocol(), self.type_name(ty), context.direction.method()));
        None
    }

    fn coding_companion(&mut self, ty: TypeId, span: Span) -> Option<Option<TypeId>> {
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
