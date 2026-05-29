# TODO-4：P5-P6 通用常量化折叠器与 Platform 收敛

> 索引：[`TODO.md`](./TODO.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 覆盖阶段：P5-P6
> 包目标：用类型特征驱动的 `is_immutable` 谓词 + `try_emit_immortal` 折叠器替换三个手写 immortal 路径，让 String literal 零分配并对 String 开内容池；Platform 作为消费者自动落入，删除一切专用 codegen。

## P5：通用谓词、折叠器与 String immortal

### [TODO] P5-T01：实现 `is_immutable(T)` 谓词

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The constantization predicate `is_immutable(T)`”
- 目标：
  - 实现一个结构、递归、可 memo 的不可变性谓词，作为常量化的通用门。
- 必须修改的文件/位置：
  - codegen 侧谓词落点（新增，靠近 `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/`）
  - 类型/字段元数据查询：`crates/scoopc_hir/src/hir/mod.rs:205`（`FieldDecl.mutable`）、`TypeStore`
- 必须实现的内容：
  1. `is_immutable(T)`：
     - 带 `@InteriorMutable` → false；
     - 值标量（Int/Bool/Float64/32/Char/Unit）→ true；
     - 值 struct / tuple → 所有字段类型 `is_immutable`（struct 字段必 `val`，无需查可变性）；
     - ref class → 所有字段 `val` 且 所有字段类型 `is_immutable`。
  2. 递归进字段类型并 memo，处理循环引用（自引用类型不可变性需有终止策略）。
- 必须遵从的约束：
  - 决策由类型特征驱动，不得名字匹配或类型白名单。
  - `var` 字段检查只在 ref class 分支有意义；`@InteriorMutable` 是值类型层唯一守卫。
- 验证：
  1. 单元：String 与合成全-val class 为 true；`RefCell` / `AtomicInt`（`var`）与 `__AtomicInt`（`@InteriorMutable`）为 false；含 `var x: RefCell` 的类型为 false。
  2. `cargo test --all --all-targets`
- 完成条件：
  - 谓词正确区分可常量化与不可常量化类型。
- 依赖：P3-T02R、P4-T02R
- 完成记录：
  - （待执行）

### [TODO] P5-T01R：Review `is_immutable` 谓词

- 参考：
  - P5-T01 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The constantization predicate”
- 目标：
  - 复核谓词的递归正确性、循环终止、`@InteriorMutable` 与 `var` 分工。
- 必须检查的文件/位置：
  - P5-T01 谓词实现与单元测试
- 必须实现的内容：
  1. 确认值 struct 分支不依赖 `var` 检查（struct 必 val），ref class 分支正确处理 `var`。
  2. 确认自引用/循环类型不会无限递归。
  3. 确认 `@InteriorMutable` 在值类型层确实是唯一否决路径。
- 必须遵从的约束：
  - 若谓词退回名字匹配或漏循环终止，必须修正后才进入 P5-T02。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - 谓词可靠。
- 依赖：P5-T01
- 完成记录：
  - （待执行）

### [TODO] P5-T02：实现 `try_emit_immortal` 折叠器并路由 String literal

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The generic immortal folder”“Emission shapes”、Phasing 4
- 目标：
  - 用一个 content-hash 缓存的递归折叠器替换 String literal 的 per-use 分配，并为聚合提供提升门。
- 必须修改的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`（`codegen_string_literal_from_bytes`）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs`（`codegen_mir_const` 的 String/SynthString 分支）
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs`（`codegen_mir_rvalue` 的 `StructLit`/`MakeTuple` arm）
  - `crates/scoopc_mir/src/mir/transport.rs`（`AggregateTransportMetadata`/`MirBoxingIntent` 查询）
- 必须实现的内容：
  1. `try_emit_immortal(value) -> Option<GlobalValue>`（content-hash 缓存）：
     - 标量 `ConstValue::*` → LLVM 标量常量；
     - `ConstValue::String/SynthString` 与 `TypeMetadataLiteral::TypeNameString` → 带 header + `SCOOP_GC_FLAG_IMMORTAL` + `SCOOP_GC_MARK_IMMORTAL`、`next=null` 的 immortal `ScoopString` 全局（`set_constant`+`set_unnamed_addr`）；
     - `Rvalue::StructLit/MakeTuple` 过提升门则发射常量聚合全局（值类型层无 header；ref 类型层带 header），否则 `None`。
  2. 提升门：① 字段全 `Operand::Const`；② 每字段 `transport.boxing.is_none()`；③ `transport.kind` 为 `Tuple`/`Struct`；④ ref 类型聚合再加 `is_immutable(aggregate_ty)`。
  3. `codegen_string_literal_from_bytes` 与 `const_pat.rs` String/SynthString 分支改走折叠器；`terminator.rs` 的 `StructLit`/`MakeTuple` arm 先试折叠器再回退现有动态路径。
- 必须遵从的约束：
  - 遇到非平凡 transport（boxing/value-erasure）或非 `Const` 字段必须安全回退 `None`。
  - 本版不追 `Local`→定义它的 StructLit（嵌套聚合回退）；不含 `EnumVariant`。
  - 决策由类型特征驱动，不逐类型特判。
- 验证：
  1. 单元：`codegen_string_literal_from_bytes("hello")` 零 `scoop_alloc_typed`；含 boxing 的聚合回退动态路径。
  2. 集成（P0-T03 度量）：literal 在 10M 循环里首个 cycle 后 `bytes_allocated` 零增长（除 print 自身）。
  3. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - String literal / `__type_name` 零堆分配，聚合按门提升或安全回退。
- 依赖：P5-T01R
- 完成记录：
  - （待执行）

### [TODO] P5-T02R：Review 折叠器与 String immortal

- 参考：
  - P5-T02 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “The generic immortal folder”
- 目标：
  - 复核提升门完整性、回退安全性、immortal header 字段正确性。
- 必须检查的文件/位置：
  - P5-T02 折叠器与各 arm 改动
- 必须实现的内容：
  1. 确认 boxing/非 Const/EnumVariant/嵌套聚合都安全回退。
  2. 确认 immortal `ScoopString` 的 header 字段（`next=null`、flag、mark sentinel、type_desc、size）正确。
  3. 确认 String 零分配在 IR 与运行期度量上都成立。
- 必须遵从的约束：
  - 若提升门有不安全提升（漏 boxing 判断等），必须修正后才进入 P5-T03。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - 折叠器正确、String immortal 安全。
- 依赖：P5-T02
- 完成记录：
  - （待执行）

### [TODO] P5-T03：String 内容池 dedup 与其它 ref 类型 per-site

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Deduplication (String only, for now)”
- 目标：
  - 把 dedup 这个身份敏感行为限定到 String（内容池），其它可常量化 ref 类型 per-site 一份。
- 必须修改的文件/位置：
  - P5-T02 折叠器的 wrapper 全局缓存键逻辑
- 必须实现的内容：
  1. String：wrapper 与 byte 数组都按 content-hash 键（`__scoop_str_lit_<hash>`），跨 site 合并成一份。
  2. 其它可常量化 ref 类型：每个 literal site 发一份全局，不跨站合并，保持 per-site 身份。
  3. 在完成记录说明 dedup 边界与理由（Scoop 无身份运算符 ⇒ String 值语义 ⇒ 合并不可观测）。
- 必须遵从的约束：
  - dedup 仅对 String；不得对其它类型跨站合并（除非未来评估身份哈希通道后另开任务）。
- 验证：
  1. 单元：同函数两个 `"hello"` 引用同一全局；两次 `__type_name(T)` 指针相等。
  2. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - String 内容池生效，其它 ref 类型 per-site。
- 依赖：P5-T02R
- 完成记录：
  - （待执行）

### [TODO] P5-T03R：Review dedup 策略

- 参考：
  - P5-T03 完成记录
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Deduplication”
- 目标：
  - 复核 dedup 仅限 String，其它类型未被误合并。
- 必须检查的文件/位置：
  - P5-T03 缓存键逻辑
- 必须实现的内容：
  1. 确认非 String 可常量化 ref 类型确实 per-site，不跨站合并。
  2. 确认 String 跨站合并、指针相等。
- 必须遵从的约束：
  - 若非 String 类型被误合并，必须修正后才进入 P6-T01。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - dedup 边界正确。
- 依赖：P5-T03
- 完成记录：
  - （待执行）

## P6：Platform 折叠与 TypeMetadataLiteral 审计

### [TODO] P6-T01：Platform lower 成 StructLit 并删除专用 codegen

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Consumers (recast) — Platform”、Phasing 5
- 目标：
  - 让 `Platform` 作为通用折叠器的消费者自动落入，删除一切 Platform 专用 codegen。
- 必须修改的文件/位置：
  - HIR→MIR `TypedIntrinsicKind::Platform` lowering 落点
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:203-292`（`codegen_platform_literal`）
  - `sysroot/lib/scoop.core/src/core.scoop`（`Platform` struct，参考）
- 必须实现的内容：
  1. 在 HIR→MIR 把 `TypedIntrinsicKind::Platform` lower 成 `Rvalue::StructLit`，5 字段为 `Operand::Const(ConstValue::SynthString(...))`，transport kind=`Struct`、各字段 `boxing: None`。
  2. 删除 `codegen_platform_literal` 及任何 `get_or_create_immortal_platform_global` 专用 helper。
  3. 确认 Platform（值类型 struct）走折叠器值类型层（无 header），5 字段引用 ref 层 immortal `ScoopString`。
- 必须遵从的约束：
  - 不得保留 Platform 专用常量化路径；它必须与普通值 struct 同路。
  - Platform 聚合本身无 GC header。
- 验证：
  1. 单元：`Platform` 访问零 `scoop_alloc_typed`。
  2. 集成：`Platform.os` 读 10M 次不触发 GC。
  3. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - Platform 由通用机制处理，无专用代码。
- 依赖：P5-T03R
- 完成记录：
  - （待执行）

### [TODO] P6-T01R：Review Platform 折叠

- 参考：
  - P6-T01 完成记录
- 目标：
  - 复核 Platform 专用 codegen 已删尽、走值类型层、字段引用 immortal String。
- 必须检查的文件/位置：
  - P6-T01 的 lowering 与删除改动
- 必须实现的内容：
  1. 反向 grep 确认无 Platform 专用常量化残留。
  2. 确认 Platform 聚合无 GC header、字段指向 ref 层 immortal。
  3. 确认零 `scoop_alloc_typed`。
- 必须遵从的约束：
  - 若残留专用路径或聚合误带 header，必须修正后才进入 P6-T02。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - Platform 收敛正确。
- 依赖：P6-T01
- 完成记录：
  - （待执行）

### [TODO] P6-T02：`TypeMetadataLiteral` 审计与指针相等断言

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md) “Consumers (recast) — TypeMetadataLiteral”、Phasing 6
- 目标：
  - 审计 `TypeMetadataLiteral` 消费者不 mutate，并断言 dedup 后两次 `__type_name(T)` 指针相等。
