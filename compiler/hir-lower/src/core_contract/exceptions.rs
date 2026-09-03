use super::*;

impl Lowerer {
    pub(crate) fn compiler_exception(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
        throwable: ClassId,
    ) -> Option<hir::CompilerException> {
        let candidate = self.classes_by_name.get(name).map(|(id, _)| *id);
        let id = candidate.filter(|id| self.class_files[id] < self.user_file_index);
        let Some(id) = id else {
            self.current_file = candidate
                .and_then(|id| self.class_files.get(&id).copied())
                .unwrap_or(0);
            self.error(
                files[0].span,
                format!("scoop.core must define class `{name}`"),
            );
            return None;
        };
        self.current_file = self.class_files[&id];
        let declaration = &self.classes[id];
        let zero_arg = declaration
            .constructors
            .iter()
            .copied()
            .find(|constructor| self.class_constructors[*constructor].parameters.is_empty());
        let valid = declaration.modifier == hir::ClassModifier::Final
            && declaration.type_params.is_empty()
            && declaration.is_declared()
            && zero_arg.is_some()
            && self.class_descends_from(id, throwable);
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "class `{name}` in scoop.core must be a non-generic final subtype of `Throwable` with a zero-argument constructor"
                ),
            );
        }
        zero_arg.map(|constructor| hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor {
                class: id,
                constructor,
            },
        })
    }

    pub(crate) fn validate_exception_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::CompilerExceptionCore> {
        let throwable = self.throwable.map(|(id, _)| id)?;
        self.current_file = self.class_files[&throwable];
        let declaration = &self.classes[throwable];
        let zero_arg = declaration
            .constructors
            .iter()
            .copied()
            .find(|constructor| self.class_constructors[*constructor].parameters.is_empty());
        let valid_throwable = declaration.modifier == hir::ClassModifier::Open
            && declaration.type_params.is_empty()
            && declaration.is_declared()
            && zero_arg.is_some();
        if !valid_throwable {
            self.error(
                declaration.span,
                "class `Throwable` in scoop.core must be a non-generic open class with a zero-argument constructor"
                    .to_string(),
            );
        }
        let throwable = hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor {
                class: throwable,
                constructor: zero_arg?,
            },
        };
        Some(hir::CompilerExceptionCore {
            throwable,
            unwrap_exception: self.compiler_exception(
                "UnwrapException",
                files,
                throwable.class(),
            )?,
            class_cast_exception: self.compiler_exception(
                "ClassCastException",
                files,
                throwable.class(),
            )?,
            arithmetic_exception: self.compiler_exception(
                "ArithmeticException",
                files,
                throwable.class(),
            )?,
            index_out_of_bounds_exception: self.compiler_exception(
                "IndexOutOfBoundsException",
                files,
                throwable.class(),
            )?,
            illegal_state_exception: self.compiler_exception(
                "IllegalStateException",
                files,
                throwable.class(),
            )?,
        })
    }

    pub(crate) fn validate_throwable(&mut self, files: &[ast::SourceFile]) {
        let Some(&candidate) = self.throwable_candidates.first() else {
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define a class `Throwable`".to_string(),
            );
            return;
        };
        self.throwable = Some(candidate);
    }

    /// The `Throwable` reference type of `scoop.core`, when validated.
    /// `throw` / catch lowering skips its subtype check when this is
    /// `None` (the misconfigured core was already diagnosed, so the
    /// module is rejected anyway).
    pub(crate) fn throwable_ty(&self) -> Option<TypeId> {
        self.throwable.map(|(_, ty)| ty)
    }
}
