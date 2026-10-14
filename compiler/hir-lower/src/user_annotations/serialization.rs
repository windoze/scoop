use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};

impl Lowerer {
    pub(super) fn validate_serialization_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[hir::SourceAnnotationApplication],
    ) {
        let serialize = self.core_source_annotation("Serialize");
        for annotation in annotations {
            if Some(annotation.annotation) != serialize {
                continue;
            }
            let config = SerializeConfig::from_arguments(&annotation.arguments);
            let span = annotation.definition_origin.span;
            let permitted = match target {
                hir::SourceAnnotationTarget::Variant(_) => true,
                hir::SourceAnnotationTarget::Field(_) => true,
                hir::SourceAnnotationTarget::VariantField(field) => {
                    let variant = field.variant();
                    self.enums[variant.enumeration()].variants[variant.local_index() as usize].style
                        != hir::VariantStyle::Positional
                }
                hir::SourceAnnotationTarget::Property(id) => matches!(
                    self.properties[id].representation,
                    hir::PropertyRepresentation::Stored(_)
                ),
                _ => false,
            };
            if !permitted {
                self.error(span, "`@Serialize` is not allowed on this target".into());
            }
            if config.ignore && (!config.rename.is_empty() || config.required || config.emit_null) {
                self.error(
                    span,
                    "Serialize ignore cannot be combined with rename, required, or emitNull".into(),
                );
            }
            if matches!(target, hir::SourceAnnotationTarget::Variant(_)) {
                if config.required || config.emit_null || config.ignore {
                    self.error(
                        span,
                        "Serialize on an enum variant only supports rename".into(),
                    );
                }
            } else if config.emit_null
                && !self
                    .serialization_target_type(target)
                    .is_some_and(|ty| self.as_option(ty).is_some())
            {
                self.error(
                    span,
                    "Serialize emitNull requires a field declared as core Option".into(),
                );
            }
        }
    }

    pub(crate) fn serialization_target_type(
        &self,
        target: hir::SourceAnnotationTarget,
    ) -> Option<hir::TypeId> {
        match target {
            hir::SourceAnnotationTarget::Field(field) => Some(
                self.structs[field.structure()].semantic_fields()[field.local_index() as usize].ty,
            ),
            hir::SourceAnnotationTarget::VariantField(field) => {
                let variant = field.variant();
                Some(
                    self.enums[variant.enumeration()].variants[variant.local_index() as usize]
                        .fields[field.local_index() as usize]
                        .ty,
                )
            }
            hir::SourceAnnotationTarget::Property(property) => Some(self.properties[property].ty),
            _ => None,
        }
    }

    /// Resolve the core declaration through its ordinary prelude binding.
    pub(crate) fn core_source_annotation(&self, name: &str) -> Option<PersistentAnnotationId> {
        if matches!(self.core, crate::CoreLoweringAuthority::Defined) {
            return self.source_annotations.iter().find_map(|(&id, input)| {
                (self.source_is_core(input.file)
                    && input.owner.is_none()
                    && input.identity.key().package().is_root()
                    && input.declaration.name.text == name)
                    .then_some(id)
            });
        }
        self.type_lookup_layers(name)
            .into_iter()
            .filter(|layer| layer.kind == ImportLookupLayer::CorePrelude)
            .flat_map(|layer| layer.candidates)
            .find_map(|candidate| match candidate.target {
                TypeLookupTarget::Current(TopLevelTypeTarget::Annotation(id)) => Some(id),
                TypeLookupTarget::Dependency(binding) => match binding.target() {
                    hir::ImportedTarget::Annotation(id) => Some(id.persistent()),
                    _ => None,
                },
                _ => None,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SerializeConfig {
    pub(crate) rename: String,
    pub(crate) required: bool,
    pub(crate) emit_null: bool,
    pub(crate) ignore: bool,
}

impl SerializeConfig {
    pub(crate) fn from_arguments(arguments: &[hir::CanonicalAnnotationValueV1]) -> Self {
        use hir::{CanonicalAnnotationValueV1::Scalar, CanonicalConstValueV1 as Value};
        let [
            Scalar(Value::String(rename)),
            Scalar(Value::Boolean(required)),
            Scalar(Value::Boolean(emit_null)),
            Scalar(Value::Boolean(ignore)),
        ] = arguments
        else {
            unreachable!("Serialize has its declared String and Boolean parameters")
        };
        Self {
            rename: rename.clone(),
            required: (*required).into(),
            emit_null: (*emit_null).into(),
            ignore: (*ignore).into(),
        }
    }

    pub(crate) fn wire_name(&self, name: &str) -> Option<String> {
        (!self.ignore).then(|| {
            if self.rename.is_empty() {
                name.into()
            } else {
                self.rename.clone()
            }
        })
    }
}
