//! Generic arguments retain source declarations needed only after concretization.

use super::*;

impl Lowerer {
    pub(crate) fn retain_generic_primitive_declarations(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        if !matches!(self.core, CoreLoweringAuthority::Imported(_)) {
            return Ok(());
        }
        let mut pending = self
            .instantiations
            .iter()
            .flat_map(|(_, application)| application.type_args.iter().copied())
            .chain(
                self.generic_method_applications
                    .iter()
                    .flat_map(|(_, application)| application.method_arguments.iter().copied()),
            )
            .collect::<Vec<_>>();
        for (_, application) in self.imported_generic_applications.iter() {
            pending.extend(application.arguments.substitution(
                &self.types,
                &self.enum_applications,
                &self.struct_applications,
                &self.class_applications,
                &self.interface_applications,
            ));
        }
        for (ty, _) in self.types.iter() {
            if let Some(application) = self.nominal_application(ty) {
                pending.extend(application.arguments.iter().copied());
            } else if matches!(self.types[ty], hir::Type::Ptr(_)) {
                pending.push(ty);
            }
        }
        let mut visited = std::collections::HashSet::new();
        let companions = self.conditional_interfaces_use_companions();
        while let Some(ty) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            if companions {
                self.retain_conditional_companion(ty)?;
            }
            if let Some(application) = self.nominal_application(ty) {
                pending.extend(application.arguments.iter().copied());
                continue;
            }
            match &self.types[ty] {
                hir::Type::Tuple(elements) => pending.extend(elements.iter().copied()),
                hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                    let function = &self.function_types[*function];
                    pending.extend(function.parameters().copied());
                    pending.push(function.return_type);
                }
                hir::Type::Ptr(pointee) => {
                    pending.push(*pointee);
                    self.retain_imported_box_source(ty)?;
                }
                _ => self.retain_imported_box_source(ty)?,
            }
        }
        Ok(())
    }
}
