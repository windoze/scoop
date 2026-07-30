//! 烟雾测试：直接构造 `LirProgram` 验证 codegen 函数体 lowering。
//!
//! 这些测试不依赖 sysroot/HIR/MIR，驱动 codegen 本体的开发。

#![cfg(feature = "llvm")]

use scoop2_codegen_llvm::{EmitOptions, emit_program};
use scoop2_hir::ty::TypeId;
use scoop2_lir::{
    LirBlock, LirBody, LirCallable, LirConstValue, LirLocalDecl, LirOperand, LirProgram, LirRvalue,
    LirStmt, LirStmtKind, LirTerminator, ParamAbi, ScalarKind, TypeLayout, TypeLayoutKind,
    TypeLayoutTable,
};

/// 构造一个最小 LirProgram：一个函数 `f`，无参，返回 Int 常量 42。
fn program_return_const() -> LirProgram {
    let mut prog = LirProgram::new();
    // 注册 TypeId(2) = Int{64} 布局。
    prog.type_layouts.insert(
        TypeId(2),
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: false,
                },
            },
        },
    );
    // main 返回 ty#2，body = { local0 = Const(42); return local0 }。
    let body = LirBody {
        locals: vec![LirLocalDecl {
            id: 0,
            name: None,
            ty: TypeId(2),
            mutable: false,
            gc_traceable: false,
        }],
        blocks: vec![LirBlock {
            id: 0,
            stmts: vec![LirStmt {
                span: scoop2_base::Span::default(),
                kind: LirStmtKind::Assign {
                    target: 0,
                    value: LirRvalue::Const(LirConstValue::Int(42, None)),
                },
            }],
            terminator: LirTerminator::Return {
                value: Some(LirOperand::Local(0)),
            },
        }],
        start_block: 0,
    };
    prog.callables.push(LirCallable {
        fqn: "f".to_string(),
        symbol_name: "f".to_string(),
        abi: scoop2_lir::LirCallableAbi::Plain,
        params: vec![],
        return_ty: TypeId(2),
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    });
    prog
}

#[test]
fn return_const_lowers_to_function() {
    let prog = program_return_const();
    let emitted = emit_program(&prog, &EmitOptions::default()).expect("emit 应成功");
    let ir = &emitted.ir_text;
    // 应定义函数 f。
    assert!(ir.contains("define"), "IR 应含函数定义：\n{ir}");
    assert!(ir.contains("f("), "IR 应含 f 函数：\n{ir}");
    // 应包含 ret i64（返回 local，load 后 ret）。
    assert!(ir.contains("ret i64"), "IR 应含 ret i64：\n{ir}");
}

#[test]
fn empty_program_produces_runtime_decls() {
    let prog = LirProgram::new();
    let emitted = emit_program(&prog, &EmitOptions::default()).expect("emit 应成功");
    assert!(
        emitted.ir_text.contains("scoop_runtime_init"),
        "IR 应包含 scoop_runtime_init 声明"
    );
}

/// 确保空闲时 TypeLayoutTable 能构造（避免 unused warning 用例）。
#[test]
fn type_layout_table_default() {
    let _ = TypeLayoutTable::new();
}

use scoop2_lir::{LirCall, LirCallKind};

