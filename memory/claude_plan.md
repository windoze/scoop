# 执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未带 `[DONE]` 的任务后停止。
- 不做开放式历史问题扫查；只处理当前任务、当前任务阻塞项，以及验证时暴露且未被明确排期的失败。
- 如遇必须新增的前置任务，更新 `TODO.md`、必要时更新 `PLAN.md`，提交后停止。

## 步骤

1. 读取 `TODO.md`，定位第一个未完成任务，并查看最近提交是否明确提到与该任务直接相关的未完成事项。
2. 阅读该任务相关代码、规格、测试和夹具，确认完成条件与验证要求。
3. 以最小正确改动实现当前任务；如果发现规格缺口或阻塞项，不用变通方案绕过。
4. 按要求更新或新增相关测试/fixture。
5. 执行格式化、lint、相关测试；如果代码有实际改动，再按要求执行完整验证。
6. 更新 `TODO.md`：任务完成时在标题加 `[DONE]` 并填写完成记录；若阻塞则插入最小必要前置任务。
7. 必要时更新 `PLAN.md`，仅限阶段计划或依赖结构真实变化。
8. 提交所有本轮相关改动，提交信息包含任务编号并描述结果。
9. 停止，不继续下一个任务。

## 进度

- 已定位当前任务：`T2-05`，目标是把 per-call-site / dispatch facts 从 `(owner_callable, site_id)` 平表迁移到 callable 体内 site 节点。
- 最近提交为 `T2-04-R` review 完成记录，未发现直接要求插入到 `T2-05` 前的未完成事项。
- 已阅读 facts 定义、builder、LLVM main context、layout dynamic invoke、reachability 与 verifier/dump 相关路径。
- 迁移方案：构造期可继续使用临时 `(owner, site)` map 去重，但最终 `LirFacts` 不再发布这些平表；payload 挂到 `LirCallableFacts`、plain call-site、control boundary/source-statement 节点中，消费侧从 active LIR program/callable 节点 walk。
- 已修改 `scoopc_lir_facts` 数据结构、builder、LLVM main context、layout dynamic invoke、reachability、verifier/dump 与相关单测构造；`LirFacts` 顶层不再发布 source/class/reflection site 与 dynamic/dispatch 平表。
- 已执行针对相关 crate 的 `cargo check -p scoopc_lir_facts -p scoopc_codegen_llvm -p scoopc --all-targets`，结果通过。
- 已修复完整 fixture suite 暴露的重复 dynamic-invoke contract：同一 `(owner, site)` 已由 boundary 拥有时，不再复制到 plain/source-statement site 的 dynamic payload。
- 已删除 `scoopc_lir_facts` 公共复合 site key 类型；builder 内仅保留私有 `BuildCallSiteKey` 作为构造期临时 key。
- 最终验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`。
- `TODO.md` 已将 `T2-05` 标记为 `[DONE]` 并记录完成内容；下一步检查 diff/status 后提交。
