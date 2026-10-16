use super::*;

impl Concretizer<'_> {
    pub(super) fn metadata_fields(&self, ty: concrete::TypeId) -> Vec<concrete::TypeId> {
        let concrete::TypeKind::Struct(id) = self.types[ty].kind else {
            unreachable!("a metadata record is an ordinary struct")
        };
        let concrete::StructRepresentation::Declared(record) = &self.structs[id].representation
        else {
            unreachable!("a metadata record has declared fields")
        };
        record.fields.iter().map(|field| field.ty).collect()
    }

    pub(super) fn metadata_record(
        &self,
        ty: concrete::TypeId,
        args: Vec<concrete::Expr>,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let concrete::TypeKind::Struct(struct_id) = self.types[ty].kind else {
            unreachable!("a metadata record is an ordinary struct")
        };
        debug_assert_eq!(
            self.metadata_fields(ty),
            args.iter().map(|arg| arg.ty).collect::<Vec<_>>()
        );
        value(
            concrete::ExprKind::StructInit { struct_id, args },
            ty,
            origin,
        )
    }

    pub(super) fn metadata_string(
        &mut self,
        text: String,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let ty = self.lower_type(self.source.string, &[]);
        value(
            concrete::ExprKind::StringLiteral {
                value: text,
                owner: export::StringConstantOwner::CurrentDefinition,
            },
            ty,
            origin,
        )
    }

    pub(super) fn metadata_option(
        &self,
        ty: concrete::TypeId,
        item: Option<concrete::Expr>,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let concrete::TypeKind::Enum(enumeration) = self.types[ty].kind else {
            unreachable!("an optional metadata relationship has the core Option type")
        };
        let (some, none) = match self.core {
            export::CoreProtocols::Defined(core) => (
                self.source.enum_member_identities[core.option.some()].id(),
                self.source.enum_member_identities[core.option.none()].id(),
            ),
            export::CoreProtocols::Imported(core) => (
                core.option().some().persistent(),
                core.option().none().persistent(),
            ),
        };
        let identity = if item.is_some() { some } else { none };
        let index = self.enums[enumeration]
            .variants
            .iter()
            .position(|variant| variant.identity == identity)
            .expect("a core Option application retains its declared variants");
        let variant = concrete::EnumVariantRef::checked(
            &self.enums,
            enumeration,
            concrete::VariantId::from_raw(index as u32),
        )
        .expect("the selected Option variant exists");
        value(
            concrete::ExprKind::VariantConstruct {
                variant,
                args: item.into_iter().collect(),
            },
            ty,
            origin,
        )
    }
}

impl Concretizer<'_> {
    pub(super) fn metadata_element(&self, ty: concrete::TypeId) -> concrete::TypeId {
        let concrete::TypeKind::Class(array) = self.types[ty].kind else {
            unreachable!("metadata sequences are core Arrays")
        };
        self.classes[array].type_arguments[0]
    }

    pub(super) fn metadata_optional_element(&self, ty: concrete::TypeId) -> concrete::TypeId {
        let concrete::TypeKind::Enum(option) = self.types[ty].kind else {
            unreachable!("metadata optional values are core Options")
        };
        self.enums[option].type_arguments[0]
    }

    pub(super) fn metadata_array(
        &self,
        ty: concrete::TypeId,
        items: Vec<concrete::Expr>,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        value(concrete::ExprKind::ArrayLiteral(items), ty, origin)
    }

    pub(super) fn metadata_variant(
        &self,
        ty: concrete::TypeId,
        name: &str,
    ) -> (concrete::EnumVariantRef, Vec<concrete::TypeId>) {
        let concrete::TypeKind::Enum(id) = self.types[ty].kind else {
            unreachable!("metadata alternatives are ordinary enums")
        };
        let index = self.enums[id]
            .variants
            .iter()
            .position(|variant| variant.name == name)
            .expect("the checked core metadata enum has this alternative");
        let variant = concrete::EnumVariantRef::checked(
            &self.enums,
            id,
            concrete::VariantId::from_raw(index as u32),
        )
        .unwrap();
        (
            variant,
            self.enums[id].variants[index]
                .fields
                .iter()
                .map(|field| field.ty)
                .collect(),
        )
    }

    pub(super) fn metadata_case(
        &self,
        ty: concrete::TypeId,
        name: &str,
        args: Vec<concrete::Expr>,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let (variant, fields) = self.metadata_variant(ty, name);
        debug_assert_eq!(fields, args.iter().map(|arg| arg.ty).collect::<Vec<_>>());
        value(
            concrete::ExprKind::VariantConstruct { variant, args },
            ty,
            origin,
        )
    }

    pub(super) fn metadata_identity(
        &mut self,
        ty: concrete::TypeId,
        identity: String,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let identity = self.metadata_string(identity, origin);
        self.metadata_record(ty, vec![identity], origin)
    }

    pub(super) fn metadata_bool(
        &mut self,
        boolean: bool,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let ty = self.intern_type(concrete::TypeKind::Boolean, true);
        value(concrete::ExprKind::BoolLiteral(boolean), ty, origin)
    }

    pub(super) fn metadata_index(
        &mut self,
        index: usize,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let ty = self.lower_integer_type(export::IntegerKind::SIGNED_64, &[]);
        value(
            concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(index as u64)),
            ty,
            origin,
        )
    }

    pub(super) fn metadata_visibility(
        &self,
        ty: concrete::TypeId,
        visibility: export::DeclaredVisibilityV1,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let name = match visibility {
            export::DeclaredVisibilityV1::Public => "Public",
            export::DeclaredVisibilityV1::Internal => "Internal",
            export::DeclaredVisibilityV1::Private => "Private",
            export::DeclaredVisibilityV1::Protected => "Protected",
        };
        self.metadata_case(ty, name, Vec::new(), origin)
    }
}
