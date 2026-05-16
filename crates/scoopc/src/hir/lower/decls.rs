//! Declaration graph lowering for typed HIR.

use super::*;

impl<'a> HirLowering<'a> {
    pub(super) fn lower_typealias_decl(
        &mut self,
        pkg_prefix: &str,
        decl: &ast::TypeAliasDecl,
    ) -> TypeAliasDecl {
        let name = decl.name.text(self.source).to_string();
        let fqn = join_prefix(pkg_prefix, &name);
        self.push_type_params(&decl.type_params);
        let type_params = self.lower_decl_type_params(&decl.type_params);
        let ty = self.lower_type_ref(&decl.ty);
        self.pop_type_params();
        TypeAliasDecl {
            span: decl.span,
            fqn,
            name,
            type_params,
            ty,
        }
    }

    pub(super) fn lower_nominal_decl(
        &mut self,
        owner_prefix: &str,
        decl: &ast::TypeDecl,
    ) -> NominalDecl {
        let name = decl.name.text(self.source).to_string();
        let fqn = join_prefix(owner_prefix, &name);
        self.push_type_params(&decl.type_params);
        let type_params = self.lower_decl_type_params(&decl.type_params);
        let supertypes = self.lower_supertype_decls(&decl.supertypes);
        let interfaces = self.interface_fqns(&supertypes);
        let constructors = self.lower_nominal_ctor_decls(decl);
        let mut members = self.lower_primary_ctor_field_members(&fqn, decl.primary_ctor.as_ref());
        members.extend(self.lower_decl_members(&fqn, decl.body.as_ref()));
        self.pop_type_params();
        NominalDecl {
            span: decl.span,
            fqn,
            name,
            kind: decl.kind,
            type_params,
            supertypes,
            interfaces,
            constructors,
            members,
        }
    }

    pub(super) fn lower_object_decl(
        &mut self,
        owner_prefix: &str,
        obj: &ast::ObjectDecl,
    ) -> Option<ObjectDecl> {
        let name = object_decl_name(self.source, obj)?;
        let fqn = join_prefix(owner_prefix, &name);
        let supertypes = self.lower_supertype_decls(&obj.supertypes);
        let interfaces = self.interface_fqns(&supertypes);
        let members = self.lower_decl_members(&fqn, obj.body.as_ref());
        Some(ObjectDecl {
            span: obj.span,
            fqn: fqn.clone(),
            name,
            kind: obj.kind,
            supertypes,
            interfaces,
            initializer_root: fqn,
            members,
        })
    }

    pub(super) fn lower_extension_property_decl(
        &mut self,
        pkg_prefix: &str,
        prop: &ast::ExtensionPropertyDecl,
    ) -> ExtensionPropertyDecl {
        let name = prop.name.text(self.source).to_string();
        let fqn = join_prefix(pkg_prefix, &name);
        self.push_type_params(&prop.type_params);
        let type_params = self.lower_decl_type_params(&prop.type_params);
        let receiver_ty = self.lower_type_ref(&prop.receiver);
        let ty = prop
            .ty
            .as_ref()
            .map(|ty| self.lower_type_ref(ty))
            .or_else(|| self.typechecked_fun_return_ty(prop.name.span))
            .unwrap_or(self.builtins.any);
        self.pop_type_params();
        ExtensionPropertyDecl {
            span: prop.span,
            fqn: fqn.clone(),
            name,
            mutable: matches!(prop.kind, ast::ValKind::Var),
            type_params,
            receiver_ty,
            ty,
            getter: prop
                .getter
                .as_ref()
                .map(|getter| self.accessor_contract(&fqn, getter)),
            setter: prop
                .setter
                .as_ref()
                .map(|setter| self.accessor_contract(&format!("{fqn}.set"), setter)),
        }
    }

    fn lower_decl_type_params(&self, params: &[ast::TypeParam]) -> Vec<DeclTypeParam> {
        params
            .iter()
            .map(|param| {
                let name = param.name.text(self.source).to_string();
                DeclTypeParam {
                    span: param.span,
                    ty: self.lookup_type_param(&name).unwrap_or(self.builtins.any),
                    name,
                    variance: param.variance,
                }
            })
            .collect()
    }

