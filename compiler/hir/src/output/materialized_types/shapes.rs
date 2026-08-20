use super::*;

impl Collector<'_> {
    pub(super) fn children(&mut self, ty: TypeId) -> Result<(), MaterializedTypeClosureError> {
        ConcreteTypeRelations::from_module(self.module).visit_children(ty, &mut |ty| self.add(ty))
    }

    pub(super) fn function_type(
        &mut self,
        signature: FunctionTypeId,
    ) -> Result<(), MaterializedTypeClosureError> {
        ConcreteTypeRelations::from_module(self.module)
            .visit_function(signature, &mut |ty| self.add(ty))
    }
}
