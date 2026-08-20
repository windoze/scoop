use super::*;
use ast::AnnotationUseSiteTarget as Site;
use hir::{
    SourceAnnotationCallable as Callable, SourceAnnotationReceiver as Receiver,
    SourceAnnotationTarget as Target,
};

impl Lowerer {
    pub(super) fn annotation_use_site(
        &mut self,
        target: Target,
        annotation: &ast::Annotation,
    ) -> Option<Target> {
        let Some(site) = annotation.use_site else {
            return Some(target);
        };
        let selected = match (target, site.target) {
            (Target::Property(property), kind) => self.property_annotation_site(property, kind),
            (Target::Field(_), Site::Field) => Some(target),
            (Target::Field(field), Site::Param) => self.structs[field.structure()].constructors.iter().copied()
                .find(|id| matches!(self.struct_constructors[*id].kind, hir::StructConstructorKind::Primary))
                .map(|id| Target::Parameter { callable: Callable::StructConstructor(id), index: field.local_index() }),
            (Target::Field(field), kind) => self.structs[field.structure()].properties.iter().copied()
                .find(|id| matches!(self.properties[*id].representation, hir::PropertyRepresentation::Stored(hir::StoredProperty { backing: hir::PropertyBacking::StructField { index, .. } }) if index == field.local_index()))
                .and_then(|id| self.property_annotation_site(id, kind)),
            (Target::VariantField(_), Site::Field) => Some(target),
            (Target::VariantField(field), Site::Param) => Some(Target::Parameter { callable: Callable::VariantConstructor(field.variant()), index: field.local_index() }),
            (Target::Parameter { .. }, Site::Param) => Some(target),
            (Target::Callable(Callable::Setter(setter)), Site::SetParam | Site::Param) => Some(Target::Parameter { callable: Callable::Setter(setter), index: 0 }),
            (Target::Callable(Callable::Function(function)), Site::Receiver)
                if self.functions[function].method.is_some() || self.extension_receivers.contains_key(&function) => {
                    Some(Target::Receiver(Receiver::Callable(Callable::Function(function))))
                }
            _ => None,
        };
        if selected.is_none() {
            self.error(
                site.span,
                format!(
                    "annotation use-site `{}` has no target on this declaration",
                    site.target.as_str()
                ),
            );
        }
        selected
    }

    pub(super) fn check_core_annotation_target(
        &mut self,
        target: Target,
        annotation: &ast::Annotation,
    ) {
        let permitted = match target {
            Target::Nominal(_) | Target::Property(_) => annotation.use_site.is_none(),
            Target::Callable(Callable::StructConstructor(_) | Callable::ClassConstructor(_)) => {
                matches!(
                    annotation.name.single_text(),
                    Some("Safe" | "Unsafe" | "NoGC")
                )
            }
            Target::Callable(_) => true,
            _ => false,
        };
        if !permitted {
            let location = if matches!(
                target,
                Target::Field(_) | Target::Variant(_) | Target::VariantField(_)
            ) {
                "a field or enum variant"
            } else {
                "this annotation target"
            };
            self.error(
                annotation.span,
                format!("`@{}` is not allowed on {location}", annotation.name),
            );
        }
    }
}
