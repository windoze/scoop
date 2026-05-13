## 当前执行计划

1. 已确认第一个未完成任务为 `P7-T01D`；最近一次提交已把它作为 `P7-T01R` 的前置 blocker 写入 `TODO.md`，本轮直接处理该任务。
2. 阅读 `PLAN.md`、`STABLE_ID.md` 与 `P7-T01D` 涉及的 LLVM codegen/RTTI/type-driven stable-id helper 代码，找出仍固定使用 `NoTypeParamResolver` 的 active production 调用链。
3. 设计并接入 LLVM codegen 层的 authoritative type-param resolver，使 generic-bearing `TypeId` 在 codegen 阶段生成 canonical key / RTTI type-id / private type-global 名称时能拿到真实语义键。
4. 按同根问题成组修复 `mir_value_box_object_type`、type-desc/global、capture-box、itable owner 及相关 sibling case，避免只修单个 fixture。
5. 补齐或更新回归测试，至少覆盖 TODO 指定的 run-pass fixture 与一个直接锁定 generic-bearing type-driven naming 的定向测试。
6. 运行任务要求的验证、`cargo fmt`、`cargo clippy -p scoopc --all-targets -- -D warnings`。
7. 更新 `TODO.md` 完成记录与本文件，然后创建一次 git 提交并停止。

## 说明

- 这里记录的是可公开的执行计划与关键决策，不包含私有推理细节。
- 如果执行中出现阻塞，会先确认是否属于当前任务的真实前置依赖，再决定是否更新 `TODO.md`/`PLAN.md`。

## 当前进展

- 已完成：读取 `TODO.md` 并定位 `P7-T01D`；确认最新提交 `542d2290` 直接把该 blocker 插入为 `P7-T01R` 的前置任务。
- 已完成：复现 blocker，确认 `cargo run -p scoop -- run tests/fixtures/run-pass/class_init_raise_cleanup_init_block_gc_basic.scoop` 仍失败于 `MIR value box LLVM type` 的 stable canonical key 构造，错误为 `missing stable type parameter key for 'B'`。
- 已完成：在 HIR lowering 侧新增“声明级 type/effect 参数 -> stable owner/index key”索引，并把它接入 `LoweredHir` / `CompilationUnitCodegenCx`。
- 已完成：LLVM codegen 的 `canonical_type_key_text_for_codegen`、`stable_rtti_type_id_for_codegen`、MIR value-box interface RTTI 路径、effect-lowered task transport / effect transport box naming、以及 effect-lowered stable naming helper 已切到 authoritative resolver，不再固定使用 `NoTypeParamResolver`。
- 已完成：新增 `llvm` 定向回归 `generic_class_init_raise_cleanup_uses_stable_type_driven_box_naming`，直接锁定 generic class init cleanup 下的 type-driven private box naming。
- 已完成：定向与全量验证均已通过；下一步只剩更新 `TODO.md` 完成记录、检查 worktree，并创建提交。