/// 构造一个程序：`f(a: Int, b: Int): Int = a + b`（通过 intrinsic `scoop.core.Int.plus`）。
fn program_arithmetic() -> LirProgram {
    use scoop2_lir::LirParam;
    let mut prog = LirProgram::new();
    prog.type_layouts.insert(
        TypeId(2),
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: false,
                },
            },
        },
    );
    let body = LirBody {
        locals: vec![
            LirLocalDecl {
                id: 0,
                name: Some("a".into()),
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
            LirLocalDecl {
                id: 1,
                name: Some("b".into()),
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
            LirLocalDecl {
                id: 2,
                name: None,
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
        ],
        blocks: vec![LirBlock {
            id: 0,
            stmts: vec![LirStmt {
                span: scoop2_base::Span::default(),
                kind: LirStmtKind::Assign {
                    target: 2,
                    value: LirRvalue::Call(LirCall {
                        kind: LirCallKind::Direct {
                            callee_symbol: "scoop.core.Int.plus".to_string(),
                            callee_fqn: "scoop.core.Int.plus".to_string(),
                            stable_instance_key: None,
                            intrinsic_name: Some("int_plus".to_string()),
                        },
                        args: vec![LirOperand::Local(0), LirOperand::Local(1)],
                        result_ty: TypeId(2),
                    }),
                },
            }],
            terminator: LirTerminator::Return {
                value: Some(LirOperand::Local(2)),
            },
        }],
        start_block: 0,
    };
    prog.callables.push(LirCallable {
        fqn: "f".to_string(),
        symbol_name: "f".to_string(),
        abi: scoop2_lir::LirCallableAbi::Plain,
        params: vec![
            LirParam {
                name: "a".into(),
                ty: TypeId(2),
                abi: ParamAbi::Direct,
                local_id: 0,
            },
            LirParam {
                name: "b".into(),
                ty: TypeId(2),
                abi: ParamAbi::Direct,
                local_id: 1,
            },
        ],
        return_ty: TypeId(2),
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    });
    prog
}

#[test]
fn arithmetic_intrinsic_lowers_to_add() {
    let prog = program_arithmetic();
    let emitted =
        emit_program(&prog, &EmitOptions::default()).unwrap_or_else(|e| panic!("emit: {e:?}"));
    let ir = &emitted.ir_text;
    // intrinsic int_plus 应 lower 为 add 指令，而非对 scoop.core.Int.plus 的调用。
    assert!(
        ir.contains("add i64"),
        "IR 应含 add i64（intrinsic 内联）：\n{ir}"
    );
    assert!(
        !ir.contains("@scoop.core.Int.plus"),
        "IR 不应含对 intrinsic 符号的调用"
    );
}

/// 构造一个程序：`f(x: Int): Int = if x != 0 { 1 } else { 2 }`（CondBr）。
fn program_condbr() -> LirProgram {
    use scoop2_lir::LirParam;
    let mut prog = LirProgram::new();
    prog.type_layouts.insert(
        TypeId(2),
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: false,
                },
            },
        },
    );
    // Bool = TypeId(3) (i8)
    prog.type_layouts.insert(
        TypeId(3),
        TypeLayout {
            size: 1,
            align: 1,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Bool,
            },
        },
    );
    let body = LirBody {
        locals: vec![
            LirLocalDecl {
                id: 0,
                name: Some("x".into()),
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
            LirLocalDecl {
                id: 1,
                name: Some("c".into()),
                ty: TypeId(3),
                mutable: false,
                gc_traceable: false,
            },
            LirLocalDecl {
                id: 2,
                name: None,
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
        ],
        blocks: vec![
            LirBlock {
                id: 0,
                stmts: vec![LirStmt {
                    span: scoop2_base::Span::default(),
                    kind: LirStmtKind::Assign {
                        target: 1,
                        // 简化：c = IntEq(x, 0)；这里直接用 Use(Bool const) 替代以测 CondBr。
                        value: LirRvalue::Const(LirConstValue::Bool(true)),
                    },
                }],
                terminator: LirTerminator::CondBr {
                    cond: LirOperand::Local(1),
                    then_target: 1,
                    else_target: 2,
                },
            },
            LirBlock {
                id: 1,
                stmts: vec![LirStmt {
                    span: scoop2_base::Span::default(),
                    kind: LirStmtKind::Assign {
                        target: 2,
                        value: LirRvalue::Const(LirConstValue::Int(1, None)),
                    },
                }],
                terminator: LirTerminator::Goto { target: 3 },
            },
            LirBlock {
                id: 2,
                stmts: vec![LirStmt {
                    span: scoop2_base::Span::default(),
                    kind: LirStmtKind::Assign {
                        target: 2,
                        value: LirRvalue::Const(LirConstValue::Int(2, None)),
                    },
                }],
                terminator: LirTerminator::Goto { target: 3 },
            },
            LirBlock {
                id: 3,
                stmts: vec![],
                terminator: LirTerminator::Return {
                    value: Some(LirOperand::Local(2)),
                },
            },
        ],
        start_block: 0,
    };
    prog.callables.push(LirCallable {
        fqn: "f".to_string(),
        symbol_name: "f".to_string(),
        abi: scoop2_lir::LirCallableAbi::Plain,
        params: vec![LirParam {
            name: "x".into(),
            ty: TypeId(2),
            abi: ParamAbi::Direct,
            local_id: 0,
        }],
        return_ty: TypeId(2),
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    });
    prog
}

#[test]
fn condbr_lowers_to_conditional_branch() {
    let prog = program_condbr();
    let emitted = emit_program(&prog, &EmitOptions::default())
        .unwrap_or_else(|e| panic!("emit failed: {e:?}"));
    let ir = &emitted.ir_text;
    assert!(ir.contains("br i1"), "IR 应含 br i1（条件分支）：\n{ir}");
    assert!(ir.contains("ret i64"), "IR 应含 ret i64");
}

/// 构造：`f(): Int { val t = (10, 20); return t.1 }`（MakeTuple + TupleIndex）。
fn program_tuple() -> LirProgram {
    let mut prog = LirProgram::new();
    prog.type_layouts.insert(
        TypeId(2),
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Scalar {
                scalar_kind: ScalarKind::Int {
                    bits: 64,
                    unsigned: false,
                },
            },
        },
    );
    // tuple (Int, Int) = TypeId(5)
    prog.type_layouts.insert(
        TypeId(5),
        TypeLayout {
            size: 16,
            align: 8,
            kind: TypeLayoutKind::Tuple {
                elements: vec![
                    scoop2_lir::FieldLayout {
                        offset: 0,
                        size: 8,
                        ty: TypeId(2),
                    },
                    scoop2_lir::FieldLayout {
                        offset: 8,
                        size: 8,
                        ty: TypeId(2),
                    },
                ],
            },
        },
    );
    let body = LirBody {
        locals: vec![
            LirLocalDecl {
                id: 0,
                name: Some("t".into()),
                ty: TypeId(5),
                mutable: false,
                gc_traceable: false,
            },
            LirLocalDecl {
                id: 1,
                name: None,
                ty: TypeId(2),
                mutable: false,
                gc_traceable: false,
            },
        ],
        blocks: vec![LirBlock {
            id: 0,
            stmts: vec![
                LirStmt {
                    span: scoop2_base::Span::default(),
                    kind: LirStmtKind::Assign {
                        target: 0,
                        value: LirRvalue::MakeTuple {
                            elements: vec![
                                LirOperand::Const(LirConstValue::Int(10, None)),
                                LirOperand::Const(LirConstValue::Int(20, None)),
                            ],
                            ty: TypeId(5),
                        },
                    },
                },
                LirStmt {
                    span: scoop2_base::Span::default(),
                    kind: LirStmtKind::Assign {
                        target: 1,
                        value: LirRvalue::TupleIndex {
                            receiver_local: LirOperand::Local(0),
                            index: 1,
                            element_ty: TypeId(2),
                        },
                    },
                },
            ],
            terminator: LirTerminator::Return {
                value: Some(LirOperand::Local(1)),
            },
        }],
        start_block: 0,
    };
    prog.callables.push(LirCallable {
        fqn: "f".to_string(),
        symbol_name: "f".to_string(),
        abi: scoop2_lir::LirCallableAbi::Plain,
        params: vec![],
        return_ty: TypeId(2),
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    });
    prog
}