    fn lower_supertype_decls(&mut self, supertypes: &[ast::SuperType]) -> Vec<SupertypeDecl> {
        supertypes
            .iter()
            .map(|supertype| SupertypeDecl {
                span: supertype.span,
                fqn: self
                    .index
                    .type_ref_to_fqn_in_file(self.source, self.file, &supertype.ty),
                ty: self.lower_type_ref(&supertype.ty),
                ctor_arg_count: supertype.ctor_args.len(),
            })
            .collect()
    }

    fn interface_fqns(&self, supertypes: &[SupertypeDecl]) -> Vec<String> {
        let mut interfaces = supertypes
            .iter()
            .filter_map(|supertype| supertype.fqn.as_ref())
            .filter(|fqn| self.type_kinds.get(*fqn) == Some(&ast::TypeKind::Interface))
            .cloned()
            .collect::<Vec<_>>();
        interfaces.sort();
        interfaces.dedup();
        interfaces
    }

    fn lower_nominal_ctor_decls(&mut self, decl: &ast::TypeDecl) -> Vec<CtorDecl> {
        let mut ctors = Vec::new();
        if let Some(primary) = &decl.primary_ctor {
            ctors.push(CtorDecl {
                span: primary.params_span,
                kind: super::super::ClassCtorKind::Primary,
                params: primary
                    .params
                    .iter()
                    .map(|param| self.lower_ctor_param_decl(param))
                    .collect(),
                delegation: None,
            });
        }
        if let Some(body) = &decl.body {
            for member in &body.members {
                if let ast::TypeMember::SecondaryCtor(ctor) = member {
                    ctors.push(CtorDecl {
                        span: ctor.span,
                        kind: super::super::ClassCtorKind::Secondary,
                        params: ctor
                            .params
                            .iter()
                            .map(|param| self.lower_ctor_param_decl(param))
                            .collect(),
                        delegation: ctor.delegation_call.as_ref().map(|call| call.kind),
                    });
                }
            }
        }
        ctors
    }

    fn lower_decl_members(
        &mut self,
        owner_fqn: &str,
        body: Option<&ast::TypeBody>,
    ) -> Vec<DeclMember> {
        let Some(body) = body else {
            return Vec::new();
        };
        let mut members = Vec::new();
        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(variant) => {
                    members.push(DeclMember::EnumVariant(
                        self.lower_enum_variant_decl(owner_fqn, variant),
                    ));
                }
                ast::TypeMember::Property(prop) if prop.is_direct_field() => {
                    let name = prop.name.text(self.source).to_string();
                    members.push(DeclMember::Field(self.lower_field_decl(
                        owner_fqn,
                        prop.name.span,
                        &name,
                        matches!(prop.kind, ast::ValKind::Var),
                        prop.ty.as_ref(),
                        FieldOrigin::BodyProperty,
                    )));
                }
                ast::TypeMember::Property(prop) => {
                    members.push(DeclMember::Property(
                        self.lower_property_decl(owner_fqn, prop),
                    ));
                }
                ast::TypeMember::Fun(fun) => {
                    members.push(DeclMember::Fun(
                        self.lower_member_fun_contract(owner_fqn, fun),
                    ));
                }
                ast::TypeMember::InitBlock(init) => {
                    members.push(DeclMember::InitBlock { span: init.span });
                }
                ast::TypeMember::Type(nested) => {
                    members.push(DeclMember::Nested(Decl::Nominal(
                        self.lower_nominal_decl(owner_fqn, nested),
                    )));
                }
                ast::TypeMember::Object(obj) => {
                    if let Some(decl) = self.lower_object_decl(owner_fqn, obj) {
                        members.push(DeclMember::Nested(Decl::Object(decl)));
                    }
                }
                ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
        members
    }

