## 当前执行计划

当前目标：完成 `CG-T07S0a16a`，修复 direct `Array<UInt8>` element path 再次退回 nominal/composite surface 的回归。

1. 复核 `TODO.md` 中 `CG-T07S0a16a` 与最近一次提交，确认这是当前首个未完成任务，且最新提交记录的 blocker 正是本任务本体。
2. 复现 `tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop` 的失败，并用 `dump-mir` / 定向测试确认 direct `Array<UInt8>` builder、`get`、compare 哪一层把 scalar surface 漂移成 composite `Struct`。
3. 阅读 direct array element expected-type / scalar alias canonicalization 相关实现（优先 typecheck、HIR/MIR lowering、materialize、LLVM lowering 中与 `UInt8` / array element surface 发布直接相关的位置），定位 regression 根因。
4. 以最小改动修复 authoritative contract，让 direct `Array<UInt8>` path 继续发布 canonical scalar `UInt8` surface，而不是在后续 `get` / compare 路径退回 nominal/composite `Struct`。
5. 补最小回归测试，覆盖 direct `Array<UInt8>` builder/get/compare path，防止再次回退。
6. 运行本任务要求的验证命令，并补跑必要的定向单测、格式化与 `clippy`；若出现阻塞当前任务的新真实缺口，则按顺序更新 `TODO.md` 并停止。
7. 在 `TODO.md` 中把 `CG-T07S0a16a` 标记为 `[DONE]` 并填写完成记录，随后提交本次任务相关变更并停止。

## 进展记录

- 已创建计划文件并读取 `TODO.md`。
- 已确认首个未完成任务是 `CG-T07S0a16a`。
- 已检查最新提交标题 `[CG-T07S0a16a] Record direct UInt8 regression blocker`，其内容直接描述当前 blocker；本次继续在该任务上完成修复。
- 已复现 `literal_numeric_expected_type_absorption_basic.scoop` 的 direct `Array<UInt8>` 回归：builder push 仍是 `UInt8` scalar，但 `scoop.core.get` 结果 transport 被重新归成 nominal aggregate/composite surface，导致末两行输出回退为 `false` / `false`。
- 已完成实现：在共享 transport/composite-layout 分类逻辑中，把 builtin nominal scalar value type 视为标量而不是 aggregate；这样 direct `Array<UInt8>.get` 不再发布 trace/drop/aggregate-return/composite-runtime metadata。
- 已补充回归覆盖：保留 LLVM 侧 builder-path 生产测试，并新增 `mir::lower::tests::dump_mir_uint8_array_get_keeps_scalar_transport_metadata` 锁定 `bytes.get(...)` 两个 direct 读取站点的 scalar transport contract。
- 已完成验证：`cargo fmt --all`、`cargo test -p scoopc uint8_array`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo run -p scoop -- dump-mir tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop`、`cargo clippy --all-targets -- -D warnings` 全部通过。
- 下一步：更新 `TODO.md` 把 `CG-T07S0a16a` 标记为 `[DONE]`，记录完成说明与验证命令，然后提交本次任务并停止。
