use scoop_ast::Span;
use scoop_hir as hir;

use crate::{IntrinsicDeclarationPolicy, Lowerer, SourceKind, SourceProvider, Type};

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
    let application = lowerer.interface_application_id(interface, Vec::new());
    let allocated = lowerer.interfaces.alloc(hir::InterfaceDecl {
        owner: None,
        name: name.to_string(),
        access: hir::NominalAccess::public(),
        self_application: application,
        type_params: Vec::new(),
        gc_free_pointee_requirements: Vec::new(),
        parents,
        methods: Vec::new(),
        private_methods: Vec::new(),
        properties: Vec::new(),
        span: Span::new(0, 0),
    });
    assert_eq!(allocated, interface);
    lowerer.interface_files.insert(interface, 0);
    lowerer.establish_nominal_identities().unwrap();
    lowerer.interface_applications[application].canonical_type
}

pub(super) fn add_generic_struct(
    lowerer: &mut Lowerer,
    name: &str,
    parameter: hir::TypeParamDecl,
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
        self_application,
        type_params: vec![parameter],
        gc_free_pointee_requirements: Vec::new(),
        attributes: hir::StructAttributes::default(),
        representation: hir::StructRepresentation::Declared(Vec::new()),
        constructors: Vec::new(),
        interfaces: Vec::new(),
        interface_implementations: Vec::new(),
        methods: Vec::new(),
        properties: Vec::new(),
        derived_equality: None,
        span: Span::new(0, 0),
    });
    assert_eq!(allocated, structure);
    lowerer.struct_files.insert(structure, 0);
    lowerer.establish_nominal_identities().unwrap();
    let actual_application = lowerer.struct_application_id(structure, vec![parameter_ty]);
    assert_eq!(actual_application, self_application);
    structure
}
