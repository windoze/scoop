use super::*;

#[derive(Default)]
pub(super) struct Control {
    pub hidden: bool,
    pub aliases: Vec<String>,
}

impl Projection<'_> {
    pub(super) fn collect_controls(
        &mut self,
        documentation: &hir::ConeDocumentation,
        warnings: &mut Vec<DocWarning>,
    ) -> Result<()> {
        for annotated in &self.export.annotations.targets {
            let mut control = Control::default();
            for application in &annotated.annotations {
                if Some(application.annotation) == documentation.hidden_annotation {
                    control.hidden = true;
                }
                if Some(application.annotation) == documentation.alias_annotation {
                    let [
                        hir::CanonicalAnnotationValueV1::Scalar(
                            hir::CanonicalConstValueV1::String(alias),
                        ),
                    ] = application.arguments.as_slice()
                    else {
                        unreachable!("the core DocAlias signature requires one static String")
                    };
                    if alias.trim().is_empty() {
                        warnings.push(DocWarning {
                            location: self.location(
                                application.definition_origin.file as usize,
                                application.definition_origin.span,
                            )?,
                            message: "documentation alias must not be blank".into(),
                            notes: Vec::new(),
                        });
                    } else if !control.aliases.contains(alias) {
                        control.aliases.push(alias.clone());
                    }
                }
            }
            self.controls.insert(annotated.target, control);
        }
        Ok(())
    }

    pub(super) fn property(&self, target: Target) -> Result<Option<PropertyCapability>> {
        let Target::Declaration(Source::Property(id)) = target else {
            return Ok(None);
        };
        let capability = self.export.properties[id].capability;
        let getter = capability.getter();
        let setter = capability.setter();
        Ok(Some(PropertyCapability {
            getter: self.accessor(
                Callable::Getter(getter),
                self.export.property_getters[getter].access.declared,
            )?,
            setter: setter
                .map(|id| {
                    self.accessor(
                        Callable::Setter(id),
                        self.export.property_setters[id].access.declared,
                    )
                })
                .transpose()?,
        }))
    }

    fn accessor(
        &self,
        callable: Callable,
        declared: hir::DeclaredVisibility,
    ) -> Result<AccessorCapability> {
        let target = Source::Callable(callable);
        Ok(AccessorCapability {
            key: self.source_target(target)?,
            visibility: visibility::label(declared),
            hidden: self
                .controls
                .get(&target)
                .is_some_and(|control| control.hidden),
        })
    }
}