    fn lower_primary_ctor_field_members(
        &mut self,
        owner_fqn: &str,
        primary_ctor: Option<&ast::PrimaryCtorDecl>,
    ) -> Vec<DeclMember> {
        let Some(primary_ctor) = primary_ctor else {
            return Vec::new();
        };
        primary_ctor
            .params
            .iter()
            .filter_map(|param| {
                let kind = param.kind?;
                let name = param.name.text(self.source).to_string();
                Some(DeclMember::Field(self.lower_field_decl(
                    owner_fqn,
                    param.name.span,
                    &name,
                    matches!(kind, ast::ValKind::Var),
                    param.ty.as_ref(),
                    FieldOrigin::PrimaryCtorParam,
                )))
            })
            .collect()
    }

    fn lower_property_decl(&mut self, owner_fqn: &str, prop: &ast::PropertyDecl) -> PropertyDecl {
        let name = prop.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");
        let ty = prop
            .ty
            .as_ref()
            .map(|ty| self.lower_type_ref(ty))
            .or_else(|| {
                prop.init
                    .as_ref()
                    .and_then(|init| self.typechecked_expr_ty(init.span))
            })
            .unwrap_or(self.builtins.any);
        PropertyDecl {
            span: prop.span,
            fqn: fqn.clone(),
            name,
            mutable: matches!(prop.kind, ast::ValKind::Var),
            ty,
            has_backing_field: prop.is_direct_field(),
            getter: prop
                .getter
                .as_ref()
                .map(|getter| self.accessor_contract(&fqn, getter)),
            setter: prop
                .setter
                .as_ref()
                .map(|setter| self.accessor_contract(&format!("{fqn}.set"), setter)),
        }
    }

    fn lower_member_fun_contract(&mut self, owner_fqn: &str, fun: &ast::FunDecl) -> MemberFunDecl {
        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");
        self.push_type_params(&fun.type_params);
        let type_params = self.lower_decl_type_params(&fun.type_params);
        let params = fun
            .params
            .iter()
            .map(|param| self.lower_ctor_param_decl(param))
            .collect();
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|ty| self.lower_type_ref(ty))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);
        self.pop_type_params();
        MemberFunDecl {
            span: fun.span,
            fqn,
            name,
            type_params,
            params,
            return_ty,
        }
    }

    fn lower_enum_variant_decl(
        &mut self,
        owner_fqn: &str,
        variant: &ast::EnumVariantDecl,
    ) -> EnumVariantDecl {
        let name = variant.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");
        let fields = variant
            .params
            .iter()
            .map(|param| {
                let name = param.name.text(self.source).to_string();
                self.lower_field_decl(
                    &fqn,
                    param.name.span,
                    &name,
                    false,
                    param.ty.as_ref(),
                    FieldOrigin::EnumVariantPayload,
                )
            })
            .collect();
        EnumVariantDecl {
            span: variant.span,
            fqn,
            name,
            fields,
        }
    }

    fn lower_field_decl(
        &mut self,
        owner_fqn: &str,
        span: Span,
        name: &str,
        mutable: bool,
        ty_ref: Option<&ast::TypeRef>,
        origin: FieldOrigin,
    ) -> FieldDecl {
        FieldDecl {
            span,
            fqn: format!("{owner_fqn}.{name}"),
            name: name.to_string(),
            mutable,
            ty: ty_ref
                .map(|ty| self.lower_type_ref(ty))
                .unwrap_or(self.builtins.any),
            origin,
        }
    }

    fn lower_ctor_param_decl(&mut self, param: &ast::Param) -> CtorParamDecl {
        CtorParamDecl {
            span: param.name.span,
            name: param.name.text(self.source).to_string(),
            ty: param
                .ty
                .as_ref()
                .map(|ty| self.lower_type_ref(ty))
                .unwrap_or(self.builtins.any),
            has_default: param.default_value.is_some(),
            property: param.kind,
        }
    }

    fn accessor_contract(&self, fqn: &str, accessor: &ast::AccessorDecl) -> AccessorContract {
        AccessorContract {
            span: accessor.span,
            fqn: fqn.to_string(),
        }
    }
}
