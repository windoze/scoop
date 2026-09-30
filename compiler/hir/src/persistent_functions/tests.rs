use la_arena::Arena;
use scoop_ast::Span;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

use super::*;
use crate::{
    CallableBodyTypeArguments, CallableModifiers, DeclarationAccess, FunctionAttributes,
    FunctionGenericity, FunctionKind, HirConstructorIdentityInputs, HirNominalIdentities,
    HirPropertyIdentities, HirTypeIdentityInputs, IntegerKind, IntegerTypeCore, IntrinsicTypeCore,
};

struct Fixture {
    functions: Arena<Function>,
    lambdas: Arena<Lambda>,
    anonymous_functions: Arena<AnonymousFunction>,
    local_functions: Arena<crate::LocalFunction>,
    property_getters: Arena<PropertyGetter>,
    property_setters: Arena<PropertySetter>,
    property_accessor_identities: HirPropertyAccessorIdentities,
    initialization_units: Arena<InitializationUnit>,
    initialization_unit_identities: HirInitializationUnitIdentities,
    derived_equality_applications: Arena<DerivedEqualityApplication>,
    structs: Arena<StructDecl>,
    enums: Arena<EnumDecl>,
    type_identities: HirTypeIdentities,
    struct_constructors: Arena<StructConstructor>,
    class_constructors: Arena<ClassConstructor>,
    constructor_identities: HirConstructorIdentities,
    enum_member_identities: HirEnumMemberIdentities,
}

impl Fixture {
    fn empty() -> Self {
        let structs = Arena::new();
        let enums = Arena::new();
        let classes = Arena::new();
        let interfaces = Arena::new();
        let objects = Arena::new();
        let nominal_identities = HirNominalIdentities::checked(
            &structs,
            Vec::new(),
            &enums,
            Vec::new(),
            &classes,
            Vec::new(),
            &interfaces,
            Vec::new(),
            &objects,
            Vec::new(),
        )
        .unwrap();

        let types = Arena::new();
        let function_types = Arena::new();
        let struct_applications = Arena::new();
        let enum_applications = Arena::new();
        let class_applications = Arena::new();
        let interface_applications = Arena::new();
        let intrinsic_core = IntrinsicTypeCore {
            integers: IntegerTypeCore::new(std::array::from_fn(|index| {
                crate::StructId::from_raw((index as u32).into())
            }))
            .unwrap(),
            boolean: crate::StructId::from_raw((IntegerKind::COUNT as u32).into()),
            string: crate::ClassId::from_raw(0_u32.into()),
            array: crate::ClassId::from_raw(1_u32.into()),
            mutable_array: crate::ClassId::from_raw(2_u32.into()),
            ptr: crate::StructId::from_raw(9_u32.into()),
            fun_ptr: crate::StructId::from_raw(10_u32.into()),
        };
        let type_inputs = HirTypeIdentityInputs {
            types: &types,
            function_types: &function_types,
            structs: &structs,
            struct_applications: &struct_applications,
            enums: &enums,
            loaded_enum_definitions: &std::collections::HashMap::new(),
            loaded_struct_definitions: &std::collections::HashMap::new(),
            enum_applications: &enum_applications,
            classes: &classes,
            class_applications: &class_applications,
            interfaces: &interfaces,
            interface_applications: &interface_applications,
            objects: &objects,
            core_types: crate::HirCoreTypeIdentityAuthority::Defined(&intrinsic_core),
            nominal_identities: &nominal_identities,
        };
        let type_identities = HirTypeIdentities::from_types(type_inputs).unwrap();

        let struct_constructors = Arena::new();
        let class_constructors = Arena::new();
        let class_constructor_applications = Arena::new();
        let constructor_identities = HirConstructorIdentities::checked(
            HirConstructorIdentityInputs {
                type_inputs,
                struct_constructors: &struct_constructors,
                class_constructors: &class_constructors,
                class_constructor_applications: &class_constructor_applications,
            },
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let enum_member_identities =
            HirEnumMemberIdentities::from_declarations(&enums, &nominal_identities).unwrap();

        let properties = Arena::new();
        let extension_properties = Arena::new();
        let property_identities =
            HirPropertyIdentities::checked(&properties, Vec::new(), &extension_properties).unwrap();
        let property_getters = Arena::new();
        let property_setters = Arena::new();
        let property_accessor_identities = HirPropertyAccessorIdentities::checked(
            &properties,
            &property_identities,
            &property_getters,
            Vec::new(),
            &property_setters,
            Vec::new(),
        )
        .unwrap();

        let initialization_units = Arena::new();
        let failure_roots = Arena::new();
        let functions = Arena::new();
        let globals = Arena::new();
        let companions = Arena::new();
        let singleton_values = Arena::new();
        let published_roots = Arena::new();
        let delegate_storages = Arena::new();
        let initialization_unit_identities = HirInitializationUnitIdentities::from_declarations(
            &initialization_units,
            &failure_roots,
            &functions,
            &globals,
            &objects,
            &companions,
            &singleton_values,
            &published_roots,
            &properties,
            &delegate_storages,
            &Arena::new(),
            &nominal_identities,
            &property_identities,
        )
        .unwrap();

        Self {
            functions,
            lambdas: Arena::new(),
            anonymous_functions: Arena::new(),
            local_functions: Arena::new(),
            property_getters,
            property_setters,
            property_accessor_identities,
            initialization_units,
            initialization_unit_identities,
            derived_equality_applications: Arena::new(),
            structs,
            enums,
            type_identities,
            struct_constructors,
            class_constructors,
            constructor_identities,
            enum_member_identities,
        }
    }

    fn checked(
        &self,
        identities: Vec<HirFunctionIdentity>,
    ) -> Result<HirFunctionIdentities, HirFunctionIdentityError> {
        HirFunctionIdentities::checked(
            HirFunctionIdentityInputs {
                functions: &self.functions,
                lambdas: &self.lambdas,
                anonymous_functions: &self.anonymous_functions,
                local_functions: &self.local_functions,
                property_getters: &self.property_getters,
                property_setters: &self.property_setters,
                property_accessor_identities: &self.property_accessor_identities,
                initialization_units: &self.initialization_units,
                initialization_unit_identities: &self.initialization_unit_identities,
                derived_equality_applications: &self.derived_equality_applications,
                structs: &self.structs,
                enums: &self.enums,
                type_identities: &self.type_identities,
                struct_constructors: &self.struct_constructors,
                class_constructors: &self.class_constructors,
                constructor_identities: &self.constructor_identities,
                enum_member_identities: &self.enum_member_identities,
            },
            identities,
        )
    }
}

fn source_key(name: &str, type_parameter_count: u32) -> SourceDeclarationKey {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        type_parameter_count,
        None,
        Vec::new(),
    )
}

