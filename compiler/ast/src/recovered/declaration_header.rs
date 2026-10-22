use crate::{Decl, DeclarationHeader, DeclarationHeaderKind as Kind, DeclarationInput};

impl DeclarationInput<'_> {
    /// Preserve known names when the enclosing file context is unavailable.
    pub fn unavailable_header(self) -> DeclarationHeader {
        let (name, visibility, kind) = match self {
            Self::Unavailable(header) => return header.clone(),
            Self::FunctionHeader(header) | Self::RecoveredFunction { header, .. } => {
                return function(header);
            }
            Self::Complete(declaration) => match declaration {
                Decl::Function(declaration) => return function(declaration.header()),
                Decl::Global(declaration) => (
                    &declaration.name,
                    declaration.visibility,
                    Kind::Property {
                        is_extension: declaration.receiver_ty.is_some(),
                    },
                ),
                Decl::TypeAlias(declaration) => {
                    (&declaration.name, declaration.visibility, Kind::Type)
                }
                Decl::AnnotationClass(declaration) => {
                    (&declaration.name, declaration.visibility, Kind::Type)
                }
                Decl::Struct(declaration) => {
                    (&declaration.name, declaration.visibility, Kind::Type)
                }
                Decl::Enum(declaration) => (&declaration.name, declaration.visibility, Kind::Type),
                Decl::Class(declaration) => (&declaration.name, declaration.visibility, Kind::Type),
                Decl::Interface(declaration) => {
                    (&declaration.name, declaration.visibility, Kind::Type)
                }
                Decl::Object(declaration) => {
                    (&declaration.name, declaration.visibility, Kind::Singleton)
                }
            },
            Self::RecoveredNominal(declaration) => match declaration {
                crate::RecoveredNominal::Class(declaration) => (
                    &declaration.header.name,
                    declaration.header.visibility,
                    Kind::Type,
                ),
                crate::RecoveredNominal::Struct(declaration) => (
                    &declaration.header.name,
                    declaration.header.visibility,
                    Kind::Type,
                ),
                crate::RecoveredNominal::Interface(declaration) => (
                    &declaration.header.name,
                    declaration.header.visibility,
                    Kind::Type,
                ),
                crate::RecoveredNominal::Enum(declaration) => (
                    &declaration.header.name,
                    declaration.header.visibility,
                    Kind::Type,
                ),
                crate::RecoveredNominal::Object(declaration) => (
                    &declaration.header.name,
                    declaration.header.visibility,
                    Kind::Singleton,
                ),
            },
        };
        let span = match self {
            Self::Complete(declaration) => declaration.span(),
            Self::RecoveredNominal(declaration) => declaration.span(),
            Self::FunctionHeader(_) | Self::RecoveredFunction { .. } | Self::Unavailable(_) => {
                unreachable!("function and unavailable headers returned above")
            }
        };
        DeclarationHeader {
            name: name.clone(),
            visibility,
            kind,
            span,
        }
    }
}

fn function(header: crate::FunctionHeaderRef<'_>) -> DeclarationHeader {
    DeclarationHeader {
        name: header.name.clone(),
        visibility: header.visibility,
        kind: Kind::Function {
            is_extension: header.receiver_ty.is_some(),
            is_operator: header.operator.is_some(),
            is_infix: header.infix.is_some(),
        },
        span: header.span,
    }
}
