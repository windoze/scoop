## 本次执行计划

说明：按安全与协作规范，这里记录的是可审计的执行计划、关键判断依据与进度更新，不暴露内部私有推理细节。

1. 先读取 `TODO.md`，找到标题中第一个未带 `[DONE]` 前缀的任务；该文件是任务顺序、依赖、验证要求与完成记录的唯一准则。
2. 在不做开放式历史问题排查的前提下，检查当前仓库状态，并查看最近提交是否直接提到与该任务相关且未完成的问题；若存在并且会阻塞当前任务，则将其视为当前任务的一部分或作为新的前置任务写入 `TODO.md`。
3. 阅读当前任务涉及的最小必要代码、测试、规范和文档，确认实现边界、依赖和验证要求；不为方便而擅自拆分任务，除非确实存在必须先补的具体前置项。
4. 实现当前任务所需的最小正确改动，避免引入规避性方案、夹带式兼容层或与规范不一致的捷径。
5. 运行与该任务直接相关的验证命令；如有需要，再运行更广泛的回归验证，至少覆盖任务在 `TODO.md` 中要求的检查。
6. 若发现阻塞当前任务的真实缺陷、缺失特性或规范不匹配：
   - 不用 workaround 继续推进；
   - 在 `TODO.md` 中以最小必要粒度加入前置任务并调整顺序/依赖；
   - 仅在阶段计划确实变化时更新 `PLAN.md`；
   - 提交这些变更并停止。
7. 若任务完成：
   - 在 `TODO.md` 中把该任务标题显式改为 `[DONE]`；
   - 更新该任务 completion record；
   - 仅在阶段计划变化时更新 `PLAN.md`；
   - 提交所有本次任务相关改动；
   - 停止，不进入下一个任务。
8. 在执行过程中，如计划改变、发现关键阻塞、开始实现、完成验证、准备提交，都回写本文件，便于审计进度。

## 进度更新

- 初始状态：已记录执行计划，尚未读取 `TODO.md`。
- 已读取 `TODO.md` 并确认第一个未完成任务为 `CG-T07S0a15a`：修复 `MutableSet.asSet()` 只读视图在同一 body 组合 `Set.len()` / `Set.contains()` 时的 alias receiver call 结果漂移。
- 已检查最近提交：`[CG-T07S0a15] Repair map empty-table transport and queue Set view blocker`，其内容与当前任务直接相关，且 `TODO.md` 已把该未完问题登记为当前任务的前置 blocker，无需额外改动任务结构。
- 当前执行细化计划：
  1. 复现 `tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop` 的 run-pass 失败，并尽量缩小到 `set_read_only_view` / `MutableSet.asSet()` / `Set.len()` / `Set.contains()` 交互路径。
  2. 阅读与该路径最相关的最小代码范围，优先检查 `stdlib/collections_set.scoop`、对应 MIR/lowering/materialize/codegen 中的 alias receiver / array view / member call 路径。
  3. 确认根因后实施最小正确修复，避免对 `0` 元素、`Array<Int>`、`Set` API 或 fixture 做特判。
  4. 运行任务要求的定向验证；若通过，再更新 `TODO.md` 完成记录并提交。
- 根因更新：`stdlib_hash_set_map_basic.scoop` 当前在更早的 `MutableSet` 路径就已受同一根因影响。`dump-ir` 显示 `Set.len/contains` 与 `MutableSet.len/contains` 作为 pass-visible 非泛型 typealias receiver 扩展，最终共享了同一个 root/callable FQN（如 `scoop.collections.len` / `scoop.collections.contains`）；materialized callable family 只为其中一个 body 发布符号，另一个 family 变成无 body symbol。结果 `main` 中的 `s.len()` / `s.contains(...)` / `ro.len()` / `ro.contains(...)` 都可能落到同一个实现，直接破坏哈希布局与只读视图的契约。
- 修复策略更新：
  1. 为 pass-visible 非泛型 ordinary callable 引入稳定的 overload-aware 发布符号，避免 alias receiver 扩展同名 body 冲突。
  2. 在 reachable-body rewrite 阶段，依据已存在的 authoritative direct-call binding / resolved non-generic callee，把相关 direct call 与 top-level ref 重写到正确的 overload-aware 符号，而不是保留共享 root FQN。
  3. 补一条 materialize 定向回归，直接断言 `main` 对 `len` / `contains` 的 direct-call target 会区分 `Set` 与 `MutableSet` 两个实现；再跑任务要求的 run-pass fixture 和全仓 `clippy`。
- 实施结果：
  1. `crates/scoopc/src/mir/materialize.rs` 现在会为 pass-visible 非泛型重名 callable（本例即 `Set`/`MutableSet` alias receiver 扩展）发布 overload-aware symbol，并在 reachable-body rewrite / direct-call rewrite 中把 `MutableSet.len/contains` 与 `Set.contains` 的 call-site 重写到稳定符号，避免共用 `scoop.collections.len` / `scoop.collections.contains` root FQN。
  2. `crates/scoopc/src/llvm/codegen/mir_body.rs`、`crates/scoopc/src/llvm/codegen/effect_refactor/value.rs`、`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 现在会把这些 suffixed pass-visible callable 反查回对应 HIR signature/root，避免 LLVM plain/effect-refactor lowering 再把它们误判成缺 signature 的 function-value call。
  3. 新增 `materialize_for_dump_keeps_set_alias_receiver_overload_targets_distinct` 回归，锁定 `stdlib_hash_set_map_basic.scoop` 中 `MutableSet.len` 与 `Set`/`MutableSet.contains` 的 overload-aware call target rewrite。
- 当前状态：当前任务 `CG-T07S0a15a` 已完成，待同步 `TODO.md` 完成记录并提交。
- 已完成文档收尾：`TODO.md` 已把 `CG-T07S0a15a` 标记为 `[DONE]`，并补充根因、修复点与验证记录；同时在 `CG-T07S0a15` 的 completion record 中注明前置任务已解除、下一次调用应恢复其 full-suite 验证。
- 当前状态：所有代码/测试/文档改动已齐备，准备提交 `CG-T07S0a15a`。
- 实际验证：
  1. `cargo test -p scoopc materialize_for_dump_keeps_`
  2. `cargo run -p scoop -- build tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop -o /tmp/stdlib_hash_set_map_basic`
  3. `/tmp/stdlib_hash_set_map_basic`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/stdlib_hash_set_map_basic.scoop`
  5. `cargo clippy --all-targets -- -D warnings`