fn function(name: &str) -> Function {
    Function {
        name: name.to_string(),
        access: DeclarationAccess::public(),
        genericity: FunctionGenericity::Plain,
        is_suspend: false,
        modifiers: CallableModifiers::default(),
        params: Vec::new(),
        return_ty: crate::TypeId::from_raw(0_u32.into()),
        attributes: FunctionAttributes::default(),
        kind: FunctionKind::User(crate::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        }),
        method: None,
        span: Span { start: 0, end: 0 },
    }
}

fn path(ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, ordinal),
        [],
    )
}

#[test]
fn source_identity_uses_own_declaration_genericity() {
    let plain = HirSourceFunctionIdentity::from_declaration(source_key("plain", 0)).unwrap();
    let generic = HirSourceFunctionIdentity::from_declaration(source_key("generic", 1)).unwrap();
    assert!(matches!(plain, HirSourceFunctionIdentity::Plain(_)));
    assert!(matches!(generic, HirSourceFunctionIdentity::Generic(_)));
}

#[test]
fn checked_relation_is_total_and_rejects_duplicate_source_identity() {
    let mut fixture = Fixture::empty();
    let first = fixture.functions.alloc(function("first"));
    let second = fixture.functions.alloc(function("second"));
    let source = HirSourceFunctionIdentity::from_declaration(source_key("same", 0)).unwrap();

    assert!(matches!(
        fixture.checked(vec![HirFunctionIdentity::source(source.clone())]),
        Err(HirFunctionIdentityError::Length {
            expected: 2,
            actual: 1
        })
    ));
    assert!(matches!(
        fixture.checked(vec![
            HirFunctionIdentity::source(source.clone()),
            HirFunctionIdentity::source(source),
        ]),
        Err(HirFunctionIdentityError::DuplicatePlainIdentity { function: 1 })
    ));

    let identities = fixture
        .checked(vec![
            HirFunctionIdentity::source(
                HirSourceFunctionIdentity::from_declaration(source_key("first", 0)).unwrap(),
            ),
            HirFunctionIdentity::source(
                HirSourceFunctionIdentity::from_declaration(source_key("second", 0)).unwrap(),
            ),
        ])
        .unwrap();
    assert!(identities[first].source_identity().is_some());
    assert!(identities[second].source_identity().is_some());
}

#[test]
fn repeated_default_instantiations_must_preserve_the_same_lexical_site() {
    let mut fixture = Fixture::empty();
    let root = fixture.functions.alloc(function("root"));
    let generated = fixture.functions.alloc(function("lambda"));
    let definition_path = path(0);
    let lambda = Lambda {
        definition_root: crate::LexicalDefinitionRoot::Function(root),
        definition_path: definition_path.clone(),
        function: generated,
        function_type: crate::FunctionTypeId::from_raw(0_u32.into()),
        owner_type_param_count: 0,
        body_type_arguments: CallableBodyTypeArguments::Lexical,
        captures: Vec::new(),
        span: Span { start: 0, end: 0 },
    };
    fixture.lambdas.alloc(lambda.clone());
    fixture.lambdas.alloc(lambda);
    let root_identity = HirSourceFunctionIdentity::from_declaration(source_key("root", 0)).unwrap();
    let generated_identity = HirFunctionIdentity::lexical_generated(
        root_identity.lexical_parent(),
        LexicalCallableRole::LambdaBody,
        definition_path,
    )
    .unwrap();
    fixture
        .checked(vec![
            HirFunctionIdentity::source(root_identity),
            generated_identity,
        ])
        .unwrap();

    fixture.lambdas[crate::LambdaId::from_raw(1_u32.into())].definition_path = path(1);
    assert!(matches!(
        fixture.checked(vec![
            HirFunctionIdentity::source(
                HirSourceFunctionIdentity::from_declaration(source_key("root", 0)).unwrap(),
            ),
            HirFunctionIdentity::lexical_generated(
                HirSourceFunctionIdentity::from_declaration(source_key("root", 0))
                    .unwrap()
                    .lexical_parent(),
                LexicalCallableRole::LambdaBody,
                path(0),
            )
            .unwrap(),
        ]),
        Err(HirFunctionIdentityError::ConflictingClaim {
            function: 1,
            first: FunctionIdentityRelation::Lambda,
            second: FunctionIdentityRelation::Lambda,
        })
    ));
}
