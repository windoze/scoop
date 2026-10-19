use super::*;
use hir::{AnnotationTypeSlot as Slot, SourceAnnotationTarget as Target};

impl Lowerer {
    pub(crate) fn annotate_type_site(&mut self, target: Target, slot: Slot, source: &ast::TypeRef) {
        let key = TypeUseKey::new(self.current_file, source.span);
        if self.type_annotations.uses.contains_key(&key) {
            self.type_annotations.sites.push((target, slot, key));
        }
    }

    pub(crate) fn annotate_type_bounds(
        &mut self,
        target: Target,
        parameters: &[ast::TypeParamDecl],
        constraints: Option<&ast::WhereClause>,
    ) {
        for (parameter, source) in parameters.iter().enumerate() {
            let mut bounds = Vec::new();
            if let Some(ast::TypeBound::Upper(ty)) = &source.inline_bound {
                bounds.push(ty);
            }
            if let Some(constraints) = constraints {
                bounds.extend(constraints.constraints.iter().filter_map(|constraint| {
                    match &constraint.bound {
                        ast::TypeBound::Upper(ty)
                            if constraint.subject.parameter().text == source.name.text =>
                        {
                            Some(ty)
                        }
                        _ => None,
                    }
                }));
            }
            for (index, ty) in bounds.into_iter().enumerate() {
                self.annotate_type_site(
                    target,
                    Slot::Bound {
                        parameter: parameter as u32,
                        index: index as u32,
                    },
                    ty,
                );
            }
        }
    }

    pub(crate) fn annotate_supertypes(&mut self, target: Target, types: &[ast::TypeRef]) {
        for (index, ty) in types.iter().enumerate() {
            self.annotate_type_site(target, Slot::Supertype(index as u32), ty);
        }
    }

    pub(crate) fn annotate_parent_specs(&mut self, target: Target, types: &[ast::SupertypeSpec]) {
        for (index, parent) in types.iter().enumerate() {
            self.annotate_type_site(target, Slot::Supertype(index as u32), &parent.ty);
        }
    }
}
