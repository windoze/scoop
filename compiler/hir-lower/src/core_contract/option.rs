use super::*;

impl Lowerer {
    /// `scoop.core` must define exactly one enum named `Option` with
    /// exactly one type parameter (hir docs, spec 7.2). A second
    /// `Option` was already rejected as a duplicate enum in pass 1, so
    /// at most one candidate reaches here.
    pub(crate) fn validate_option_enum(&mut self, files: &[ast::SourceFile]) {
        let Some(&(id, file_index, span, type_param_count)) = self.option_candidates.first() else {
            // Attribute to a core source when the existing M22 core input is
            // present; a core-less test input falls back to its user source.
            let core_diagnostic_file = self.core_diagnostic_file();
            self.current_file = core_diagnostic_file;
            self.error(
                files[core_diagnostic_file].span,
                "scoop.core must define an enum `Option<T>`".to_string(),
            );
            return;
        };
        if type_param_count != 1 {
            self.current_file = file_index;
            self.error(
                span,
                format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {type_param_count}"
                ),
            );
            return;
        }
        self.pending_option_enum = Some(id);
    }

    /// Validate the complete source contract after enum variants have been
    /// resolved, then register the two variants as ordinary typed prelude
    /// bindings. No later source resolver recognizes `Some` or `None`
    /// specially.
    pub(crate) fn validate_option_variants(&mut self) {
        let Some(enumeration) = self.pending_option_enum else {
            return;
        };
        self.current_file = self.enum_files[&enumeration];
        let declaration = self.enums[enumeration].clone();
        let mut valid = true;

        if declaration.variants.len() != 2 {
            self.error(
                declaration.span,
                format!(
                    "enum `Option` in scoop.core must define exactly `Some(T)` and `None`, found {} variant(s)",
                    declaration.variants.len()
                ),
            );
            valid = false;
        }

        let some_index = declaration
            .variants
            .iter()
            .position(|variant| variant.name == "Some");
        let none_index = declaration
            .variants
            .iter()
            .position(|variant| variant.name == "None");
        let parameter = declaration.type_params[0].id;

        let some_valid = some_index.is_some_and(|index| {
            let variant = &declaration.variants[index];
            matches!(
                self.variant_styles.get(&(enumeration, index as u32)),
                Some(VariantStyle::Positional)
            ) && matches!(variant.fields.as_slice(), [field]
                if matches!(self.types[field.ty], Type::Param(found) if found == parameter))
        });
        if !some_valid {
            self.error(
                declaration.span,
                "enum `Option` in scoop.core must define positional variant `Some(T)`".to_string(),
            );
            valid = false;
        }

        let none_valid = none_index.is_some_and(|index| {
            declaration.variants[index].fields.is_empty()
                && matches!(
                    self.variant_styles.get(&(enumeration, index as u32)),
                    Some(VariantStyle::Unit)
                )
        });
        if !none_valid {
            self.error(
                declaration.span,
                "enum `Option` in scoop.core must define unit variant `None`".to_string(),
            );
            valid = false;
        }

        if !valid {
            return;
        }
        let some = hir::EnumVariantRef::checked(
            &self.enums,
            enumeration,
            some_index.expect("validated Option has Some") as u32,
        )
        .expect("validated Some index belongs to Option");
        let none = hir::EnumVariantRef::checked(
            &self.enums,
            enumeration,
            none_index.expect("validated Option has None") as u32,
        )
        .expect("validated None index belongs to Option");
        let some_payload = hir::EnumVariantFieldRef::checked(&self.enums, some, 0)
            .expect("validated Some has exactly one payload field");
        self.option_core = Some(
            hir::OptionCore::checked(&self.enums, &self.types, some_payload, none)
                .expect("validated Option variants form the checked HIR contract"),
        );
        self.pending_option_enum = None;
        self.core_prelude_variants.insert(
            declaration.variants[some.local_index() as usize]
                .name
                .clone(),
            some,
        );
        self.core_prelude_variants.insert(
            declaration.variants[none.local_index() as usize]
                .name
                .clone(),
            none,
        );
    }
}
