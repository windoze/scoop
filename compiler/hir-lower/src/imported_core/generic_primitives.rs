//! Generic bodies can require boxing only after their arguments become concrete.

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
            }
        }
        let mut visited = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            if let Some(application) = self.nominal_application(ty) {
                pending.extend(application.arguments.iter().copied());
                continue;
            }
            match &self.types[ty] {
                hir::Type::Tuple(elements) => pending.extend(elements.iter().copied()),
                hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                    let function = &self.function_types[*function];
                    pending.extend(function.parameter_types.iter().copied());
                    pending.push(function.return_type);
                }
                hir::Type::Ptr(pointee) => pending.push(*pointee),
                _ => self.retain_imported_box_source(ty)?,
            }
        }
        Ok(())
    }
}
