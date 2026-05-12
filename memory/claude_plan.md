## 当前执行计划

说明：按安全与协作要求，这里记录可执行计划、关键判断依据、进度与变更，不记录内部私有推理细节。

当前任务：`P0-T02：固化现有测试基线，移除对旧命名字符串的强绑定`

### 任务理解

- `TODO.md` 中首个未完成任务是 `P0-T02`；`P0-T02R` 及后续任务均不得提前执行。
- 本任务的核心不是修改命名规则本身，而是把测试从“锁死旧字符串”迁移到“验证 visibility / linkage / namespace / 稳定性语义”。
- `.cone` / JSON 相关 active schema 目前被 `PLAN.md` 与 `STABLE_ID.md` 明确视为健康基线；本任务只补防回归断言，不重写 schema 结构。

### 执行步骤

1. 检查最近一次提交信息，确认是否存在与 `P0-T02` 直接相关且显式未完成的事项；如有，视为本任务范围或在 `TODO.md` 中补成前置依赖。
2. 读取 `crates/scoopc/src/llvm/tests.rs` 中 `TODO.md` 已点名的旧字符串强绑定断言位置，以及与其直接相关的辅助函数和测试样例。
3. 读取四个健康 schema 文件及其现有测试入口，确认当前已有的 `.cone` / JSON 基线测试覆盖方式，并找出适合补“无 dense id / 无绝对路径泄漏”断言的位置。
4. 最小化修改测试：
   - 把 `llvm/tests.rs` 中锁定旧名字拼写的断言改为锁定 external/private、命名空间、是否含稳定 hash 主体、是否避免 dense-id 形状。
   - 复核 `Step__schema*`、`lambda`、`object_init` 相关 IR 断言，避免继续把 `scoop.lambda$0`、`__schema3`、`__scoop_object_init__...` 之类字符串当金标准。
   - 为 `schema.rs`、`pre_specialize.rs`、`visibility.rs`、`annotations.rs` 补充基线断言，确认不泄漏 dense id、绝对路径或明显 path/span identity。
   - 在测试常量、注释或 helper 中明确区分“允许变化的 textual surface”和“禁止漂移的行为语义”。
5. 运行格式化与验证：至少执行 `cargo test -p scoopc`；若本任务改动触发 lint 风险，则补跑 `cargo clippy --all-targets -- -D warnings`。
6. 若验证通过，更新 `TODO.md`：将 `P0-T02` 标为 `[DONE]`，填写改动范围、核心决策、验证结果、与 `PLAN.md` / `STABLE_ID.md` 的闭合关系；如未发生阶段级依赖变化，不修改 `PLAN.md`。
7. 检查工作区，按要求提交本次任务改动并停止。

### 关注点

- 不能把旧命名再“包装一下”继续作为测试金标准。
- 不能通过简单放松断言来掩盖真实行为回归；行为语义相关断言必须保留。
- 若发现当前任务被真实实现缺口阻塞，必须先修复阻塞问题或在 `TODO.md` 中新增最小前置任务，然后提交并停止。

## 进度记录

- 已读取 `TODO.md`，确认首个未完成任务是 `P0-T02`。
- 已读取 `PLAN.md` P0 与 `STABLE_ID.md` §3.1 / §3.4 / §10 的相关段落，确认本任务边界是“测试基线整改”，不是“命名规则重写”。
- 已检查最近一次提交：`[P0-T01] Establish stable-id audit scaffolding`，未发现必须并入 `P0-T02` 的显式未完事项。
- 已完成第一轮测试迁移与基线补强：
  1. 在 `crates/scoopc/src/llvm/tests.rs` 中新增语义化 helper，把一组已知旧命名强绑定断言改为围绕 Step payload 结构、surface-resume 路径、hidden-init private helper、descriptor family 与 closure 行为建模。
  2. 新增 `stable_id_source_inventory_removes_known_legacy_name_bindings_from_behavior_tests`，防止已知的 `scoop.lambda$0` / `__scoop_object_init__...` / `Step__schema3` / `__scoop_refactor_surface_resume__k3` 行为测试回流。
  3. 在 `crates/scoopc/src/cone/scoopir/schema.rs`、`pre_specialize.rs`、`visibility.rs`、`annotations.rs` 增加 JSON 基线测试，断言这些健康 schema 的示例序列化仍不包含 dense-id/path/span 文本。
- 额外观察：`pre_specialize.rs` 的生成路径仍有 `mir_debug: format!("{mir_file:#?}")`。当前按 `TODO.md` 指定的 `44-84` schema 线段先补 schema-surface 基线断言；验证阶段若暴露出实际 `PRE_SPECIALIZE.json` 产物仍泄漏 dense id，需要把它视作当前任务的真实 blocker 处理。
- 已完成验证：
  1. `cargo fmt`
  2. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  3. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
  4. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  5. `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- 已完成文档回写：`TODO.md` 已把 `P0-T02` 标为 `[DONE]`，并补全改动范围、核心决策、验证结果与闭合说明；`PLAN.md` 无需变更。
- 下一步：检查 diff 与工作区状态，准备创建 `P0-T02` 提交，然后停止。