#[test]
fn tuple_make_and_index_lowers_correctly() {
    let prog = program_tuple();
    let emitted =
        emit_program(&prog, &EmitOptions::default()).unwrap_or_else(|e| panic!("emit: {e:?}"));
    let ir = &emitted.ir_text;
    // MakeTuple 的元素是常量时，LLVM 会常量折叠为 struct 常量（无显式 insertvalue）；
    // 故只断言 tuple 类型 + extractvalue。
    assert!(
        ir.contains("{ i64, i64 }"),
        "IR 应含 tuple 类型 {{ i64, i64 }}：\n{ir}"
    );
    assert!(
        ir.contains("extractvalue"),
        "IR 应含 extractvalue（TupleIndex）：\n{ir}"
    );
}

#[test]
fn object_output_produces_valid_object_file() {
    use scoop2_codegen_llvm::emit_object_to_file;
    let prog = program_return_const();
    // 需要 user main：program_return_const 的 fqn 是 "f"，不是 main。改为构造一个 main。
    let mut prog = prog;
    prog.callables[0].fqn = "main".to_string();
    let tmp = std::env::temp_dir().join("scoop_codegen_smoke.o");
    let ir = emit_object_to_file(&prog, &tmp, &EmitOptions::default())
        .unwrap_or_else(|e| panic!("object 输出失败：{e:?}"));
    assert!(tmp.exists(), "object 文件应存在");
    assert!(
        tmp.metadata().map(|m| m.len() > 0).unwrap_or(false),
        "object 文件应非空"
    );
    // 验证 object 文件是 ELF/Mach-O（用 file 命令或 magic）。
    let magic = std::fs::read(&tmp).unwrap_or_default();
    let is_obj = magic.len() >= 4
        && (
            magic.starts_with(&[0x7f, b'E', b'L', b'F']) // ELF
        || (magic.len() >= 4 && magic[0] == 0xfe && magic[1] == 0xed && magic[2] == 0xfa) // Mach-O
        || magic.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
            // Mach-O 64 LE
        );
    assert!(is_obj, "应为合法 object 文件（ELF/Mach-O magic）");
    let _ = ir;
    let _ = std::fs::remove_file(&tmp);
}

