# 本轮执行计划与进度记录

## 说明

- 本文件记录当前轮可共享的执行计划、判断依据摘要、关键发现、风险与进度。
- 不记录模型私有内部推理逐字过程，但会尽量完整记录外显步骤，便于审阅。

## 当前目标

完成 `TODO.md` 中第一个未完成任务 `T4011S`：收口 enum payload 中的一般 nested enum / builtin enum 字段表示。本轮只处理这一个任务，完成后提交并停止。

## 已完成的前置检查

1. 已检查工作区状态：`git status --short` 为空，当前无未提交改动。
2. 已检查最新提交：`fd9aadf8ea1bba8dd6c53c840346fc2ff75c5a26`，提交信息为 `[T4011R] Review payload or-pattern boundaries`。
3. 已检查最新提交是否明确挂出先修 issue：
   - 提交信息与变更文件仅记录 `T4011R` review 收口，没有额外挂出的未修复问题。
   - 因此当前仍按 `TODO.md` 的顺序继续推进。
4. 已读取 `TODO.md` 与 `PLAN.md`：
   - 当前第一个未完成任务为 `T4011S`。
   - `PLAN.md` 当前下一项也明确推进到 `T4011S`，与 `TODO.md` 一致。

## 任务边界摘要

`T4011S` 当前明确覆盖两类真实实现缺口：

1. boxed enum payload object / type descriptor 遇到 builtin enum 字段（例如 `Option<T>`）时，LLVM 类型映射或 metadata 仍可能报 `struct field type`。
2. 非 boxed 的 nested enum payload 当前只收口了 niche-nested 的局部路径；普通 custom enum 作为另一个 enum 的 payload 时，仍可能报 `enum payload (nested enum, unsupported repr)`。

验收标准：

- 相关 probe 不再报 `struct field type`。
- 相关 probe 不再报 `enum payload (nested enum, unsupported repr)`。
- 需要补充 run-pass / regression，覆盖 builtin enum field 与 custom enum field 作为 enum payload 的组合。

## 当前执行计划

1. 读取 `T4011S` 相关代码路径与已有测试，定位当前失败入口：
   - enum layout / metadata
   - LLVM enum payload lowering
   - payload object type / type descriptor 生成
2. 构造或复用最小 probe，确认两类报错是否仍可稳定复现，并分离是否属于同一个底层表示缺口。
3. 评估 `T4011S` 是否可在本轮完整交付：
   - 若范围收敛且主线统一，则直接实现。
   - 若发现任务实际上包含多个必须串行收口的前置缺口，则按要求在 `TODO.md` / `PLAN.md` 拆分，并仅执行第一个子任务。
4. 实现代码修改，要求：
   - 不通过“只避开某种 payload 形状”的方式绕过问题。
   - 统一修复 builtin enum field metadata 与 nested enum payload 表示主线。
5. 运行验证：
   - 定向最小 probe / 新增回归
   - 相关 fixture 子集
   - `cargo test --all -- --test-threads=1`
   - `cargo clippy --all-targets -- -D warnings`
6. 更新 `TODO.md` / `PLAN.md` / 本文件，并提交。

## 当前进展

