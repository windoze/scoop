# 执行计划

## 约束与目标

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在推进计划任务前，先检查最新提交是否提到既有问题；若提到，则优先修复。
- 在执行、测试、发现阻塞、调整任务拆分、完成关键步骤时，持续更新本文件。
- 若遇到任何既有缺陷、规格不匹配、实现边界缺失或依赖前置能力不足，必须优先修复，或将其作为新的前置任务写入 `TODO.md` / `PLAN.md` 后停止，不能绕过。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否显式提到待修复的既有问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和已有拆分。
4. 评估该任务是否过大；若过大，则先拆分并更新 `TODO.md` / `PLAN.md`。
5. 阅读与该任务直接相关的代码、测试、规格或文档，确定正确实现边界。
6. 实施修改，并补充或调整测试。
7. 运行相关验证；至少覆盖任务相关测试，并尽量满足 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等质量要求。若全量验证成本过高或受环境限制，需明确记录原因。
8. 更新 `TODO.md`、`PLAN.md`、本文件，标记当前任务已完成或记录新的前置阻塞。
9. 使用清晰的 Git 提交信息提交改动。
10. 停止，不继续处理下一个任务。

## 当前已知情况

- 已检查最新提交：`f723e3b2 [T5000e1R] Insert dump-ir materializer prerequisite tasks`。
- 最新提交没有直接修代码，而是把 `T5000e1R` review 暴露出的两个阻塞缺口回写成新的前置任务。
- `TODO.md` / `PLAN.md` 中第一个未完成任务已确认是 `T5000e1a 为 dump-ir materializer 补齐跨文件 / sysroot generic template 目录与正确的声明源身份`。
- `T5000e1a` 的直接背景：
  - `typecheck/lower.rs` 里记录的 `MonomorphKey.symbol.decl_file` 仍错误写成调用点文件；
  - `mir/materialize.rs` 当前只为“当前输入文件”的 generic template 建目录，导致 imported / sysroot generic fun 在 `dump-ir` 路径上找不到 template。
- 该问题已被上一提交明确认定为阻塞 `T5000e1R` 的既有缺陷，因此本轮应直接修复它，而不是继续做后续 review 或新功能。

## 接下来要做的事

1. 阅读 `crates/scoopc/src/typecheck/lower.rs` 中 monomorph 请求记录逻辑，确认声明源信息当前如何生成。
2. 阅读 `crates/scoopc/src/mir/materialize.rs`、`crates/scoop/src/commands/dump_ir.rs` 与相关测试，确认 dump-ir template catalog 当前只覆盖本地文件的具体实现边界。
3. 设计并实现 `T5000e1a`：
   - 修正 imported / sysroot generic fun 的请求键声明源身份；
   - 扩展 dump-ir materializer 的 template catalog，使其覆盖调试路径可达的外部 generic template；
   - 保持改动边界停留在 dump/debug 路径，不提前把编译单元主路径整体迁移进来。
4. 增加或更新回归测试，优先覆盖 imported / sysroot generic direct call 在 `dump-ir` 上的 materialization。
5. 运行格式化、相关测试和必要的全量验证。
6. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 当前实现设计

- `record_monomorph_call(...)` 现有签名不足以记录真实声明源，因为调用点已经拿得到 `sig.decl_file`，但函数本身只收 `decl_span`。本轮会把它改成显式接收 `decl_file`，并更新所有调用点。
- `dump-ir` 目前的 generic template 输入有两个缺口：
  - 只解析/resolve/typecheck 当前输入文件，没有把 `sysroot/print.scoop` 这类“可编译 sysroot 实现文件”加入调试路径；
  - template catalog 只扫当前文件，而且只收“带 body 的 generic fun”，导致 `scoop.channels.channelCreate<T>` 这类 declaration-only generic fun 也会漏掉。
- 拟采用的修复方式：
  1. 在 `mir/materialize.rs` 内为 dump/debug 路径补一个小型 frontend 准备流程，只覆盖：
     - sysroot 声明文件（克隆 AST，供索引/模板目录/声明型 generic fun lowering 使用）；
     - `session.sysroot().compilable_source_paths` 中的可编译 sysroot 源；
     - 当前输入文件。
  2. 该流程会：
     - 为上述文件建立统一 `Index`；
     - 对需要的文件做 resolve/typecheck；
     - 仅从当前输入文件收集 monomorph 请求；
     - 用整组文件 lower 出 generic HIR/MIR template 输入，供 materializer 查询外部 template。
  3. `collect_generic_template_infos(...)` 会扩成按整个编译单元收集，并纳入 declaration-only generic fun，而不是继续假设“有 body 且定义在当前文件”。
  4. 回归测试至少补两类：
     - `print<T>` 这种位于 compilable sysroot 源中的 generic fun；
     - `channelCreate<T>` 这种位于声明型 sysroot 文件中的 generic fun。

## 已完成步骤

1. 已修正 `MonomorphKey` 声明源记录：
   - `crates/scoopc/src/typecheck/lower.rs` 的 `record_monomorph_call(...)` 已改为接收真实 `decl_file`；
   - `crates/scoopc/src/typecheck/expr/call.rs` 的所有泛型调用记录点都已传入 `sig.decl_file` / `sig.decl_span`。
2. 已扩展 dump/debug template 输入：
   - `crates/scoopc/src/mir/materialize.rs` 现在会准备 sysroot 声明文件克隆、`stdlib/*.scoop`、可编译 sysroot 源和当前输入文件；
   - 会在该文件集上统一做 trim / index / resolve / typecheck / lowering；
   - template catalog 已扩成按整组文件收集，并纳入 declaration-only generic fun。
3. 已补回归测试：
   - `monomorph_materializes_compilable_sysroot_generic_template`
   - `monomorph_materializes_declaration_only_sysroot_generic_template`

## 验证结果

- `cargo fmt --all`：通过。
- `cargo test -p scoopc monomorph::lower -- --nocapture`：通过。
- `cargo run -q -p scoop -- dump-ir <tmp print case>`：通过；已确认 `print::<Int>` 指向 `sysroot/print.scoop`。
- `cargo run -q -p scoop -- dump-ir <tmp channelCreate case>`：通过；已确认 `channelCreate::<Int>` 指向 `sysroot/channels.scoop`。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

## 收尾步骤

1. 将 `T5000e1a` 标记为完成，并在 `TODO.md` / `PLAN.md` 中记录实现与验证结果。
2. 检查工作区 diff，确认只包含本轮任务需要的改动。
3. 提交 Git commit，并停止；下一轮应进入 `T5000e1aR`。