/// 程序：`f(s: String): String { return s }`（GC local → root frame push/pop）。
fn program_gc_local() -> LirProgram {
    use scoop2_lir::LirParam;
    let mut prog = LirProgram::new();
    // String = TypeId(16)，GC 引用。
    prog.type_layouts.insert(
        TypeId(16),
        TypeLayout {
            size: 8,
            align: 8,
            kind: TypeLayoutKind::Reference {
                gc_traceable: true,
                ref_kind: scoop2_lir::RefKind::String,
            },
        },
    );
    let body = LirBody {
        locals: vec![LirLocalDecl {
            id: 0,
            name: Some("s".into()),
            ty: TypeId(16),
            mutable: false,
            gc_traceable: true,
        }],
        blocks: vec![LirBlock {
            id: 0,
            stmts: vec![],
            terminator: LirTerminator::Return {
                value: Some(LirOperand::Local(0)),
            },
        }],
        start_block: 0,
    };
    prog.callables.push(LirCallable {
        fqn: "f".to_string(),
        symbol_name: "f".to_string(),
        abi: scoop2_lir::LirCallableAbi::Plain,
        params: vec![LirParam {
            name: "s".into(),
            ty: TypeId(16),
            abi: ParamAbi::Direct,
            local_id: 0,
        }],
        return_ty: TypeId(16),
        return_abi: ParamAbi::Direct,
        body: Some(body),
        gc_info: None,
        frame_schema: None,
        step_layout: None,
        state_dispatch: None,
        continuation_layout: None,
        effect_info: None,
    });
    prog
}

#[test]
fn gc_local_emits_root_frame_push_pop() {
    let prog = program_gc_local();
    let emitted = emit_program(&prog, &EmitOptions::default()).expect("emit");
    let ir = &emitted.ir_text;
    // 应含 root frame alloca。
    assert!(
        ir.contains("root_frame"),
        "IR 应含 root_frame alloca：\n{ir}"
    );
    // 应引用 TLS 全局 __scoop_explicit_root_frame_top（load + store）。
    assert!(
        ir.contains("__scoop_explicit_root_frame_top"),
        "IR 应引用 TLS root frame top：\n{ir}"
    );
    // 应有 desc 全局 + offsets 全局。
    assert!(
        ir.contains("__scoop_root_desc_f"),
        "IR 应含 root frame desc 全局：\n{ir}"
    );
}

#[test]
fn gc_root_frame_object_output() {
    use scoop2_codegen_llvm::emit_object_to_file;
    let prog = program_gc_local();
    let tmp = std::env::temp_dir().join("scoop_codegen_gc.o");
    let _ = emit_object_to_file(&prog, &tmp, &EmitOptions::default())
        .unwrap_or_else(|e| panic!("GC object 输出失败：{e:?}"));
    assert!(tmp.exists(), "GC object 文件应存在");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn gc_local_load_reads_from_frame_slot() {
    // GC local 的 load 必须从 frame slot 取（moving GC 权威源），而非直接读 alloca。
    let prog = program_gc_local();
    let emitted = emit_program(&prog, &EmitOptions::default()).expect("emit");
    let ir = &emitted.ir_text;
    // 应有从 frame slot 的 load（命名为 ldf*）+ int<->ptr 转换。
    assert!(
        ir.contains("ldf") || ir.contains("ptrtoint"),
        "GC local load 应从 frame slot 读并转换指针：\n{ir}"
    );
    // return 值应是转换后的 GC 指针（addrspace(1)），而非直接 alloca load。
    assert!(
        ir.contains("ret ptr addrspace(1)"),
        "GC 函数应返回 addrspace(1) 指针：\n{ir}"
    );
}
