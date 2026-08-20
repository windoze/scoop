use super::*;
use crate::imports::{ImportLookupLayer, lookup::TypeLookupTarget};

impl Lowerer {
    pub(super) fn validate_serialization_annotations(
        &mut self,
        target: hir::SourceAnnotationTarget,
        annotations: &[hir::SourceAnnotationApplication],
    ) {
        let serial_name = self.core_source_annotation("SerialName");
        let transient = self.core_source_annotation("Transient");
        let mut previous = self
            .annotation_metadata
            .targets
            .iter()
            .find(|entry| entry.target == target)
            .and_then(|entry| {
                entry.annotations.iter().find_map(|a| {
                    if Some(a.annotation) == serial_name {
                        Some("SerialName")
                    } else if Some(a.annotation) == transient {
                        Some("Transient")
                    } else {
                        None
                    }
                })
            });
        for annotation in annotations {
            let name = if Some(annotation.annotation) == serial_name {
                "SerialName"
            } else if Some(annotation.annotation) == transient {
                "Transient"
            } else {
                continue;
            };
            if previous.is_some_and(|name_before| name_before != name) {
                self.error(
                    annotation.definition_origin.span,
                    "SerialName and Transient cannot be used on the same target".into(),
                );
            }
            previous = Some(name);
            let permitted = match target {
                hir::SourceAnnotationTarget::Nominal(_)
                | hir::SourceAnnotationTarget::Annotation(_)
                | hir::SourceAnnotationTarget::Callable(_)
                | hir::SourceAnnotationTarget::Parameter { .. }
                | hir::SourceAnnotationTarget::Receiver(_)
                | hir::SourceAnnotationTarget::TypeAlias(_)
                | hir::SourceAnnotationTarget::ClassField(_)
                | hir::SourceAnnotationTarget::PropertyStorage { .. } => false,
                hir::SourceAnnotationTarget::Variant(_) => name == "SerialName",
                hir::SourceAnnotationTarget::Field(_) => true,
                hir::SourceAnnotationTarget::VariantField(field) => {
                    let variant = field.variant();
                    self.enums[variant.enumeration()].variants[variant.local_index() as usize].style
                        != hir::VariantStyle::Positional
                }
                hir::SourceAnnotationTarget::Property(id) => {
                    matches!(
                        self.properties[id].representation,
                        hir::PropertyRepresentation::Stored(_)
                    )
                }
            };
            if !permitted {
                self.error(
                    annotation.definition_origin.span,
                    format!("`@{name}` is not allowed on this target"),
                );
            }
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
