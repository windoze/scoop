use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_pattern(
        &mut self,
        source: &export::Pattern,
        expected: concrete::TypeId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Pattern {
        match source {
            export::Pattern::Binding { local } => concrete::Pattern::Binding {
                local: self.lower_local(*local, locals),
            },
            export::Pattern::Wildcard => concrete::Pattern::Wildcard,
            export::Pattern::Literal {
                value,
                equality,
                subject_ty,
            } => concrete::Pattern::Literal {
                value: self.lower_expr(value, substitution, locals),
                equality: match *equality {
                    export::LiteralPatternEquality::Integer { kind } => {
                        concrete::LiteralPatternEquality::Integer { kind }
                    }
                    export::LiteralPatternEquality::Ordinary { equals } => {
                        concrete::LiteralPatternEquality::Ordinary {
                            equals: self.lower_callable(equals, substitution),
                        }
                    }
                },
                subject_ty: self.lower_type(*subject_ty, substitution),
            },
            export::Pattern::Variant {
                application,
                fields,
            } => {
                let variant = self.lower_enum_variant(*application, substitution);
                self.lower_enum_pattern(
                    variant.enumeration(),
                    variant.variant().into_raw(),
                    fields,
                    substitution,
                    locals,
                )
            }
            export::Pattern::Tuple(patterns) => {
                let concrete::TypeKind::Tuple(elements) = self.types[expected].kind.clone() else {
                    panic!("a resolved tuple pattern has a concrete tuple subject")
                };
                concrete::Pattern::Tuple(
                    patterns
                        .iter()
                        .zip(elements)
                        .map(|(pattern, expected)| {
                            self.lower_pattern(pattern, expected, substitution, locals)
                        })
                        .collect(),
                )
            }
            export::Pattern::Struct { owner, fields } => {
                let owner = self.lower_type(*owner, substitution);
                let concrete::TypeKind::Struct(concrete_struct) = self.types[owner].kind else {
                    unreachable!("a checked struct pattern has a concrete struct subject")
                };
                let definition = self.structs[concrete_struct].declared_fields().to_vec();
                concrete::Pattern::Struct {
                    struct_id: concrete_struct,
                    fields: fields
                        .iter()
                        .map(|(field, pattern)| {
                            (
                                *field,
                                self.lower_pattern(
                                    pattern,
                                    definition[*field as usize].ty,
                                    substitution,
                                    locals,
                                ),
                            )
                        })
                        .collect(),
                }
            }
        }
    }

    fn lower_enum_pattern(
        &mut self,
        enumeration: concrete::EnumId,
        index: u32,
        fields: &[(u32, export::Pattern)],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Pattern {
        let definition = self.enums[enumeration].variants[index as usize].clone();
        let fields = fields
            .iter()
            .map(|(field, pattern)| {
                (
                    *field,
                    self.lower_pattern(
                        pattern,
                        definition.fields[*field as usize].ty,
                        substitution,
                        locals,
                    ),
                )
            })
            .collect();
        concrete::Pattern::Variant {
            enum_id: enumeration,
            variant: concrete::VariantId::from_raw(index),
            fields,
        }
    }
}
