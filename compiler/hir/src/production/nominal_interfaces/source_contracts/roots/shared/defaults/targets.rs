use super::*;

impl SourceRoots {
    pub(super) fn default_callable(
        &mut self,
        export: &ExportHir,
        target: ExportDefaultCallableTarget,
        index: &super::super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        match target {
            ExportDefaultCallableTarget::Callable(callable) => {
                self.function(export, callable.function(export), roots)
            }
            ExportDefaultCallableTarget::FunctionAddress(function) => {
                self.callable_target(export, function, roots)
            }
            ExportDefaultCallableTarget::Bound(id) => self.bound(export, id, index, roots),
            ExportDefaultCallableTarget::DerivedEquality(id) => roots.require_field_type(
                export,
                index,
                export.derived_equality_applications[id].owner_ty,
            ),
            ExportDefaultCallableTarget::CallableReference(id) => {
                match &export.callable_references[id].target {
                    CallableReferenceTarget::Named(callable)
                    | CallableReferenceTarget::BoundExtension {
                        callee: callable, ..
                    } => self.callable_target(export, *callable, roots),
                    CallableReferenceTarget::BoundMember { callee, .. } => match *callee {
                        MethodCallee::Callable(callable) => {
                            self.callable_target(export, callable, roots)
                        }
                        MethodCallee::Bound(id) => self.bound(export, id, index, roots),
                        MethodCallee::DerivedEquality(id) => roots.require_field_type(
                            export,
                            index,
                            export.derived_equality_applications[id].owner_ty,
                        ),
                    },
                    // The lifted declaration and its dependencies are attached
                    // to this default body, rather than a top-level record.
                    CallableReferenceTarget::Local { .. }
                    | CallableReferenceTarget::BoundIntrinsic { .. } => Ok(()),
                }
            }
            // Imported calls keep the actual provider; attached lexical bodies
            // keep their own generated identity and reference closure.
            ExportDefaultCallableTarget::ImportedDependency(_)
            | ExportDefaultCallableTarget::ImportedGeneric(_)
            | ExportDefaultCallableTarget::LocalFunction(_)
            | ExportDefaultCallableTarget::Lambda(_)
            | ExportDefaultCallableTarget::AnonymousFunction(_) => Ok(()),
        }
    }

    fn bound(
        &mut self,
        export: &ExportHir,
        id: BoundCallableRefId,
        index: &super::super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let source = &export.bound_callable_refs[id];
        let ty = match source.source {
            BoundCallableSource::Class { bound, .. } => {
                export.class_applications[bound].canonical_type
            }
            BoundCallableSource::Interface { bound, .. } => {
                export.interface_applications[bound].canonical_type
            }
        };
        roots.require_field_type(export, index, ty)?;
        self.callable_target(export, source.declared_callable(), roots)
    }

    fn callable_target(
        &mut self,
        export: &ExportHir,
        target: CallableTarget,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        match target {
            CallableTarget::Local(callable) => {
                self.function(export, callable.function(export), roots)
            }
            CallableTarget::Application(_) | CallableTarget::Dependency(_) => Ok(()),
        }
    }
}
