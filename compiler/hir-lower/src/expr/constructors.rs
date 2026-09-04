use super::*;

mod arrays;
mod ffi;

mod classes;
mod structs;
mod variants;

impl Lowerer {
    /// What a `Name` / `Name(...)` construction site resolves to. A
    /// dotted path `E.V` is always an enum variant; a bare name is a
    /// globally visible `Option` variant (`Some` / `None`), then a
    /// struct, then a class, then — for `Call` nodes only — a
    /// function.
    pub(super) fn classify_constructor(&mut self, name: &ast::Ident) -> Option<Constructor> {
        if let Some((enum_name, variant_name)) = name.text.split_once('.') {
            let Some(&enum_id) = self.enums_by_name.get(enum_name) else {
                self.error(name.span, format!("unknown enum `{enum_name}`"));
                return None;
            };
            if !self.enum_is_accessible(enum_id) {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` is not accessible here"),
                );
                return None;
            }
            let Some(variant) = self.find_variant(enum_id, variant_name) else {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` has no variant `{variant_name}`"),
                );
                return None;
            };
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some((enum_id, variant)) = self.option_variant(&name.text) {
            return Some(Constructor::Variant { enum_id, variant });
        }
        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) {
            if !self.nominal_is_accessible(ty) {
                self.error(
                    name.span,
                    format!("struct `{}` is not accessible here", name.text),
                );
                return None;
            }
            return Some(Constructor::Struct { struct_id, ty });
        }
        if let Some(&(class_id, ty)) = self.classes_by_name.get(&name.text) {
            if !self.nominal_is_accessible(ty) {
                self.error(
                    name.span,
                    format!("class `{}` is not accessible here", name.text),
                );
                return None;
            }
            return Some(Constructor::Class { class_id });
        }
        Some(Constructor::Unmatched)
    }
}
