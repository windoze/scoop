use super::*;
use crate::aliases::{AliasOrigin, AliasTemplate};

impl Lowerer {
    pub(crate) fn record_alias_type_annotations(
        &mut self,
        template: &AliasTemplate,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
    ) {
        if self.type_annotations.suspended {
            return;
        }
        let Some(frame) = self.type_annotations.frames.last() else {
            return;
        };
        if frame.selector != Some(name.span) {
            return;
        }
        let mut actual = (0..template.outer_parameter_count)
            .map(|index| {
                frame.host.map(|key| TypeUsePart {
                    key,
                    from: vec![TypeUseStep::Argument(index as u32)],
                    to: Vec::new(),
                })
            })
            .collect::<Vec<_>>();
        actual.extend(arguments.iter().map(|argument| {
            Some(TypeUsePart {
                key: TypeUseKey::new(self.current_file, argument.span),
                from: Vec::new(),
                to: Vec::new(),
            })
        }));
        let annotations = match &template.origin {
            AliasOrigin::Source(id) => {
                let (file, span) = self.source_alias_type_use(*id);
                AliasAnnotations::Source(TypeUseKey::new(file, span))
            }
            AliasOrigin::Imported(binding) => {
                let hir::ImportedTarget::TypeAlias(id) = binding.target() else {
                    unreachable!("an alias template retains its alias binding");
                };
                let nodes = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| {
                        dependencies.annotated_type(
                            hir::AnnotationTargetV1::TypeAlias(id.persistent()),
                            hir::AnnotationTypeSlot::Value,
                        )
                    })
                    .map_or_else(Vec::new, |usage| usage.nodes.clone());
                AliasAnnotations::Imported(nodes)
            }
        };
        self.type_annotations
            .frames
            .last_mut()
            .expect("the alias resolution retains its type frame")
            .alias = Some(AliasTypeUse {
            target: template.target,
            parameters: template
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect(),
            annotations,
            arguments: actual,
        });
    }
}
