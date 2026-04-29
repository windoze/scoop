# Claude Plan

说明：按安全与协作约束，这里记录可执行计划与关键决策摘要，不记录隐藏的完整内部推理。

## 当前回合目标

只完成 `TODO.md` 中第一个未完成任务；若发现其前置缺陷或实现边界问题，则先修复该问题，或把该问题作为新的前置任务插入 `TODO.md` 并停止。

## 执行步骤

1. 检查最新提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估任务规模；若过大，则把它拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 读取相关代码与测试，确认实现位置、规格约束与可能的既有缺陷。
5. 实现当前任务或其首个子任务，保持改动尽量小且符合现有结构。
6. 运行相关测试；若发现任何既有缺陷、回归、规格不匹配或阻塞项，立即优先修复，或将其登记为前置任务并调整顺序。
7. 更新 `memory/claude_plan.md`、`PLAN.md`、`TODO.md` 以反映进展。
8. 按仓库约定创建一次 git 提交，然后停止。

## 完成标准

- 当前目标任务已完整实现，或已被合理拆分并完成首个子任务。
- 相关测试通过；如适用，运行 `cargo clippy --all-targets -- -D warnings`。
- `TODO.md` 与 `PLAN.md` 已同步更新。
- 变更已提交到 git。

## 待确认点

- 需要先核实最新提交是否声明了必须先处理的问题。
- 需要确认 `TODO.md` 的首个未完成项是否已经足够小。

## 当前进展

- 已检查最新提交 `779e45a3bc36e97e7b7566b20513866b9c9cfda3`，提交主题为 `T5001e2R` review，提交信息本身未声明新的既有缺陷需要抢先修复。
- 已确认 `TODO.md` 中首个未完成任务是 `T5001f`：切换默认 explicit mode 到 explicit root frame，并停止默认路径的 stackmap 生成与使用。
- 当前轮接手的是一组已中断的未提交改动；这些改动已覆盖编译器、runtime 与 fixture，目标均指向 `T5001f`。
- 已初步核实当前改动方向：
  - 默认 codegen 已移除托管函数与入口 `main` 的 `gc "statepoint-example"` 标记；
  - runtime 默认初始化已停止自动注册当前进程 stackmap registry；
  - managed roots 枚举在默认路径下仅消费 explicit root frame snapshot，不再把 stackmap ctx 视为默认来源；
  - LLVM/object/fixture 回归已改为断言默认 explicit mode 不再出现 stackmap/statepoint；
  - 一批仅服务默认 stackmap 路径的 fixture 已被删除，并新增默认 explicit-mode 无 stackmap build fixture。
- 下一步将继续完成三件事：
  1. 细查剩余代码路径，确认默认 explicit mode 是否还残留隐含 stackmap 依赖；
  2. 运行定向测试与 fixture，若暴露既有缺陷则优先修复；
  3. 在确认实现完整后，更新 `PLAN.md` / `TODO.md` 的完成记录并准备提交。

- 已完成首轮实现：
  - 移除了默认 codegen 对托管函数和入口 `main` 的 `gc "statepoint-example"` 标记；
  - 保留 `rewrite-statepoints-for-gc` 管线代码，但在默认模式下不再有函数进入该路径；
  - 停止 `scoop_runtime_init()` 的默认 `stackmap registry` 注册；
  - 把 LLVM/object 侧测试改为断言默认产物不再含 stackmap/statepoint；
  - 删除默认测试矩阵中依赖 stackmap dump/registry smoke 的 run-pass fixture，并新增默认 explicit-mode build fixture。

- 已进一步核实：
  - 最新提交 `779e45a3` 仅是 `T5001e2R` review，未声明新的待优先修复问题；
  - 当前未提交改动已覆盖 runtime 初始化、GC in-native roots 枚举、默认 main lowering，以及 LLVM/object/runtime 测试的默认无 stackmap 合同；
  - `stackmap_registry` Rust 测试已改为“默认不自动注册，但仍允许手动注册当前进程 stackmaps”，符合“stackmap 保留为可选实现”的目标。

- 下一步：
  1. 继续检查其余 codegen 改动（`mod.rs`、`mir_body.rs`、`closure`、`object_init`、`state_machine_emitter` 等），确认默认 explicit mode 不再隐式依赖 stackmap/statepoint。
  2. 运行定向单测、build fixture 与 `clippy`；任何单个用例若接近 1 分钟仍未结束，立即视为异常并停下来查原因。
  3. 若验证通过，补齐 `PLAN.md` / `TODO.md` 的 T5001f 完成记录，并按 `PROMPT.md` 要求收尾（包含提交）。

- 当前结果：
  - 已确认默认 explicit mode 不再给 synthetic `main` 与其他托管函数打 `gc "statepoint-example"`，默认 IR / object 产物不再生成 statepoint/stackmap。
  - 期间暴露并修复了一处真实 lowering 缺口：`main` 的 explicit frame storage alloca 若沿用当前 builder 插入点，会因为不支配后续 frame setup/use 而触发 verifier；现已改为强制插到函数 entry 的 alloca 区。
  - 期间也修正了两处已过期断言：`thread_join` LLVM 单测现在匹配 explicit-frame home-slot keepalive 命名；`extern_enter_native_no_statepoint_writeback` fixture 现在断言 native 边界后从 `%explicit_root_frame_slot_0` reload。
  - 验证已完成并通过：`cargo test -p scoop_runtime`、五条定向 `scoopc` LLVM 单测、`cargo run -p scoop -- test --fixtures tests/fixtures/build`、`cargo clippy --all-targets -- -D warnings`。
  - 下一步只剩按仓库要求创建 `T5001f` 提交，然后停止，不继续推进 `T5001fR`。
