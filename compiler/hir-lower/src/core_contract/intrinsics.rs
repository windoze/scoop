use super::*;

impl Lowerer {
    pub(crate) fn register_intrinsic_function(
        &mut self,
        function: FunctionId,
        intrinsic: hir::IntrinsicFunction,
        span: Span,
    ) {
        if let Some(&(previous, previous_provider)) = self.intrinsic_functions.get(&intrinsic.kind)
        {
            let previous_name = self.functions[previous].name.clone();
            self.error(
                span,
                format!(
                    "intrinsic `{}` is already defined by provider {} as `{previous_name}`; provider {} cannot define it again",
                    intrinsic.kind.name(),
                    previous_provider.into_raw(),
                    intrinsic.provider.into_raw(),
                ),
            );
            return;
        }
        self.intrinsic_functions
            .insert(intrinsic.kind, (function, intrinsic.provider));
    }

    pub(crate) fn validate_intrinsic_type_source_shape(
        &mut self,
        spec: &'static hir::IntrinsicTypeSpec,
        name: &ast::Ident,
        parameters: &[ast::TypeParamDecl],
        where_clause: Option<&ast::WhereClause>,
        representation_omitted: bool,
        span: Span,
    ) {
        if name.text != spec.kind.source_name() {
            self.error(
                name.span,
                format!(
                    "intrinsic type `{}` must be declared with source name `{}`",
                    spec.name,
                    spec.kind.source_name()
                ),
            );
        }
        if !representation_omitted {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` must omit its fields or primary constructor",
                    spec.name
                ),
            );
        }
        let valid_parameters = match spec.kind.parameters() {
            hir::IntrinsicTypeParameters::None => parameters.is_empty(),
            hir::IntrinsicTypeParameters::OneInvariantUnconstrained => {
                matches!(parameters, [parameter] if parameter.inline_bound.is_none())
                    && where_clause.is_none()
            }
        };
        if !valid_parameters {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` has an invalid type-parameter declaration",
                    spec.name
                ),
            );
        }
    }

    pub(crate) fn register_intrinsic_type(
        &mut self,
        intrinsic: hir::IntrinsicTypeDeclaration,
        owner: IntrinsicTypeOwner,
        span: Span,
    ) {
        if let Some(&(_previous, previous_provider)) =
            self.intrinsic_type_owners.get(&intrinsic.kind)
        {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` is already defined by provider {}; provider {} cannot define it again",
                    intrinsic.kind.name(),
                    previous_provider.into_raw(),
                    intrinsic.provider.into_raw(),
                ),
            );
            return;
        }
        self.intrinsic_type_owners
            .insert(intrinsic.kind, (owner, intrinsic.provider));
    }

    pub(crate) fn validate_intrinsic_type_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::IntrinsicTypeCore> {
        let require = |this: &mut Self, kind: hir::IntrinsicTypeKind| {
            let Some(&(owner, _provider)) = this.intrinsic_type_owners.get(&kind) else {
                this.current_file = 0;
                this.error(
                    files[0].span,
                    format!(
                        "scoop.core must define exactly one `{}` intrinsic type",
                        kind.name()
                    ),
                );
                return None;
            };
            Some(owner)
        };
        let int = require(self, hir::IntrinsicTypeKind::Int)?;
        let uint = require(self, hir::IntrinsicTypeKind::UInt)?;
        let boolean = require(self, hir::IntrinsicTypeKind::Boolean)?;
        let string = require(self, hir::IntrinsicTypeKind::String)?;
        let array = require(self, hir::IntrinsicTypeKind::Array)?;
        let mutable_array = require(self, hir::IntrinsicTypeKind::MutableArray)?;
        let (
            IntrinsicTypeOwner::Struct(int),
            IntrinsicTypeOwner::Struct(uint),
            IntrinsicTypeOwner::Struct(boolean),
            IntrinsicTypeOwner::Class(string),
            IntrinsicTypeOwner::Class(array),
            IntrinsicTypeOwner::Class(mutable_array),
        ) = (int, uint, boolean, string, array, mutable_array)
        else {
            unreachable!("the intrinsic registry fixes every declaration target")
        };
        Some(hir::IntrinsicTypeCore {
            int,
            uint,
            boolean,
            string,
            array,
            mutable_array,
        })
    }
}
