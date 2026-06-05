# 当前执行计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务。
2. 检查该任务的要求、依赖、验证条件和完成记录；必要时查看最近提交是否提到与该任务直接相关的未完成问题。
3. 只围绕该任务收集代码上下文，避免进行无关的历史问题扫描。
4. 如果任务可以直接实现，则完成实现、补充或更新最小必要测试。
5. 按要求先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，通过后运行相关测试、完整 Rust 测试和完整 fixture 套件，除非确认本次仅修改文档且可复用已有绿色结果。
6. 若发现阻塞当前任务的真实实现缺口或未安排的测试/fixture 失败，则修复它；如果无法在本次按原任务完成，则在 `TODO.md` 中加入最小必要前置任务并停止。
7. 完成后在 `TODO.md` 中给当前任务标题加 `[DONE]`，更新完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
8. 检查工作区变更，提交本次任务相关所有改动，然后停止，不进入下一个任务。

## 进度记录

- 已建立本次执行计划；下一步读取 `TODO.md` 确认第一个未完成任务。
- 已确认第一个未完成任务为 `TC-04-FIX1`。本次范围限定为清除 LLVM codegen 生产路径中剩余的 root/FQN live callable 查找；下一步核对最近提交并定位命中点。
- 最近提交 `d94a1491 [TC-04-R] Schedule FQN lookup review fix` 与当前任务直接相关；工作区除本计划文件外无其他未提交改动。
- 已定位生产路径命中点集中在 `identity.rs`、`call/abi.rs`、`call/lowering.rs`、`effect_lowered/{value,closure_adapter,types,body}`、`mir_body/{call,callable_lookup,operand}` 和 `gc.rs`。下一步先引入 handle/body-version-key 查询入口并替换直接生产路径。
- 已完成第一轮替换：body 发射绑定当前 active LIR program；新增 `LirCallableId`/`LirCallableRef` 与 body-version-key layout 查询；删除 `lir_callable_ref_for_root` / `exported_abi_symbol_for_lir_root` 生产 helper；生产路径旧 grep 已清零，剩余命中仅在 layout 测试辅助。下一步运行 `cargo fmt` 并修复编译/lint 问题。
- 完整 Rust 测试首次暴露 `p7_default_pipeline` effect fixture 运行期 `exit(7)`；定位为 LIR plain interface dispatch 缺少旧 MIR 路径已有的静态 interface target 选择，`println<String>` 退化为空 itable 动态分派。已补齐 LIR 静态 interface dispatch helper 并复测 `cargo test -p scoop --test p7_default_pipeline -- --nocapture` 通过；下一步重跑 clippy 与完整基线。
- 最终验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。已将 `TC-04-FIX1` 在 `TODO.md` 标记为 `[DONE]` 并补充完成记录；下一步提交所有本任务改动。
