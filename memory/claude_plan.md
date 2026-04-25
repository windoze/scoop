# 执行计划与进度记录

## 约束说明

- 按用户要求，先写本文件，再执行其余命令或代码。
- 这里记录的是精简后的执行计划、决策依据和进度日志，不包含逐字内部推理。
- 本轮目标：完成 `TODO.md` 中第一个未完成任务；若发现既有问题或前置阻塞，则先修复该问题，或把前置任务插入 `TODO.md`/`PLAN.md` 后停止。

## 初始计划

1. 检查最新一次提交信息，确认是否提到需要先修复的既有问题。
2. 检查当前工作树状态，避免误覆盖已有改动。
3. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并判断是否需要拆分。
4. 若任务过大：
   - 更新 `PLAN.md`，补充更细的子任务。
   - 更新 `TODO.md`，把原任务替换或扩展为子任务，并选择第一个子任务作为本轮执行目标。
5. 实现目标任务；若过程中发现任何既有 bug、回归、规范不匹配或实现边界缺失：
   - 先修复；
   - 若无法在本轮直接修复，则把它作为前置任务插入 `TODO.md`，更新 `PLAN.md` 说明阻塞关系，然后停止。
6. 运行必要验证，至少覆盖：
   - 与改动直接相关的测试；
   - 必要时运行更广泛的 `cargo test --all`；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`（如果当前仓库状态允许且与本轮改动相关）。
7. 更新文档与跟踪文件：
   - 在 `TODO.md` 标记完成或重排阻塞依赖；
   - 更新 `PLAN.md` 当前状态；
   - 视需要补充 `README.md` / 行内注释。
8. 检查 `git diff`，确认只包含应提交的内容。
9. 使用清晰提交信息提交本轮变更。
10. 停止，不继续做下一个任务。

## 进度日志

- 已创建本文件，准备开始仓库检查。
- 已检查最新提交、工作树、`TODO.md` 与 `PLAN.md`。
  - 最新提交 `0f2cc6c5af23ba9df31e58ffc29ede735b0c0009` 的提交说明未留下新的“必须先修复的既有问题”条目；
  - 当前工作树只有本文件处于修改状态；
  - `TODO.md` 中首个未完成任务为 `T5000b3d 拆出 enum_lowering.rs 与 object_init.rs lowering 模块`。
- 已初步勘察 `crates/scoopc/src/llvm/codegen/mod.rs`：
  - enum lowering 主要残留在 `codegen_unresolved_ident`、`codegen_enum_variant_ctor_call`、`build_enum_variant_value_from_field_values`、`coerce_enum_payload`、`build_enum_value`、`try_codegen_qualified_enum_unit_variant_value`；
  - object lowering 主要残留在 `lookup_object_property_by_fqn`、`codegen_object_property_access`、`ensure_object_init_function_defined`、`codegen_object_init_fun_body`、`declare_object_init_guard`、`declare_object_instance_global`、`allocate_object_singleton_instance`、`codegen_object_value_access`、`declare_object_property_global`。
- 当前判断：`T5000b3d` 范围清晰，暂不需要拆成更小的 TODO 子任务。

## 当前执行顺序

1. 核对上述 enum/object lowering 的调用面，确保对外只暴露最小接口。
2. 新建 `crates/scoopc/src/llvm/codegen/enum_lowering.rs`，迁出 enum ctor / payload / unit-variant lowering。
3. 新建 `crates/scoopc/src/llvm/codegen/object_init.rs`，迁出 object property / singleton / init lowering。
4. 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中收口模块声明与必要桥接。
5. 运行格式化与测试；若过程中暴露既有缺陷，先修复再继续。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，最后提交并停止。

## 执行进展补记

- 已新增 `crates/scoopc/src/llvm/codegen/enum_lowering.rs`：
  - 迁入 `codegen_unresolved_ident`、`codegen_enum_variant_ctor_call`、`build_enum_variant_value_from_field_values`、`coerce_enum_payload`、`build_enum_value`、`try_codegen_qualified_enum_unit_variant_value`；
  - 其中 `build_enum_value` 已按当前仓库现状补齐 `CgEnumRepr::ValueOnly` 分支，并对齐 `NicheStorage::U8` 的最新 lowering 逻辑，避免回退到旧实现。
- 已新增 `crates/scoopc/src/llvm/codegen/object_init.rs`：
  - 迁入 object property access、singleton value access、object init function 生成与 body lowering、once guard / singleton global / property global helper。
- 已从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除上述 enum/object lowering 主体实现，仅保留模块声明与共享/通用 helper。
- 过程中暴露并修复了一个本轮改动引入的可见性问题：
  - `object_init.rs` 中 `LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE` 需要从 `crate::llvm` 导入，不能继续走旧的 `super::...` 路径。
- 当前验证结果：
  - `cargo fmt --all` 通过；
  - `cargo test -p scoopc llvm::` 通过；
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 下一步：
  - 更新 `TODO.md` 与 `PLAN.md`，把 `T5000b3d` 标记完成并把下一条待执行任务切换到 `T5000b3dR`；
  - 检查 diff 后提交本轮改动并停止。
