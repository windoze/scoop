use scoop_ast::Span;
use scoop_hir as hir;

use crate::persistent_nominals::NominalIdentityInput;
use crate::{IntrinsicDeclarationPolicy, Lowerer, Owner, SourceKind, SourceProvider, Type};
use scoop_identity::SourceNominalKind;

pub(super) fn lowerer() -> Lowerer {
    let files = [crate::tests::file(Vec::new())];
    let mut lowerer = Lowerer::new().with_intrinsic_sources(
        vec![SourceProvider {
            provider: hir::IntrinsicProviderId::from_raw(1),
            kind: SourceKind::CurrentUnit,
            identity: crate::tests::test_source_identity("src/solver.scoop"),
            name: "solver.scoop".to_owned(),
            source: String::new(),
        }],
        IntrinsicDeclarationPolicy::CoreOnly,
    );
    let cone = lowerer.current_cone();
    lowerer
        .top_level_namespaces
        .initialize_sources([cone], cone, &files);
    lowerer
}

pub(super) fn add_interface(
    lowerer: &mut Lowerer,
    name: &str,
    parents: Vec<hir::TypeId>,
) -> hir::TypeId {
    let interface = hir::InterfaceId::from_raw(
        u32::try_from(lowerer.interfaces.len())
            .expect("test interface count fits u32")
            .into(),
    );
    let application = hir::InterfaceApplicationId::from_raw(
        u32::try_from(lowerer.interface_applications.len())
            .unwrap()
            .into(),
    );
    let allocated = lowerer.interfaces.alloc(hir::InterfaceDecl {
        owner: None,
        name: name.to_string(),
        access: hir::NominalAccess::public(),
        definition: hir::InterfaceDefinition {
            self_application: application,
            type_params: Vec::new(),
            parents,
        },
        gc_free_pointee_requirements: Vec::new(),
        methods: Vec::new(),
        private_methods: Vec::new(),
        properties: Vec::new(),
        span: Span::new(0, 0),
    });
    assert_eq!(allocated, interface);
    lowerer.interface_files.insert(interface, 0);
    let identity = lowerer
        .prepare_nominal_identity(NominalIdentityInput {
            name,
            parent: None,
            access: hir::DeclaredVisibility::Public,
            type_parameter_count: 0,
            kind: SourceNominalKind::Interface,
            file: 0,
            span: Span::new(0, 0),
        })
        .unwrap();
    lowerer.register_nominal_identity(Owner::Interface(interface), identity);
    lowerer.establish_nominal_identities().unwrap();
    assert_eq!(
        lowerer.interface_application_id(interface, Vec::new()),
        application
    );
    lowerer.interface_applications[application].canonical_type
}

pub(super) fn add_generic_struct(
    lowerer: &mut Lowerer,
    name: &str,
    parameter: hir::TypeParamDecl,
    representation: hir::StructRepresentation,
) -> hir::StructId {
    let structure = hir::StructId::from_raw(
        u32::try_from(lowerer.structs.len())
            .expect("test struct count fits u32")
            .into(),
    );
    let self_application = hir::StructApplicationId::from_raw(
        u32::try_from(lowerer.struct_applications.len())
            .expect("test struct application count fits u32")
            .into(),
    );
    let parameter_ty = lowerer.intern_type(Type::Param(parameter.id));
    let allocated = lowerer.structs.alloc(hir::StructDecl {
        owner: None,
        name: name.to_string(),
        access: hir::NominalAccess::public(),
        gc_free_pointee_requirements: Vec::new(),
        attributes: hir::StructAttributes::default(),
        constructors: Vec::new(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),
        definition: hir::StructDefinition {
            self_application,
            type_params: vec![parameter],
            representation,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
        },
    });
    assert_eq!(allocated, structure);
    lowerer.struct_files.insert(structure, 0);
    let identity = lowerer
        .prepare_nominal_identity(NominalIdentityInput {
            name,
            parent: None,
            access: hir::DeclaredVisibility::Public,
            type_parameter_count: 1,
            kind: SourceNominalKind::Struct,
            file: 0,
            span: Span::new(0, 0),
        })
        .unwrap();
    lowerer.register_nominal_identity(Owner::Struct(structure), identity);
    lowerer.establish_nominal_identities().unwrap();
    let actual_application = lowerer.struct_application_id(structure, vec![parameter_ty]);
    assert_eq!(actual_application, self_application);
    structure
}
