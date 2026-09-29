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
            export::Pattern::ImportedVariant {
                owner,
                variant,
                fields,
            } => {
                let concrete_owner = self.lower_type(*owner, substitution);
                assert_eq!(
                    concrete_owner, expected,
                    "the variant owner matches its subject"
                );
                let concrete::TypeKind::Enum(enumeration) = self.types[concrete_owner].kind else {
                    unreachable!("an imported enum pattern has a concrete enum representation")
                };
                let index = self.enums[enumeration]
                    .variants
                    .iter()
                    .position(|value| value.identity == *variant)
                    .expect("the pattern retains an actual variant of its dependency enum");
                self.lower_enum_pattern(enumeration, index as u32, fields, substitution, locals)
            }
            export::Pattern::Variant {
                application,
                variant,
                fields,
            } => {
                let concrete_enum = self.lower_enum_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Enum(concrete_enum),
                    "the checked pattern application must match its subject"
                );
                self.lower_enum_pattern(concrete_enum, *variant, fields, substitution, locals)
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