- 已确认本轮目标是 `T4011S`。
- 已完成最小复现，当前判断本轮不需要继续拆分；两类报错都落在同一层 enum payload 表示 / layout 类型保留问题上，可作为一个完整切片收口。
- 已定位的直接代码入口：
  - `crates/scoopc/src/hir/lower/util.rs`
  - `crates/scoopc/src/typecheck/layout.rs`
  - `crates/scoopc/src/llvm/codegen/layout.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 当前下一步：修复 builtin `Option<T>` 字段在 layout 收集中的 `TypeId`/layout FQN 保留，并统一 nested enum payload 的 boxing 决策。

## 复现结论

### Probe 1：boxed payload + builtin enum 字段

- 文件：`/tmp/t4011s_boxed_builtin_enum_field_probe.scoop`
- 命令：
  - `cargo run -q -p scoop -- build /tmp/t4011s_boxed_builtin_enum_field_probe.scoop -o /tmp/t4011s_boxed_builtin_enum_field_probe.out`
- 结果：
  - 失败，报 `scoop::llvm::unsupported_main_body`
  - 具体节点：`struct field type`
- 当前判断：
  - `Option<T>` 这类 builtin enum path 在 layout 收集中没有稳定恢复为字段 `TypeId`，后端回退到只看 `ty_fqn` 时丢失了具体实例信息。

### Probe 2：custom nested enum payload

- 文件：`/tmp/t4011s_nested_custom_enum_probe.scoop`
- 命令：
  - `cargo run -q -p scoop -- build /tmp/t4011s_nested_custom_enum_probe.scoop -o /tmp/t4011s_nested_custom_enum_probe.out`
- 结果：
  - 失败，报 `scoop::llvm::unsupported_main_body`
  - 具体节点：`enum payload (nested enum, unsupported repr)`
- 当前判断：
  - 当前实现只让 niche-nested enum 继续走 inline payload；普通 tagged union nested enum 仍被错误当成 inline 小 payload，而没有在布局阶段进入 boxed payload 主线。

## 实现决策

- 本轮收口边界拟定为：
  - builtin `Option<T>` 等 enum-typed 字段在 layout 收集阶段必须保留真实 `TypeId` / layout FQN；
  - nested enum payload 中，仍可 inline 的仅限既有 niche 路径；
  - 其余 nested enum payload 统一进入 boxed payload 主线，不再落到“unsupported repr”。

## 待记录项

- 实现摘要：已完成；见下方“实现摘要”
- 测试命令与结果：已完成；见下方“测试结果”
- 文档更新摘要：已完成；见下方“文档更新摘要”
- 最终提交信息：`[T4011S] Support nested enum payload boxing`

## 实现摘要

- `crates/scoopc/src/hir/lower/util.rs`
  - 为 builtin `Option<T>` 增加稳定 layout key 生成，使 enum / boxed payload field 收集能够恢复真实 `TypeId`，不再只剩基名 `scoop.core.Option`。
- `crates/scoopc/src/typecheck/layout.rs`
  - 固定 nested enum payload 的 boxing 边界：仍保持 niche 表示的 nested `Option<T>` 可继续 inline，其余 nested enum 一律 boxed。
  - `Option<T>` 的 tagged-union fallback 现在也会按同一条 boxing 规则决定 `Some` payload 是否装箱。
- `crates/scoopc/src/llvm/codegen/layout.rs`
  - LLVM enum layout 与前端 layout 现复用相同边界：多字段 / tuple / struct / 非 niche nested enum 全部走 boxed payload 主线。
- `crates/scoopc/src/llvm/codegen/ty.rs`
  - boxed payload struct / object / type descriptor 的命名入口已扩展到 enum-like `TypeId`，builtin `Option<T>` 在 outer-option boxed payload 路径下也能生成稳定 LLVM 类型和 runtime type descriptor。
- 新增 run-pass 回归：
  - `tests/fixtures/run-pass/enum_payload_boxed_builtin_option_field_basic.scoop`
  - `tests/fixtures/run-pass/enum_payload_nested_custom_enum_basic.scoop`
  - `tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop`

## 测试结果

- 最小复现 probe：
  - `/tmp/t4011s_boxed_builtin_enum_field_probe.scoop` 现可成功 build，产物退出码为 `9`。
  - `/tmp/t4011s_nested_custom_enum_probe.scoop` 现可成功 build，产物退出码为 `7`。
- 新增回归定向验证：
  - `cargo run -q -p scoop -- build tests/fixtures/run-pass/enum_payload_boxed_builtin_option_field_basic.scoop -o /tmp/enum_payload_boxed_builtin_option_field_basic.out`
  - `/tmp/enum_payload_boxed_builtin_option_field_basic.out`：退出码 `94`
  - `cargo run -q -p scoop -- build tests/fixtures/run-pass/enum_payload_nested_custom_enum_basic.scoop -o /tmp/enum_payload_nested_custom_enum_basic.out`
  - `/tmp/enum_payload_nested_custom_enum_basic.out`：退出码 `37`
  - `cargo run -q -p scoop -- build tests/fixtures/run-pass/option_nested_custom_enum_payload_basic.scoop -o /tmp/option_nested_custom_enum_payload_basic.out`
  - `/tmp/option_nested_custom_enum_payload_basic.out`：退出码 `33`
- 全量验证：
  - `cargo fmt --all`
  - `cargo run -q -p scoop -- test`：通过，`fixtures: ok (1112)`
  - `cargo test --all -- --test-threads=1`：通过
  - `cargo clippy --all-targets -- -D warnings`：通过

## 文档更新摘要

- 已将 `TODO.md` 中的 `T4011S` 标记为完成，并记录本轮固定下来的实现边界与回归。
- 已将 `PLAN.md` 更新到 `T4011S` 完成状态，`P7` 标记为完成，下一项推进到 `T4012`。
- 本文件已补齐实现摘要、测试结果与后续状态。