- 必须修改的文件/位置：
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201`
  - MIR `TypeMetadataLiteral` 消费者
- 必须实现的内容：
  1. 审计所有 `TypeMetadataLiteral` 消费者，确认无人 mutate 其结果。
  2. 新增断言/测试：两次 `__type_name(T)` 读返回指针相等的 `ScoopString`。
- 必须遵从的约束：
  - 若发现消费者 mutate，必须先修正其语义或退出该消费者的 immortal 路径。
- 验证：
  1. `cargo test --all --all-targets`、`python3 tools/run_fixtures.py`
- 完成条件：
  - TypeMetadataLiteral 不可变性确认，指针相等成立。
- 依赖：P6-T01R
- 完成记录：
  - （待执行）

### [TODO] P6-T02R：Review TypeMetadata 审计

- 参考：
  - P6-T02 完成记录
- 目标：
  - 复核审计覆盖面与指针相等断言。
- 必须检查的文件/位置：
  - P6-T02 审计范围与新增断言
- 必须实现的内容：
  1. 确认审计覆盖所有消费者。
  2. 确认指针相等断言真实成立。
- 必须遵从的约束：
  - 若审计有遗漏，必须补全后才进入 TODO-5。
- 验证：
  1. `cargo test --all --all-targets`
- 完成条件：
  - immortal codegen 线收口（P5-P6 完成）。
- 依赖：P6-T02
- 完成记录：
  - （待执行）
