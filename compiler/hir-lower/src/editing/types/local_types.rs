use super::*;
use crate::Owner;

impl Lowerer {
    /// Local nominal signatures and bodies use ordinary declaration scopes.
    /// Keep their source observations with the existing local-type cache.
    pub(crate) fn observe_local_type(
        &mut self,
        declaration: &ast::LocalTypeDecl,
        lower: impl FnOnce(&mut Self) -> Option<Owner>,
    ) -> Option<Owner> {
        if self.in_semantic_query || (self.editing_types.is_none() && self.editing_body.is_none()) {
            return lower(self);
        }
        let outer = self.editing_types.replace(TypeObservation::default());
        let file = self.current_file;
        let result = lower(self);
        let mut observed = self.editing_types.take().expect("local type observation");
        self.editing_types = outer;
        let owner = result?;
        let span = declaration.span();
        observed.uses.retain(|(source, origin), _| {
            *source == file && span.start <= origin.start && origin.end <= span.end
        });
        if !observed.uses.is_empty() {
            self.editing_local_types.insert(owner, observed);
        }
        if let Some(observed) = self.editing_local_types.get(&owner).cloned() {
            if let Some(outer) = &mut self.editing_types {
                outer.uses.extend(observed.uses);
            } else if self.observes_current_body() {
                self.editing_body
                    .as_mut()
                    .expect("active body observation")
                    .types
                    .uses
                    .extend(observed.uses);
            }
        }
        Some(owner)
    }
}
