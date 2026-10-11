use super::*;

impl Lowerer {
    pub(crate) fn resolve_enum_supertypes(&mut self, id: EnumId, declaration: &ast::EnumDecl) {
        self.type_params_in_scope = self.enums[id].type_params.clone();
        let mut interfaces = Vec::new();
        for source in &declaration.interfaces {
            let Some(ty) = self.resolve_type_ref(source) else {
                continue;
            };
            match self.types[ty] {
                Type::Integer(kind) => {
                    if self.enums[id].representation.integer_kind().is_some() {
                        self.error(
                            source.span,
                            "an enum may have only one integer parent".into(),
                        );
                    } else {
                        self.enums[id].representation = hir::EnumRepresentation::Integral {
                            kind,
                            values: Vec::new(),
                        };
                    }
                }
                Type::Interface(_) => {
                    if self.check_sealed_supertype(ty, source.span)
                        && !interfaces.iter().any(|&other| self.types_equal(other, ty))
                    {
                        interfaces.push(ty);
                    }
                }
                _ => self.error(
                    source.span,
                    format!(
                        "enum parent must be a primitive integer or an interface, found {}",
                        self.type_name(ty),
                    ),
                ),
            }
        }
        self.enums[id].interfaces = interfaces;
        if self.enums[id].representation.integer_kind().is_some() {
            for method in &declaration.methods {
                if method.operator.is_some()
                    && matches!(
                        method.name.text.as_str(),
                        "unaryPlus"
                            | "unaryMinus"
                            | "inc"
                            | "dec"
                            | "plus"
                            | "minus"
                            | "times"
                            | "div"
                            | "rem"
                            | "plusAssign"
                            | "minusAssign"
                            | "timesAssign"
                            | "divAssign"
                            | "remAssign"
                            | "rangeTo"
                            | "rangeUntil"
                            | "compareTo"
                            | "equals"
                    )
                {
                    self.error(
                        method.name.span,
                        "integral enums use the integer parent's operators".into(),
                    );
                }
            }
        }
        self.type_params_in_scope.clear();
    }

    pub(crate) fn integral_enum_kind(&self, ty: TypeId) -> Option<hir::IntegerKind> {
        let Type::Enum(application) = self.types[ty] else {
            return None;
        };
        let owner = self.enum_applications[application].template;
        self.enum_definition(owner).representation.integer_kind()
    }
}
