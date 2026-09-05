use scoop_hir as hir;

use crate::VariantStyle;

use super::integers::IntegerDomain;

pub(super) enum ConstructorSpace {
    Closed(Vec<Constructor>),
    Open {
        prefix: Vec<Constructor>,
        remainder: Constructor,
    },
}

pub(super) enum Constructor {
    EnumVariant {
        variant: u32,
        name: String,
        style: VariantStyle,
        field_names: Vec<String>,
        field_types: Vec<hir::TypeId>,
    },
    Tuple {
        field_types: Vec<hir::TypeId>,
    },
    Struct {
        name: String,
        field_names: Vec<String>,
        field_types: Vec<hir::TypeId>,
    },
    Boolean(bool),
    Unit,
    String {
        value: String,
        /// The symbolic partition for every literal not explicitly present.
        remainder: bool,
    },
    /// An arbitrary value of a domain that has no match constructors.
    Opaque,
}

impl Constructor {
    pub(super) fn field_types(&self) -> &[hir::TypeId] {
        match self {
            Self::EnumVariant { field_types, .. }
            | Self::Tuple { field_types }
            | Self::Struct { field_types, .. } => field_types,
            Self::Boolean(_) | Self::Unit | Self::String { .. } | Self::Opaque => &[],
        }
    }

    pub(super) fn arity(&self) -> usize {
        self.field_types().len()
    }

    pub(super) fn string_value(&self) -> &str {
        let Self::String { value, .. } = self else {
            unreachable!("only String constructors enter the String prefix")
        };
        value
    }

    pub(super) fn into_witness(self, fields: Vec<Witness>) -> Witness {
        debug_assert_eq!(fields.len(), self.arity());
        match self {
            Self::EnumVariant {
                name,
                style,
                field_names,
                ..
            } => Witness::EnumVariant {
                name,
                style,
                field_names,
                fields,
            },
            Self::Tuple { .. } => Witness::Tuple(fields),
            Self::Struct {
                name, field_names, ..
            } => Witness::Struct {
                name,
                field_names,
                fields,
            },
            Self::Boolean(value) => Witness::Boolean(value),
            Self::Unit => Witness::Unit,
            Self::String { value, .. } => Witness::String(value),
            Self::Opaque => Witness::Opaque,
        }
    }
}

pub(super) enum Witness {
    EnumVariant {
        name: String,
        style: VariantStyle,
        field_names: Vec<String>,
        fields: Vec<Witness>,
    },
    Tuple(Vec<Witness>),
    Struct {
        name: String,
        field_names: Vec<String>,
        fields: Vec<Witness>,
    },
    Boolean(bool),
    Unit,
    Integer {
        domain: IntegerDomain,
        raw: u64,
    },
    String(String),
    Opaque,
}

impl Witness {
    pub(super) fn render(&self) -> String {
        match self {
            Self::EnumVariant {
                name,
                style,
                field_names,
                fields,
            } => {
                if fields.is_empty() {
                    return name.clone();
                }
                let fields = match style {
                    VariantStyle::Named => field_names
                        .iter()
                        .zip(fields)
                        .map(|(name, value)| format!("{name}: {}", value.render()))
                        .collect::<Vec<_>>()
                        .join(", "),
                    VariantStyle::Unit | VariantStyle::Positional | VariantStyle::Constructor => {
                        fields
                            .iter()
                            .map(Witness::render)
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                };
                if *style == VariantStyle::Named {
                    format!("{name} {{ {fields} }}")
                } else {
                    format!("{name}({fields})")
                }
            }
            Self::Tuple(fields) => {
                let trailing_comma = if fields.len() == 1 { "," } else { "" };
                let fields = fields
                    .iter()
                    .map(Witness::render)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({fields}{trailing_comma})")
            }
            Self::Struct {
                name,
                field_names,
                fields,
            } => {
                let fields = fields
                    .iter()
                    .zip(field_names)
                    .map(|(value, field)| format!("{field}: {}", value.render()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {fields} }}")
            }
            Self::Boolean(value) => value.to_string(),
            Self::Unit => "()".to_string(),
            Self::Integer { domain, raw } => domain.render(*raw),
            Self::String(value) => format!("{value:?}"),
            Self::Opaque => "_".to_string(),
        }
    }
}
