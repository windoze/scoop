use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_array_assembly(
        &mut self,
        assembly: &hir::DefaultArrayAssemblyV1,
        result_type: hir::TypeId,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ArrayAssembly, ImportedDefaultMaterializationError> {
        let element_type = self
            .imported_default_type_with_bindings(assembly.element_type(), context.bindings)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        let parts = assembly
            .parts()
            .iter()
            .map(|part| {
                Ok(match part {
                    hir::DefaultArrayAssemblyPartV1::Element(value) => {
                        hir::ArrayAssemblyPart::Element(
                            self.materialize_imported_default_expression(value, context)?,
                        )
                    }
                    hir::DefaultArrayAssemblyPartV1::CopyArray(value) => {
                        hir::ArrayAssemblyPart::CopyArray(
                            self.materialize_imported_default_expression(value, context)?,
                        )
                    }
                })
            })
            .collect::<Result<_, ImportedDefaultMaterializationError>>()?;
        Ok(hir::ArrayAssembly {
            element_type,
            parts,
            result_type,
        })
    }
}
