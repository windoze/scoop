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
                    export::LiteralPatternEquality::Integer { kind, target } => {
                        concrete::LiteralPatternEquality::Integer {
                            kind,
                            target: concrete::NoGcCallableRef::map_from_export(target, |source| {
                                self.lower_integer_callable(kind, source)
                            }),
                        }
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
                variant,
                fields,
            } => {
                let concrete_enum = self.lower_enum_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Enum(concrete_enum),
                    "the checked pattern application must match its subject"
                );
                let variant_id = concrete::VariantId::from_raw(*variant);
                let definition = self.enums[concrete_enum].variants[*variant as usize].clone();
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
                    enum_id: concrete_enum,
                    variant: variant_id,
                    fields,
                }
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
            export::Pattern::Struct {
                application,
                fields,
            } => {
                let concrete_struct = self.lower_struct_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Struct(concrete_struct),
                    "the checked pattern application must match its subject"
                );
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
}
