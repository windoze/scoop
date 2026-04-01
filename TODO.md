# TODO（Scoop 0.1：按“可单独实现 & 单独验证”的小步拆分）

> 生成时间：2026-03-31（持续维护更新）  
> 依据：`SCOOP_FULL_SPEC.md` + `PLAN.md`  
> DONE 标记来自：对仓库现状的代码检查，并本地运行 `cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test` 均通过。

## 约定（阅读方式）

- 每个任务用 `TxxYY` 编号：
  - `xx` 表示阶段/主题（大方向）
  - `YY` 表示该主题下的序号
- 任务状态：
  - `[TODO]`：可立即实现与验证（本 agent 默认优先挑选第一个 `[TODO]`）
  - `[BLOCKED]`：依赖未满足，暂不可实现；需先完成依赖任务再回到该任务
  - `[DONE]`：已实现并验证
- 每个任务必须包含：
  - **描述**：一句话说明要做什么
  - **目标**：实现范围/不做什么（避免任务膨胀）
  - **验收**：可以复制执行的验证方式（尽量用 fixtures + `scoop test`）
  - **依赖**：写明依赖的任务编号（空表示可立即并行启动）
- “可并行”的含义：依赖不互相阻塞即可并行推进；但若代码写入范围冲突，仍建议串行合并。

---

## T00：现状核对（已完成 / DONE）

### T0001 [DONE] Rust workspace / crate 骨架齐全
- 描述：工程已拆分为 `scoop`/`scoopc`/`scoop_runtime`/`scoop_tools` 四个 crate 并可构建。
- 目标：保持 workspace 可增量扩展，不引入循环依赖。
- 验收：`cargo test --all` 通过；`Cargo.toml` workspace members 完整。
- 依赖：无

### T0002 [DONE] `scoop` CLI 基础命令可运行
- 描述：`scoop test`/`scoop dump-ast`/`scoop build`/`scoop run` 可用（`build/run` 需要启用 `--features llvm` 才会生成并运行二进制；默认 `build` 仅做前端检查）。
- 目标：driver 只做 I/O/调度；编译逻辑留在 `scoopc`。
- 验收：`cargo run -p scoop -- --help` 可输出帮助；`cargo run -p scoop -- test` 通过；`cargo run -p scoop -- dump-ast tests/fixtures/parse/hello.scoop` 能打印 AST。
- 依赖：无

### T0003 [DONE] 最小 fixtures runner（parse/resolve）可回归
- 描述：递归发现 `tests/fixtures/**/*.scoop`，按目录执行 parse 或 resolve，支持 pass/fail 断言。
- 目标：把“回归底座”先立住；后续扩展更多 phase 但保持兼容。
- 验收：`cargo run -p scoop -- test` 输出 `fixtures: ok (...)`；新增 fixture 后仍可稳定回归。
- 依赖：无

### T0004 [DONE] fixtures 指令解析（EXPECT/ERROR/位置/错误码）
- 描述：支持 `// EXPECT: pass|fail`、`// EXPECT-ERROR:*` 等文件头注释指令。
- 目标：保持指令解析简单、鲁棒；只扫描前 32 行。
- 验收：`cargo test -p scoop` 中 `fixtures::expectations` 单测通过。
- 依赖：无

### T0005 [DONE] 统一诊断框架（miette + thiserror）
- 描述：lexer/parser/resolve 等错误具备稳定 error code 与 span label。
- 目标：所有“可预期的用户错误”都应是结构化诊断，而非 panic。
- 验收：`cargo test --all` 通过；fixtures 的 `EXPECT-ERROR-CODE/AT` 能断言错误码与位置。
- 依赖：无

### T0006 [DONE] 日志基础设施（tracing）
- 描述：driver 初始化 `tracing-subscriber`，可用 `RUST_LOG` 控制日志级别。
- 目标：未来各 pass 逐步接入 tracing；默认不刷屏。
- 验收：`RUST_LOG=debug cargo run -p scoop -- test` 可看到 debug 日志（若后续加入）。
- 依赖：无

### T0007 [DONE] `SourceFile`/`Span` 行列映射基础设施
- 描述：支持 offset ↔ line/col 映射，用于精确诊断。
- 目标：行列映射要稳定且有单测覆盖。
- 验收：`cargo test -p scoopc source::tests::line_col_mapping_basic` 通过。
- 依赖：无

### T0008 [DONE] Lexer v0（关键字/注释/字符串/常见符号）
- 描述：`scoopc::syntax::lexer` 已覆盖早期需要的 token 子集（含 `f"..."`/raw string/`as?`/`?:`/`!!` 等）。
- 目标：先保证能支撑 parser/fixtures；语法糖的更深语义留给 parser/typecheck。
- 验收：`cargo test -p scoopc syntax::lexer::*` 全部通过。
- 依赖：无

### T0009 [DONE] Parser v0（文件头 + 顶层声明 + 括号平衡 body）
- 描述：可解析 `package/import`、顶层 `fun`、顶层 `val/var`、顶层 `class/interface/struct/enum/effect` 的最小骨架。
- 目标：先“能解析并产出 AST”，表达式/语句后续再补齐。
- 验收：`cargo test -p scoopc parser::*` 通过；`scoop dump-ast` 可输出基本结构。
- 依赖：T0008

### T0010 [DONE] Sysroot v0（`.scoop` 声明源）与加载注入
- 描述：`sysroot/core.scoop` 提供 `Any`、`Option<T>`、`Raise<E>`；`Session::new()` 默认加载 sysroot。
- 目标：把“内建 API 的表面”从硬编码迁移到可版本化的源文件。
- 验收：`cargo test -p scoopc sysroot::tests::load_default_sysroot` 通过；`session::tests::index_includes_sysroot_symbols` 通过。
- 依赖：T0009

### T0011 [DONE] 顶层符号索引（FQN）与最小名字绑定检查
- 描述：构建 `package + 顶层名` 的 FQN 索引，检测重复定义；检查 import 存在性；检查顶层签名里的 `TypeRef::Path` 可解析。
- 目标：先做“存在性 + 诊断精确”；不做作用域/重载/可见性。
- 验收：`cargo test -p scoopc resolve::*` 通过；`tests/fixtures/resolve/*.scoop` 在 `scoop test` 下通过。
- 依赖：T0009、T0010

### T0012 [DONE] spec doctest fixtures 抽取工具（`scoop_tools spec-fixtures`）
- 描述：从 `SCOOP_FULL_SPEC.md` 抽取带 `// FIXTURE:` 的 fenced code blocks 到 `tests/fixtures/spec_doctest/` 并校验路径规则。
- 目标：让规范示例可执行、可回归；防止漂移。
- 验收：`cargo run -p scoop_tools -- spec-fixtures check` 通过；`tests/fixtures/spec_doctest` 目录存在对应文件。
- 依赖：无

### T0013 [DONE] CI（ubuntu + clang）能跑全套最小回归
- 描述：GitHub Actions 安装 clang，跑 `cargo test --all`、spec-fixtures check、`scoop test`。
- 目标：保证任何 PR 都可回归（永远可回归原则）。
- 验收：`.github/workflows/ci.yml` 存在且包含上述步骤。
- 依赖：T0001、T0003、T0012

### T0014 [DONE] 早期 C runtime build glue（强制 clang）
- 描述：`crates/scoop_runtime/build.rs` 用 `cc` crate 编译 `runtime/c/scoop_runtime.c` 生成可链接静态库。
- 目标：先保证“可构建”，运行时语义后续逐步补齐。
- 验收：`cargo test --all` 在本机/CI 均可通过（含 clang 安装）。
- 依赖：无

---

## T01：测试体系与工具（增强，但保持小步）

### T0101 [DONE] fixtures runner：按目录路由更多 phase（typecheck/infer/effects/...）
- 描述：把 phase 判定从“只有 parse/resolve”扩展为按 `tests/fixtures/<phase>/` 路由。
- 目标：先把路由与报错信息做对；每个 phase 仍可先返回 “未实现” 的清晰错误。
- 验收：新增空目录 `tests/fixtures/typecheck`；`scoop test` 能识别该 phase 并给出“未实现”但带文件路径的诊断（并在 fixture 里可 EXPECT fail）。
- 依赖：T0003

### T0102 [DONE] fixtures 指令：支持 `// ARGS:`（传递给 driver/编译器阶段）
- 描述：允许单个 fixture 指定额外参数（例如 `--dump-ast` / `--emit-llvm` / `--gc-stress`）。
- 目标：只实现解析与结构化存储；不急于支持所有参数。
- 验收：新增 `crates/scoop/src/fixtures/expectations.rs` 单测：`ARGS` 可解析为 Vec<String>；原有指令保持兼容。
- 依赖：T0004

### T0103 [DONE] parse fixtures：支持 AST snapshot（golden）比对
- 描述：为 `tests/fixtures/parse/**` 增加 `// EXPECT-AST: <file>`，将 `Debug`/pretty-print 输出与 golden 文件对比。
- 目标：先只覆盖 parser 输出（不涉及 resolver/typecheck）；golden 采用“全文一致”。
- 验收：新增一个 parse fixture + 对应 `.ast` 文件；`scoop test` 在 parse phase 里完成比对。
- 依赖：T0003、T0009

### T0104 [DONE] compile-fail golden：升级为“错误码 + 位置 + 子串”组合断言
- 描述：把当前 fixtures fail 断言策略固化成推荐模板，并补齐文档与示例。
- 目标：先不引入 regex；保持实现简单。
- 验收：新增至少 2 个 compile-fail fixture（parse/resolve 各 1 个）同时断言 code+at+contains；`scoop test` 通过。
- 依赖：T0003、T0005

### T0105 [DONE] spec-fixtures：支持 `check --fix`（自动写回生成文件）
- 描述：新增选项在 spec 变更时自动更新 `tests/fixtures/spec_doctest/*`。
- 目标：默认仍是 check-only；`--fix` 只改动受影响文件。
- 验收：`cargo run -p scoop_tools -- spec-fixtures check --fix` 可运行；新增单测覆盖“写回行为不改变未变更文件”。
- 依赖：T0012

### T0107 [DONE] fixtures 指令：新增 `RUN-STDOUT`/`EXPECT-EXIT`/`TIMEOUT`
- 描述：扩展文件头指令解析，支持运行期断言（stdout 文件、退出码、超时毫秒）。
- 目标：先只实现“解析与结构化存储”；fixture runner 暂可忽略这些字段直到 T0106b2。
- 验收：为 `crates/scoop/src/fixtures/expectations.rs` 新增单测覆盖三个字段；旧指令保持兼容。
- 依赖：T0004

### T0106 run-pass fixtures（拆分为子任务）
- 描述：在 fixtures 体系里新增 run-pass（建议目录 `tests/fixtures/codegen` 或 `run-pass`），编译后运行并断言 stdout。
- 目标：先只做 stdout 对比；stderr/超时/退出码后续任务补齐。
- 备注：该能力依赖 driver 的 `scoop run` 与后端/链接（T0807）。为保证“可单独实现 & 单独验证”，拆分为以下两个子任务。

### T0106a [DONE] fixtures runner：run-pass phase + stdout golden 比对（不依赖 codegen）
- 描述：让 `scoop test` 识别 `tests/fixtures/codegen/**`（或 `run-pass/**`）为 run-pass phase；实现读取 `// RUN-STDOUT:` 指定的 golden 文件并与“实际 stdout”做比较（换行归一化）。
- 目标：只实现 stdout golden 的读取与比对；不实现真正编译/运行（由 T0106b2/T0807 接入）；不实现 stderr/超时/退出码断言。
- 验收：新增单测覆盖 stdout golden 的 pass/mismatch；`cargo test -p scoop` 通过。
- 依赖：T0107

### T0106b（拆分为子任务）
- 描述：run-pass phase 的“真实执行”需要同时具备：
  - fixtures runner 层面的进程执行/捕获（可单测）
  - driver 层面的 `scoop run`（T0807）与 build/link/codegen pipeline
- 目标：保证可单独实现 & 单独验证：先把 runner 的执行能力做出来，再接入 `scoop run` 并补一个真实可运行 fixture。
- 备注：与 `scoop run` 的集成（T0106b2）因依赖执行链路，已移动到 T08（紧随 T0807）以保持 TODO 顺序可用。

### T0106b1 [DONE] run-pass fixtures：引入可注入的“进程执行器”（捕获 stdout）
- 描述：为 run-pass phase 引入执行接口：给定一个命令（后续由 `scoop run` 提供），运行并捕获 stdout，然后与 `RUN-STDOUT` golden 做比对。
- 目标：只实现“执行外部命令 + 捕获 stdout + stdout golden 比对”；不实现真正编译 Scoop；不实现 stderr/超时/退出码断言。
- 验收：新增单测：执行一个最小外部命令并通过 stdout golden 比对；`cargo test -p scoop` 通过。
- 依赖：T0106a

### T0109 [DONE] lexer/parser fuzz（崩溃防线，可选但高收益）
- 描述：引入 `cargo-fuzz` 或最小随机输入测试，保证 lexer/parser 对任意输入不 panic。
- 目标：只要求“不崩溃”；不要求高质量错误恢复。
- 验收：新增 fuzz target（或单测）跑固定轮数；CI 可选跳过（但本地可跑）。
- 依赖：T0008、T0009

### T0110 [DONE] 覆盖矩阵检查：每个 spec 章节至少 1 pass/1 fail（PLAN §10.6）
- 描述：在 `tools/scoop_tools` 增加检查命令，扫描 fixtures 目录并提示缺口（按 spec/plan 章节映射）。
- 目标：先只做”提示/报告”（非强制 fail）；规则可逐步细化。
- 验收：`cargo run -p scoop_tools -- fixtures-matrix check` 输出报告；并有单测覆盖”缺口检测”。
- 依赖：T0012、T0003

### T0111a [DONE] run-pass fixtures：`RUN-STDERR` golden（核心逻辑，可单测）
- 描述：在 run-pass fixtures 中增加 stderr 捕获与 golden 比对能力，支持“仅 stdout”、“仅 stderr”或“两者同时断言”的场景，并保证 mismatch 诊断能区分 stdout/stderr 差异。
- 目标：不改变现有 `RUN-STDOUT` 语义；先只做文本 stderr golden，不做流间时序重建；不要求 `scoop test` 真实执行（留给 T0106b2）。
- 验收：新增单测覆盖：
  - `RUN-STDERR` golden 匹配通过；
  - stderr mismatch 返回稳定错误码（可断言 `err.code()`）；
  - 同时断言 stdout/stderr 时，stderr mismatch 仍能给出 stderr 的稳定错误码（而不是 stdout 的）。
- 依赖：T0106b1、T0107

---

## T02：Parser/AST（阶段 1：从“可解析声明”走向“可解析表达式”）

### T0200 [DONE] Lexer：补齐位运算与移位运算符 token（spec §2.3.4 / Appendix B.8）
- 描述：在 lexer 中新增对 `&`、`|`、`^`、`~`、`<<`、`>>` 的 token 支持。
- 目标：只做词法层 longest-match；不引入任何优先级/结合性逻辑；不实现复合赋值（如 `&=`）除非 spec 明确要求。
- 验收：新增 lexer 单测：包含上述符号的源码能被正确分词（并与 `&&`/`||` 区分）；`cargo test -p scoopc syntax::lexer::*` 通过。
- 依赖：T0008

### T0201 [DONE] AST：类型体成员（TypeDecl members）建模
- 描述：把 `TypeDecl.body: Option<Block>` 升级为”可包含成员列表”的结构（仍可保留 span）。
- 目标：先支持成员声明列表：`val/var/fun/nested type` 的最小骨架；成员体可继续只做括号平衡。
- 验收：新增 `tests/fixtures/parse/type_members_minimal.scoop` 覆盖 class/interface/struct/effect 的成员；`scoop test` 通过。
- 依赖：T0009

### T0202 [DONE] Parser：解析类型体内的 `val/var` 成员声明头
- 描述：在 type body 中解析 `val x: T`/`var x: T`，initializer 先保留 span。
- 目标：不做 accessor、不做 delegated property、不做表达式解析。
- 验收：parse fixture 覆盖成功/失败（缺少名字/缺少冒号等）；错误应给出 `scoop::parse::*` 错误码与准确位置。
- 依赖：T0201

### T0203 [DONE] Parser：解析类型体内的 `fun` 成员声明头
- 描述：在 type body 中解析 `fun name(params): Ret { ... }`（body 仍是 span）。
- 目标：不解析函数体语句；不支持表达式体 `= expr`（后续任务）。
- 验收：新增 parse fixture 覆盖成员函数；`scoop dump-ast` 输出包含成员列表。
- 依赖：T0201

### T0204 [DONE] Parser：解析类型体内的 nested type（class/interface/struct/enum/effect）
- 描述：允许在类型体内声明嵌套类型，并保留 span。
- 目标：仅做语法层嵌套，不做语义（inner/this 等）处理。
- 验收：新增 parse fixture 覆盖嵌套类型与重复定义错误（先由 resolver 后续阶段处理也可）。
- 依赖：T0201

### T0205 [DONE] AST：引入表达式/语句最小骨架（Expr/Stmt）
- 描述：为后续解析函数体与 initializer 做 AST 扩展：`Expr`/`Stmt` 的最小子集。
- 目标：第一步只需要：标识符、整数/字符串字面量、块表达式（空块即可）、缺失占位（Missing）。
- 验收：`cargo test -p scoopc` 通过；新增一个 parse fixture：`val x = 1` 能在 AST 里看到字面量表达式（或暂时 Missing，取决于实现选择）。
- 依赖：T0009

### T0206 [DONE] Parser：解析顶层 `val/var` initializer 的”原子表达式”
- 描述：把 `ValDecl.init: Option<Span>` 升级为 `Option<Expr>`，并能解析：ident/int/string/`( ... )`。
- 目标：不解析二元运算、不解析调用；错误恢复先最小化。
- 验收：新增 parse fixture 覆盖 `val a = 1`、`val b = "x"`、`val c = foo`；`scoop test` 通过。
- 依赖：T0205

### T0207 [DONE] Parser：解析块表达式 `{ ... }` 为 `Block { stmts }`
- 描述：把当前 `Block` 从仅 span 扩展为包含语句列表；先支持空语句与表达式语句。
- 目标：暂不支持控制流（if/when/return）；先保证括号匹配与 span 正确。
- 验收：新增 parse fixture：`fun f() { 1 }`；AST 中 block 至少包含 1 个 expr stmt。
- 依赖：T0205

### T0208 [DONE] Parser：解析函数体（block）中的 `val/var` 局部绑定（spec §9）
- 描述：支持 `val x: T = expr`/`val x = expr` 作为语句出现在 block 内。
- 目标：先不实现 destructuring；`var` 的赋值语义留给 typecheck。
- 验收：新增 parse fixture 覆盖局部 val/var；新增 parse-fail fixture：`val = 1`。
- 依赖：T0207

### T0209 [DONE] Parser：引入 postfix 表达式（调用 `f(...)`）
- 描述：支持函数调用表达式（callee + args），args 先支持逗号分隔表达式（基于已有原子表达式）。
- 目标：不实现命名参数、不实现 trailing lambda。
- 验收：新增 parse fixture：`val x = f(1, 2)`；以及 `fun main(){ f() }`。
- 依赖：T0206、T0207

### T0210 [DONE] Parser：引入成员访问表达式（`.`）与链式组合
- 描述：支持 `a.b`、`a.b.c()` 这种 postfix 链。
- 目标：只做语法树，不做名字解析；不处理 safe-call `?.`（如果 spec 需要可后续补）。
- 验收：新增 parse fixture：`val x = a.b.c(1)`；并确保 span 覆盖整段表达式。
- 依赖：T0209

### T0211 [DONE] Parser：实现二元运算优先级（precedence climbing / Pratt）
- 描述：支持 `+ - * / %`、比较/相等、位运算 `& | ^`、移位 `<< >>`、逻辑 `&& ||` 等优先级集合。
- 目标：先不引入操作符重载绑定规则（那是 typecheck 阶段）；仅保证语法树结合性正确。
- 验收：新增 parse fixture：`1 + 2 * 3` 解析为 `+(1, *(2,3))`；加一个 snapshot golden（配合 T0103 更佳）。
- 依赖：T0205、T0200

### T0212 [DONE] Parser：支持 Elvis `?:` 与 not-null `!!`（spec Appendix B.3）
- 描述：把 `?:` 作为低优先级二元；把 `!!` 作为 postfix。
- 目标：只做解析；类型/效果语义后续做。
- 验收：新增 parse fixture：`val x = a ?: b`、`val y = a!!`；`scoop test` 通过。
- 依赖：T0211

### T0213 [DONE] Parser：支持类型判断/转换操作符（`is`/`!is`/`as`/`as?`）（spec §4.3~§4.5）
- 描述：为表达式引入类型相关二元/三元节点（expr + TypeRef）。
- 目标：先完成语法；smart cast 与失败语义留给 typecheck/effect。
- 验收：新增 parse fixture：`if (x is Foo) { x }`（只要能解析）；`val y = x as? Foo`。
- 依赖：T0211

### T0214 [DONE] Parser：支持 `if` 表达式（spec §14.6/Appendix B）
- 描述：解析 `if (cond) thenExpr else elseExpr?` 作为表达式。
- 目标：先只支持必须带括号条件；允许 else 缺省（作为 `Unit` 或语法错误，按设计决定）。
- 验收：新增 parse fixture：`val x = if (a) 1 else 2`；新增 parse-fail fixture：缺少 `)` 的错误恢复。
- 依赖：T0205、T0207

### T0215 [DONE] Parser：支持 `when` 表达式骨架（spec §4）
- 描述：解析 `when (expr) { ... }`，case 先支持：`is T`、`else`、常量字面量分支。
- 目标：先不做穷尽性检查；pattern 的完整语义后续补。
- 验收：新增 parse fixture：带 3 个分支的 when；再加 1 个 parse-fail fixture：缺少 `else` 时仍能解析（允许后续 typecheck 报错）。
- 依赖：T0214

### T0216 [DONE] Parser：值类型更新 `with` 表达式语法（spec §2.6）
- 描述：解析 `expr with { path: value, ... }`，path 为 `a.b.c` 的字段路径。
- 目标：仅语法；不做字段存在性与类型检查。
- 验收：新增 parse fixture：`val p2 = p with { x: 1, y: 2 }`；覆盖多字段与嵌套 path。
- 依赖：T0211

### T0217 [DONE] Parser：字符串插值表达式（`f"..."`）的 token 分片
- 描述：把 lexer 产出的 f-string token 进一步在 parser 里拆为“文本片段 + 插值 expr”列表（spec §8.2）。
- 目标：先支持 `${expr}` 或 `{expr}` 一种形式（以 spec 为准）；raw f-string 同理。
- 验收：新增 parse fixture：`val s = f\"hi {name}\"`（按语言语法）；AST 中能区分 text 与 expr parts。
- 依赖：T0205、T0008

### T0218 [DONE] Parser：声明处泛型参数列表（`<T, U>`）（spec §3）
- 描述：为 `fun`/`class`/`struct`/`enum`/`effect` 增加 type params 的 AST 与解析。
- 目标：先支持不带约束的 `T`；variance/in/out/where 约束后续任务。
- 验收：新增 parse fixture：`fun id<T>(x: T): T { x }`；`struct Box<T> { val v: T }`（若已支持成员）。
- 依赖：T0201、T0203

### T0219 [DONE] Parser：函数类型与 effect row 的语法（spec §7.5、§5.8、§14.7）
- 描述：支持类型位置的函数类型：`(A, B) -> T` 以及带 `/ RowExpr` 形式。
- 目标：先只做解析，不做 subeffecting/推断；`RowExpr` 先支持 `Pure` 与 `E1+E2`。
- 验收：新增 parse fixture：`val f: (Int) -> Int / Pure`（或 sysroot 类型名）；确保 TypeRef/TypeKind 正确建模。
- 依赖：T0218

### T0220 [DONE] Parser：错误恢复（top-level 同步点 + block 同步点）
- 描述：遇到语法错误时跳过到下一个同步 token（如 `fun/class/val/var` 或 `}`）继续解析，产出更多诊断。
- 目标：先保证”不 panic + 尽量多报”；不追求 IDE 级别恢复质量。
- 验收：新增 parse fixture：同一文件里 2 个错误，runner 能同时报出（或至少不被第 1 个错误终止）；错误码稳定。
- 依赖：T0009
- 完成说明：
  - `parse_file_recovering()` 新 API：返回 `ParseResult { ast, errors }` 收集所有错误
  - 顶层恢复：`recover_to_top_level()` 跳到 `fun/class/val/var/...` 或 EOF
  - 块内恢复：`recover_to_stmt_boundary()` 跳到 `val/var/if/when/return/...` 或 `}`
  - 类型体恢复：成员解析失败时跳到下一个成员开始或 `}`
  - `parse_file()` 保持向后兼容（返回第一个错误）
  - fixture 新增 `EXPECT-ERROR-COUNT: N` 指令验证错误数量
  - 新增 2 个 fixtures + 5 个单元测试覆盖

### T0221 [DONE] AST：lambda 表达式节点（spec §12 / Appendix B.5）
- 描述：为表达式新增 `Expr::Lambda`（参数列表 + body），并支持参数可选类型注解。
- 目标：先不实现捕获分析/闭包 lowering；仅 AST 结构。
- 验收：`cargo test -p scoopc` 通过；新增一个 parse fixture（配合 T0222）可在 AST 看到 lambda。
- 依赖：T0205

### T0222 [DONE] Parser：lambda 表达式解析（`{ x -> expr }` / `{ expr }`）
- 描述：在表达式解析中识别 `{ ... }` 为 lambda，并解析 `->` 前参数、`->` 后 body（可用 block 语句列表）。
- 目标：先只支持：0~N 参数、单表达式 body 或 block body；不做类型推断。
- 验收：新增 parse fixture：`val f = { x: Any -> x }` 与 `list.map { it }`（只要能解析）；`scoop test` 通过。
- 依赖：T0221、T0207

### T0223 [DONE] AST：struct literal 表达式节点（spec §12）
- 描述：新增 `Expr::StructLit`（类型名 + 字段初始化列表），字段项包含 `name: expr`。
- 目标：先不支持字段省略写法；只支持显式 `name: expr`。
- 验收：`cargo test -p scoopc` 通过；新增 parse fixture（配合 T0224）可在 AST 看到 struct literal。
- 依赖：T0205

### T0224 [DONE] Parser：struct literal 解析（`Point { x: 1, y: 2 }`）
- 描述：在 postfix/primary 解析中支持 `TypeName { field: expr, ... }` 形式。
- 目标：先只支持 `TypePath` + `{}`；不支持泛型推断/构造函数重载。
- 验收：新增 parse fixture：`val p = Point { x: 1, y: 2 }`；字段缺少冒号时报 parse error。
- 依赖：T0223、T0216

### T0225 [DONE] Parser：消解 `{}` 歧义（struct literal vs lambda）（spec §12）
- 描述：当遇到 `{ ... }` 时，根据是否出现 `->` 或 `name: expr` 结构决定解析为 lambda 或 struct literal。
- 目标：歧义消解只发生在 parser；不把“不确定”留给后续阶段。
- 验收：新增 parse fixture：`Point { x: 1 }` 解析为 struct lit；`list.map { it }` 解析为 lambda；两者都能通过。
- 依赖：T0222、T0224

### T0226 [DONE] Parser：语句 `return`（spec §7.1/§7.3）
- 描述：在 block 语句中支持 `return` 与 `return expr`。
- 目标：先不支持 label/non-local return（后续在 inline 语义里处理）。
- 验收：新增 parse fixture：`fun f(): Any { return x }`；`return` 在顶层报错（或解析后由 typecheck 报错，需明确策略）。
- 依赖：T0207

### T0227 [DONE] Parser：赋值语句/表达式（`lhs = rhs`）
- 描述：支持 `x = expr` 与 `a.b = expr`（lhs 先限 ident/member）。
- 目标：先不支持复合赋值（`+=` 等）；不支持解构赋值。
- 验收：新增 parse fixture：`fun f(){ var x: Any = a; x = b }`；以及 `p.x = 1`（语法层先允许）。
- 依赖：T0210、T0207

### T0228 [DONE] Parser：循环语句（`while`/`break`/`continue`）（PLAN §4.6）
- 描述：解析 `while (cond) { ... }` 与 `break/continue`。
- 目标：先不支持 `for`；先不支持带 label 的 break/continue。
- 验收：新增 parse fixture：最小 while；`break` 在非循环内的错误策略明确（parse 或 typecheck）。
- 依赖：T0214、T0207

### T0229 [DONE] Parser：safe-call `?.`（Appendix B.3.1）
- 描述：把 `x?.member` 与 `x?.call()` 解析为专用 AST 节点（或在 AST 中标记为 safe-call）。
- 目标：先只做语法；desugar 规则交给 lowering/typecheck。
- 验收：新增 parse fixture：`val y = x?.foo(1)` 与 `val z = x?.bar`；AST 中能区分 `.` 与 `?.`。
- 依赖：T0210

### T0230 [DONE] Parser：函数参数默认值（Appendix B.5.2）
- 描述：在参数声明中支持 `param: T = expr`。
- 目标：先只解析并保存 Expr；不实现默认值应用规则。
- 验收：新增 parse fixture：`fun f(x: Any = default()) {}`；默认值缺失表达式时报错。
- 依赖：T0205

### T0231 [DONE] Parser：命名参数调用（Appendix B.5.3）
- 描述：在调用实参中支持 `name = expr` 作为命名参数（只在 call-arg 位置生效）。
- 目标：先只做解析；不做”命名参数重排/默认值补齐”。
- 验收：新增 parse fixture：`f(x = 1, y = 2)`；并确保不与赋值表达式混淆（仅在参数列表内）。
- 依赖：T0209、T0227

### T0232 [DONE] Parser：trailing lambda（Appendix B.5.4）
- 描述：支持 `f(a, b) { ... }` 形式，把尾随 lambda 作为最后一个实参。
- 目标：先不支持同时存在多个尾随 lambda。
- 验收：新增 parse fixture：`list.map { it }`；`f(1) { x -> x }`；AST args 最后一个为 lambda。
- 依赖：T0222、T0209

### T0233 [DONE] Parser：扩展函数 receiver（spec §7.4）
- 描述：支持 `fun T.name(...)` 与 `fun T.() -> R` 这类 receiver 语法（声明处）。
- 目标：先只解析 receiver TypeRef；不做分发规则。
- 验收：新增 parse fixture：`fun Any.id(): Any { this }`（按语言关键字）；AST 中 receiver 可见。
- 依赖：T0218

### T0234 [DONE] AST+Parser：属性声明与 accessors（spec §10.1）
- 描述：在 type body 中支持 `val/var name: T`，以及可选 `get()`/`set(value)` accessor（body 可为 block 或 `= expr`）。
- 目标：先只解析 class 的属性；value type 的限制交给 typecheck。
- 验收：新增 parse fixture：最小 property + getter；property 缺少类型时的错误策略明确。
- 依赖：T0201、T0207

### T0235 [DONE] Parser：delegated property `by`（spec §10.4）
- 描述：在属性声明中支持 `by expr`，并在 AST 中区分”普通属性 vs 委托属性”。
- 目标：先只支持 `val/var name: T by expr`；不实现标准库 delegates。
- 验收：新增 parse fixture：`val x: Any by lazy { ... }`（语法层）；`by` 后缺表达式时报错。
- 依赖：T0234、T0222

### T0236 [DONE] AST+Parser：rich enum 的 variant 声明（spec §2.3.2）
- 描述：为 enum body 解析 variant 列表（支持 `Variant` 与 `Variant(val x: T, ...)`）。
- 目标：先不支持 enum 内方法/属性；先只解析 variant 声明。
- 验收：新增 parse fixture：`enum Option<T> { Some(val value: T), None }`；variant 参数缺类型时报错。
- 依赖：T0201、T0218

### T0237 [DONE] AST：Pattern 节点（spec §4.2）
- 描述：引入 `Pattern` AST（Wildcard/Literal/Bind/Tuple/Variant/Struct 等），供 when 与 destructuring 复用。
- 目标：先只建结构；解析与 typecheck 分开做。
- 验收：`cargo test -p scoopc` 通过；新增单测构造几个 Pattern 确认 span/字段。
- 依赖：T0205

### T0238 [DONE] Parser：pattern v0（wildcard + literal）（spec §4.2）
- 描述：扩展 `when` 分支头解析为 pattern，先支持 `_` 与字面量（数字/字符串/bool）。
- 目标：不支持 or-pattern、guard、解构；先覆盖最小分支。
- 验收：新增 parse fixture：`when (x) { 0 -> 1; _ -> 2 }`；AST 中分支头为 Pattern。
- 依赖：T0237、T0215
- 完成：`WhenArm.pattern` 从 `WhenPattern` 迁移到 `Pattern`；`parse_when_pattern` 返回 `Pattern`，支持 wildcard `_`、int/string/bool 字面量、`is`/`!is` Type、`else`、裸标识符 bind；删除旧 `WhenPattern` 枚举；2 个 pass fixtures + 6 个 unit tests 覆盖

### T0239 [DONE] Parser：pattern v1（tuple pattern）（spec §4.2）
- 描述：支持 `(p1, p2, ..)` 形式的 tuple pattern。
- 目标：先不支持 rest `..`；仅支持定长 tuple。
- 验收：新增 parse fixture：`when (pair) { (0, _) -> 1; else -> 0 }`。
- 依赖：T0238

### T0240 [DONE] Parser：pattern v2（enum variant pattern）（spec §4）
- 描述：支持 `Some(x)`/`None` 这种 variant pattern（无 `is` 前缀）。
- 目标：先只支持位置参数；struct-like variant（若未来支持）后置。
- 验收：新增 parse fixture：`when (opt) { Some(x) -> x; None -> 0 }`。
- 依赖：T0238、T0236
- 实现：`parse_when_pattern` 在裸标识符后检测 `(`，若匹配则调用 `parse_variant_pattern` 解析 `Name(p1, p2, ...)` 为 `Pattern::Variant`；裸标识符（如 `None`）仍保持为 `Bind`（消歧留给后续 resolve 阶段）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖

### T0241 [DONE] Parser：pattern v3（struct pattern）（spec §4.2）
- 描述：支持 `Point { x, y }` 与 `Point { x: px, y: py }`（重命名）。
- 目标：先不支持 rest `..`；嵌套 pattern 已支持（字段值可为任意 pattern）。
- 验收：新增 parse fixture：`when (p) { Point { x, y } -> x }`；字段列表语法错误可诊断。
- 依赖：T0238、T0201
- 实现：`parse_when_pattern` 中标识符后 peek `{` 调用 `parse_struct_pattern()`；解析 `Name { field, field: pattern, ... }` 为 `Pattern::Struct`；支持 shorthand（`x`）、rename（`x: pattern`）、空 struct（`Unit {}`）、尾随逗号、嵌套 pattern（`first: Some(x)`）；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖

### T0242 [DONE] Parser：pattern v4（or-pattern `A | B`）（spec §4.2）
- 描述：支持 `North | South -> ...` 这种 or-pattern。
- 目标：先只支持同层 `|`（左结合或右结合需固定）。
- 验收：新增 parse fixture：`when (dir) { North | South -> 1; else -> 0 }`。
- 依赖：T0238

### T0243 [DONE] Parser：pattern v5（guard `if <expr>`）（spec §4 / tuple 示例）
- 描述：支持 `pattern if cond -> expr` 的 guard 子句。
- 目标：先只支持单个 guard；cond 复用现有 expr 解析。
- 验收：新增 parse fixture：`(x, y) if x == y -> ...`（按语法）；缺少 guard 条件时报错。
- 依赖：T0242、T0211
- 实现：`parse_when_arm` 在 pattern 与 `->` 之间检测 `if` 关键字，解析 guard 表达式并包装为 `Pattern::Guard`；`looks_like_tuple_pattern_ahead` 更新为同时接受 `->` 和 `if` 作为 tuple pattern 判定条件；1 个 pass + 1 个 fail fixture + 6 个 unit tests 覆盖

### T0244 [DONE] Parser：`val` destructuring（tuple/struct pattern）（spec §4.2、§9）
- 描述：支持 `val (a, b) = expr` 与 `val Point { x, y } = expr`。
- 目标：明确限制：`var` 不支持 destructuring（按 spec）。
- 验收：新增 parse fixture：两个 destructuring 例子；新增 parse-fail fixture：`var (a, b) = ...`。
- 依赖：T0237、T0208
- 完成：`ValDecl` 引入 `ValBinding::{Name, Pattern}`；新增 `Pattern` AST（Wildcard/Bind/Tuple/Struct）；block 内解析 `val` 的 tuple/struct 解构并要求 initializer；`var (...)` 给出稳定 `scoop::parse::expected` 并避免级联错误；1 个 pass + 1 个 fail fixture 覆盖

### T0245 [DONE] Parser：修饰符与可见性（public/internal/private/open/abstract/sealed/inline/override）
- 描述：在顶层声明与类型成员上解析修饰符列表，并在 AST 中保存（顺序无关）。
- 目标：先只解析并存储；合法性（如 `override` 只能用于 member）交给 resolve/typecheck。
- 验收：新增 parse fixture：带多修饰符的 class/fun/property；`scoop dump-ast` 可看到 modifiers。
- 依赖：T0009
- 完成：lexer 新增 5 个 modifier 关键字（`public/internal/private/inline/override`）；AST 引入 `Modifier` 并在 `TypeDecl/FunDecl/ValDecl/PropertyDecl` 保存 `modifiers`（空时不影响既有 AST snapshot）；parser 在顶层与 type body 内统一解析 modifiers（排序去重，顺序无关）；新增 parse fixture：`tests/fixtures/parse/modifiers_basic.scoop`

### T0246 [DONE] Parser：`const fun` / `comptime` / splice 语法（spec §6）
- 描述：支持 `const fun`、`comptime { ... }`、`comptime if/for`、以及 splice `value.[field]`（用于在 comptime 中通过 FieldMeta 访问值的字段）。
- 目标：先只做语法与 AST 表达；执行语义留给 T12。
- 验收：新增 parse fixture：包含 const fun 与 comptime block；解析成功并保留 span/节点。
- 依赖：T0207
- 完成：lexer 新增关键字 `const`/`comptime`/`for`/`in`；AST 新增 `Modifier::Const`、`StmtKind::ComptimeBlock/ComptimeIf/ComptimeFor` 与 `ExprKind::SpliceField`；parser 支持解析 `comptime { ... }`、`comptime if/for`（含 `else comptime if` 链）与 splice；新增 parse fixture `tests/fixtures/parse/comptime_syntax_basic.scoop`（含 AST golden）；新增单测 `parser::tests::parse_comptime_syntax_and_splice`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0247 [DONE] Parser：`annotation class` 声明语法（spec §15.2）
- 描述：支持 `annotation class Name(...)`（可视为 class + annotation modifier）。
- 目标：先只解析；target/retention 等后续。
- 验收：新增 parse fixture：声明注解并使用（使用部分依赖 T1001）；解析成功。
- 依赖：T0245
- 完成：lexer 新增关键字 `annotation`；AST 新增 `Modifier::Annotation`；parser 将 `annotation` 作为 modifier 解析并允许 `annotation class`；新增 parse fixture `tests/fixtures/parse/annotation_class_basic.scoop`（含 AST golden；注解使用语法将由 T1001 进一步接入到 AST）

### T0248 [DONE] Parser：class/interface 的继承列表与主构造头（简化版）（spec §2.2 / Appendix B.2）
- 描述：支持 `class Dog(name: String) : Animal(name), IFoo` 的最小语法：构造参数列表 + `:` 后基类/接口列表。
- 目标：先不解析基类构造调用参数（可只保留 span）；不解析 supertype 泛型实参（可先保留 TypeRef）。
- 验收：新增 parse fixture：class 继承与实现接口；缺少 `:` 但有 supertype 时 parse-fail。
- 依赖：T0218、T0201
- 完成：AST `TypeDecl` 新增 `primary_ctor` 与 `supertypes`；parser 在 type decl header 解析主构造参数列表与 `:` 后 supertype 列表（基类构造调用参数仅保留括号 span）；新增 `type_inheritance_basic` pass + `type_inheritance_missing_colon_fail` fail fixtures；更新相关 AST goldens；`cargo test --all` 与 `scoop test` 通过。

### T0249 [DONE] Parser：star projection `*` 与变型 `in/out` 的语法支持（spec §3.2~§3.3）
- 描述：在类型实参位置支持 `*`；在 type param 声明位置支持 `in T`/`out T`。
- 目标：先只解析并存储；合法性检查交给 typecheck。
- 验收：新增 parse fixture：`List<*>`、`interface ReadOnlyProperty<in T, out V>`；解析成功。
- 依赖：T0218
- 完成：lexer 新增关键字 `out`；AST 新增 `TypeParamVariance` 与 `TypeParam.variance`、`TypeRef::Star`；parser 支持 type param 列表内的 `in/out` 与 type args 内的 `*`；新增 parse fixture `tests/fixtures/parse/type_args_star_and_variance.scoop`（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0250 [DONE] Parser：effect row 参数 `eff`（spec §3.4 / §5.8）
- 描述：在泛型参数列表中支持 `eff E = Pure`，并在函数/函数类型上使用 `/ E`。
- 目标：先只解析 `eff` 参数；复杂 row 约束后续。
- 验收：新增 parse fixture：`fun <eff E = Pure> f(): Int / E { ... }`（body 可省略）；AST 中能看到 eff 参数。
- 依赖：T0249、T0219
- 完成：AST 新增 `EffectRowParam`，并在 `FunDecl`/`TypeDecl` 上挂载 `eff_param`；`FunDecl` 新增 `effects: Option<EffectRowExpr>`；parser 支持 `<eff E (= RowExpr)?>` 并解析函数签名的 `/ RowExpr`；函数泛型列表支持 `fun <...> name` 与 `fun name<...>` 两种位置（兼容历史 fixtures）；新增 parse fixture `tests/fixtures/parse/effect_row_param_basic.scoop`（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0251 [DONE] Parser：`typealias` 声明语法（为 sysroot 标准别名铺路）（Appendix B.10）
- 描述：支持顶层 `typealias Name = Type` 的解析，并把它纳入 AST（含 span）。
- 目标：先只支持“非泛型 typealias”；先只允许顶层；不做循环检测与展开（留给 resolver/typecheck）。
- 验收：新增 parse fixture：`typealias Byte = UInt8`、`typealias UIntPtr = UInt` 可解析；错误形式（缺 `=`/缺类型）给出稳定错误码。
- 依赖：T0009
- 完成：lexer 新增关键字 `typealias`；AST 新增 `TypeAliasDecl` 与 `Item::TypeAlias`；parser 支持解析顶层 `typealias Name = Type`；resolve 的 `Index`/绑定检查将 typealias 视为 type-level symbol 并解析 RHS `TypeRef`；新增 parse fixtures `typealias_basic`（含 AST golden）与两个 fail fixtures；新增单测 `parser::tests::parse_top_level_typealias`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0252 [DONE] Parser：prefix 一元运算（`!`/`-`/`~`）（spec §2.3.4 / Appendix B.8）
- 描述：在表达式语法中加入 prefix unary：逻辑非 `!`、数值取负 `-`、按位取反 `~`。
- 目标：先只实现语法与 AST 节点；优先级规则固定为“高于二元运算、低于 postfix（调用/成员访问/`!!`）”。
- 验收：新增 parse fixture：`val x = ~a`、`val y = -(1 + 2)`、`val z = !flag`；AST 结构符合预期。
- 依赖：T0206、T0209、T0210
- 完成：AST 新增 `UnaryOp` 与 `ExprKind::Unary`；parser 引入 `try_parse_expr_prefix()`，支持 `!`/`-`/`~` 并确保 postfix 优先级更高；新增 parse fixture `tests/fixtures/parse/prefix_unary_basic.scoop`（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0253 [DONE] Parser：use-site effect row 实参 `Type<eff Row>`（spec §3.4 / §5.8）
- 描述：在类型实参列表中支持 `Disposable<eff Async>`、`Disposable<eff (Async + Raise<IOError>)>` 这类 use-site effect row 参数写法。
- 目标：先只支持单个 `eff` clause，且必须出现在泛型实参列表最后；合法性检查留给 typecheck。
- 验收：新增 parse fixture：`Disposable<eff Pure>`、`Disposable<eff (Async + Raise<IOError>)>` 可解析；`<eff E, Int>` 之类非法顺序报错。
- 依赖：T0250、T0219
 - 完成：AST 新增 `TypeRef::EffectRowArg`；parser 在类型实参列表 `<...>` 内把 `eff` 作为上下文关键字解析为 row expr，并强制其必须位于列表末尾；resolve/typecheck 对该节点做最小兼容处理；新增 parse pass+fail fixtures（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0254 [DONE] Parser：import alias `import foo.bar.Baz as Qux`（Appendix B.7）
- 描述：扩展 import 语法，支持 Kotlin 风格 alias import，并把 alias 名记录到 AST。
- 目标：先只支持顶层 import；不支持在表达式/局部作用域中出现 import。
- 验收：新增 parse fixture：普通 import、`*` import、alias import 混用可解析；缺 alias 名时报错。
- 依赖：T0009
- 完成：AST `ImportDecl` 新增 `alias: Option<Ident>`；parser 在 import 末尾解析 `as <Ident>` 并更新 span；resolver/typecheck 的 import 规则与 `ImportTable` 构建使用 alias 作为 local 名；新增 parse pass+fail fixtures（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0255 [DONE] Parser：pattern rest `..`（spec §4.2）
- 描述：在 pattern 语法中支持 `..`，用于忽略剩余字段/元素。
- 目标：先只把 `..` 解析进 AST；类型规则与“只能出现一次”等约束留给 typecheck。
- 验收：新增 parse fixture：tuple/struct pattern 中的 `..` 可解析；重复 `..` 或非法位置报错。
- 依赖：T0239、T0241、T0240
- 完成：AST `PatternKind` 新增 `Rest`，struct pattern 记录 `rest: Option<Span>`；parser 支持 tuple/struct pattern 中的 `..` 并在重复/非法位置时给出稳定 `scoop::parse::expected`；新增 parse pass+fail fixtures（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0256 [DONE] Parser：class `init { ... }` blocks（Appendix B.2.2）
- 描述：在 class body 中解析 `init { ... }` 初始化块，并把它作为成员节点纳入 AST。
- 目标：先只支持 class；初始化顺序与语义留给 resolver/typecheck。
- 验收：新增 parse fixture：含多个 `init` block 的 class 可解析；`init` 缺 block 报错。
- 依赖：T0207、T0201、T0248
- 完成：AST `TypeMember` 新增 `InitBlock(InitBlockDecl)`；parser 在 class type body 识别上下文关键字 `init` 并解析其 `{ ... }` 为 `Block`；`is_type_member_start` 将 `init` 视为 member 起始以避免 property initializer 吞掉后续 init；新增 parse pass fixture `class_init_blocks_basic`（含 AST golden）与 fail fixture `class_init_missing_block_fail`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0257 [DONE] Parser：secondary constructors（Appendix B.2.2）
- 描述：支持 class 内 `constructor(...) { ... }` 的最小语法，并保留可选 delegation call（如 `: this(...)` / `: super(...)`）的 span/AST。
- 目标：先只解析签名、delegation 头和 body；初始化顺序与调用合法性留给 typecheck。
- 验收：新增 parse fixture：含 secondary constructor 的 class 可解析；缺参数列表或缺 body 报错。
- 依赖：T0248、T0207
- 完成：AST 新增 `SecondaryCtorDecl`/`CtorDelegationCall` 与 `TypeMember::SecondaryCtor`；parser 在 class body 识别上下文关键字 `constructor` 并解析参数列表、可选 `: this(...)`/`: super(...)` delegation call（仅保留括号 span）、以及必需的 `{ ... }` body；新增 parse pass fixture `class_secondary_ctor_basic`（含 AST golden）与两个 fail fixtures（缺参数列表/缺 body）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0258 [DONE] Parser：`object` / `companion object` 声明（Appendix B.9）
- 描述：支持 top-level / nested `object Name { ... }`，以及 class 内 `companion object { ... }` / `companion object Name { ... }`。
- 目标：先只做语法与 AST；单例语义、成员访问和初始化留给后续阶段。
- 验收：新增 parse fixture：top-level object、nested object、named/unnamed companion object 可解析；非法 companion 位置报错。
- 依赖：T0201、T0248
- 完成：lexer 新增 `object`/`companion` 关键字；AST 新增 `ObjectDecl/ObjectKind` 并在 `Item/TypeMember` 接入；parser 支持 top-level/nested object 与 class body 内 companion object（非法位置给出稳定 `scoop::parse::expected` 且避免级联错误）；新增 parse pass fixture `object_and_companion_object_basic`（含 AST golden）与 fail fixture `companion_object_illegal_position_fail`；`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test` 通过。

### T0259 [DONE] Parser：receiver function type 语法 `T.(A, B) -> C / R`（spec §7.5）
- 描述：在类型语法中补齐 receiver function type：支持 `T.() -> R`、`T.(A, B) -> C` 以及带 effect row 的 `T.(A, B) -> C / E`。
- 目标：先只做语法与 AST 建模；子类型、推断、codegen 继续复用/依赖 T0435 等后续任务。
- 验收：新增 parse fixture：`val f: String.() -> Int`、`val g: List<Int>.(Int) -> Bool / Pure` 可解析；非法写法（如缺 `->` 或 receiver 后不是函数类型）报错。
- 依赖：T0219、T0233
- 完成：parser `TypeRef` 解析已支持 receiver function type（`T.(...) -> ... / E`）；新增 parse pass fixture `receiver_function_type_basic`（含 AST golden）与 fail fixture `receiver_function_type_missing_arrow_fail`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0260 [DONE] Parser：泛型 `where` 子句（spec §3 / Appendix B）
- 描述：为 `fun` / `class` / `struct` / `interface` / `enum` / `effect` 的声明头补齐 `where` 子句语法，支持在声明处泛型参数列表之后附加约束列表。
- 目标：先固定 AST 形状与解析顺序；约束语义、约束满足性与冲突诊断交给 resolve/typecheck。
- 验收：新增 parse fixtures：`fun <T> f(x: T): T where T: Show`、`class Box<T> where T: Clone` 可解析；语法损坏时给出稳定 parse error。
- 依赖：T0218、T0249
- 完成：lexer 新增 `where` 关键字；AST 新增 `WhereClause/WhereConstraint` 并在 `FunDecl/TypeDecl` 上记录；parser 在 fun/type 声明头解析 `where T: Bound(, ...)` 约束列表（允许与 effect `/ ...` 交换顺序）；新增 parse pass+fail fixtures（含 AST golden）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

---

## T03：名字解析（阶段 2：从“顶层存在性”走向“作用域/多命名空间”）

### T0301 [DONE] Resolver：区分 type/value 命名空间（但共享同一套 FQN）
- 描述：把 `Index` 从 “by_fqn → Symbol” 扩展为按 namespace 分类（type/value/fun）。
- 目标：仍先只覆盖顶层；不实现重载解析。
- 验收：新增 resolve fixture：同名 type 与 fun 是否允许（按设计决定）；冲突规则有诊断。
- 依赖：T0011
- 完成：`scoopc::resolve::Index` 的 `by_fqn` 升级为 `FQN → NamespacedSymbols(type/fun/value)`，允许同一 FQN 下 type 与 fun/value 并存；同命名空间内重复定义仍报 `scoop::resolve::duplicate_definition`；新增 resolve fixture `tests/fixtures/resolve/type_and_fun_same_name_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0302 [DONE] Resolver：把 type body 的成员也纳入索引（成员级符号）
- 描述：基于 T0201 的 AST，把成员（fields/methods/nested types）加入索引并检测重复。
- 目标：先只检测同一类型体内重复；不做继承/override。
- 验收：新增 resolve fixture：类内重复字段/方法名能报错并给出两个 span label。
- 依赖：T0201、T0301
- 完成：`scoopc::resolve::Index` 递归把类型体成员（property/fun/nested type）纳入索引并复用 `duplicate_definition` 诊断；新增 resolve fixture `tests/fixtures/resolve/duplicate_member_definition_in_type_body.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0303 [DONE] Resolver：import 解析扩展为“类型与值”两套规则
- 描述：当前 import 只用于 type path；扩展为后续表达式里的值解析准备。
- 目标：先只建 import 表（显式/星号）；不立刻应用到 expr。
- 验收：新增单测：构造 import 表并序列化/debug 输出可见；现有 resolve fixtures 不回归。
- 依赖：T0301
- 完成：新增 `scoopc::resolve::ImportTable`，在构建时把显式 import 按 type/value（fun/value）命名空间拆分，并保留 `*` import 前缀列表；`check_file_bindings` 改为构建 ImportTable 以统一验证 import 存在性；新增单测覆盖 type/value 分流与 debug 输出；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0304 [DONE] Resolver：块级作用域（val/var）符号表（spec §9）
- 描述：在 resolve 阶段为 block 建立 scope 栈，记录局部变量定义与遮蔽规则。
- 目标：先只支持 block 内 `val/var`；不做捕获/闭包。
- 验收：新增 resolve fixture：引用未定义局部变量报错；同名遮蔽按规则允许/禁止（需决定）。
- 依赖：T0207、T0301
- 完成：在 `scoopc::resolve` 新增块级作用域检查（局部 `val/var` + 参数；嵌套块允许遮蔽），并对 `ExprKind::Ident` 做最小 value 名字存在性解析（先查局部 scope，再查同包/导入的顶层 fun/value）；新增 resolve fixtures：`unresolved_value_in_block`（fail）与 `local_shadowing_ok`（pass）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0305 [DONE] Resolver：表达式中的标识符引用解析（到变量/参数/顶层）
- 描述：对 `Expr::Ident` 绑定到某个 SymbolId（或暂用字符串），并在 AST/HIR 中记录解析结果。
- 目标：先不解析成员访问（`.`）的具体 target；先解析裸 ident。
- 验收：新增 resolve fixture：函数参数引用 OK；未定义 ident 报错并精确指到 ident span。
- 依赖：T0304
- 完成：AST 将 `ExprKind::Ident` 升级为 `ValueIdent { span, resolved }` 并引入 `ResolvedValueRef(Local/TopLevel)`；parser 构造未解析 `ValueIdent`；resolver 在块级作用域检查中把裸 ident 解析到“局部绑定（参数/val）或顶层符号（FQN）”并写回；新增 resolve fixtures `param_reference_ok`（pass）与 `unresolved_value_ident_span`（fail）；新增单测 `resolve::scopes::tests::value_ident_resolution_is_written_back`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0306 [DONE] Resolver：可见性修饰符（public/internal/private）（PLAN §3.1）
- 描述：在 AST/解析层引入 visibility，并在 resolve 阶段做最小合法性检查。
- 目标：先只实现语法 + 文件内可见性检查；跨包规则后续补。
- 验收：新增 resolve fixture：private 顶层符号在其他文件引用时报错（需要多文件 fixture 支持或单测构造多文件）。
- 依赖：T0301、T0245
- 完成：`scoopc::resolve::Index` 的 `Symbol` 记录 `Visibility(public/internal/private)` 与 `decl_file`；resolve 阶段对 type/value（含 fun）解析增加可见性过滤，并在跨文件引用 `private` 符号时返回 `scoop::resolve::not_visible`；新增单测覆盖“private 顶层符号在其他文件引用时报错”与“非法可见性修饰符组合报错”；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0307 [DONE] Session/fixtures：支持“多文件编译单元”的 resolve/typecheck
- 描述：允许一个 fixture case 包含多个 `.scoop` 文件，并作为同一编译单元构建 index/resolve/typecheck。
- 目标：先只用于 resolve/typecheck；run-pass 后续可复用同一机制。
- 验收：新增 `tests/fixtures/resolve_multi/<case>/`（目录内 2+ 文件）并跑通；`scoop test` 能按“目录作为单元”执行。
- 依赖：T0003、T0011
- 完成：fixtures runner 新增 `resolve_multi/<case>/` 支持（按 case 目录聚合多文件，构建单一 `Index` 后逐文件 resolve 并断言）；新增多文件用例 `tests/fixtures/resolve_multi/cross_file_type_ref/`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0308 [DONE] Resolver：两阶段解析（先收集声明头，再解析 body/init）
- 描述：把当前“一次性检查”拆为：collect headers → build symbol tables → resolve bodies（支持 forward reference 的明确规则）。
- 目标：第一步只做结构拆分与数据流；不改变现有错误码太多。
- 验收：新增 resolve fixture：函数体里引用同文件后定义的顶层符号（是否允许按设计）；resolver 行为稳定且有诊断。
- 依赖：T0307
- 完成：将 `check_file_bindings` 拆分为 `check_file_headers`（构建 import 表 + 解析声明头里的 TypeRef）与 `check_file_bodies`（块级作用域 + 值解析）；新增 `FileHeaders` 作为 phase 间数据载体；type decl headers（主构造参数类型、supertype、成员签名）也纳入 type 引用解析；将值解析扩展到顶层 `val/var` initializer；新增 resolve fixtures：`forward_ref_top_level_symbol_ok`（pass）与 `unresolved_value_in_top_level_init`（fail）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0309 [DONE] Resolver：泛型参数作用域与解析（type params 是符号）
- 描述：在 resolve 阶段把声明处 type params 纳入作用域，使 `TypeRef` 中的 `T` 可解析到泛型参数。
- 目标：先只支持同一声明内引用；不支持 where 约束。
- 验收：resolve fixture：`fun id<T>(x: T): T { x }` 通过；`fun f(x: T) {}` 报未定义类型参数。
- 依赖：T0218、T0308
- 完成：`scoopc::resolve` 引入声明级 `TypeParamScopes` 并在 `check_file_headers`/`resolve_type_decl_headers` 推入/弹出 type params；`resolve_type_path` 对单段路径优先命中当前作用域的 type param；新增 resolve fixtures：`tests/fixtures/resolve/generic_type_param_ok.scoop`（pass）与 `tests/fixtures/resolve/unresolved_type_param_in_signature.scoop`（fail）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0310 [DONE] Resolver：成员访问解析（`.`）绑定到字段/方法/属性
- 描述：把 `a.b` 的 `b` 绑定到 struct 字段或 class/interface 成员（先做存在性）。
- 目标：先只处理“静态可确定”的情况；动态分发/override 后续。
- 验收：resolve fixture：`p.x` 解析到字段；`p.m()` 解析到方法；不存在时报错并指向成员名 span。
- 依赖：T0302、T0210
- 完成：在 `scoopc::resolve::scopes` 中实现 `resolve_member_access`，基于 receiver 的“静态可确定类型”（局部带类型注解 / `this` / `StructLit`）将 `receiver.member` 解析到成员符号并写回 `MemberIdent.resolved`（fun/value 命名空间）；新增 resolve fixtures：`tests/fixtures/resolve/member_access_ok.scoop`、`tests/fixtures/resolve/unresolved_member_access.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0311 [DONE] Resolver：调用解析（把 `Call(Ident)` 绑定到具体函数）
- 描述：把 `f(...)` 的 callee 从“裸 ident”解析为某个 fun symbol（先要求唯一匹配）。
- 目标：先不支持重载；若同名多个定义则报歧义错误。
- 验收：resolve fixture：调用顶层函数成功；同名多个函数时报 `ambiguous_call`（新错误码）。
- 依赖：T0305、T0209
- 完成：在 `scoopc::resolve::scopes` 中为 `ExprKind::Call` 的裸标识符 callee 增加顶层 fun 命名空间解析：唯一匹配写回 `ValueIdent.resolved = TopLevel { fqn }`，多候选时报 `scoop::resolve::ambiguous_call`；新增/补齐 resolve fixtures `tests/fixtures/resolve/call_top_level_fun_ok.scoop` 与 `tests/fixtures/resolve_multi/ambiguous_call/`，并加入单测覆盖；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0312 [DONE] Resolver：扩展函数/扩展属性的分发优先级（spec §7.4 / §10.3）
- 描述：实现最小规则：member 优先于 extension；extension 需要 receiver 类型可匹配。
- 目标：先只在同包/同文件找 extension；跨包 import 扩展后续。
- 验收：resolve fixture：同名 member 与 extension 并存时解析到 member；只有 extension 时解析到 extension。
- 依赖：T0233、T0310
- 完成：`scoopc::resolve::Index` 收集同包扩展函数并记录 receiver 类型 FQN；`ScopeChecker::resolve_member_access` 实现 member 优先、无 member 时按 receiver 匹配 extension fun；补齐 resolve fixtures `tests/fixtures/resolve/extension_member_prefers_member.scoop` 与 `tests/fixtures/resolve/extension_member_only_extension_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0313 [DONE] Resolver：`this`/构造参数/成员初始化作用域（class 场景）
- 描述：为 class 主构造参数、属性初始化表达式、成员函数体建立正确作用域（含 `this`）。
- 目标：先不实现 `super`；先不处理 capture/闭包。
- 验收：resolve fixture：class 成员函数可引用 `this` 与构造参数；未定义时报错。
- 依赖：T0248、T0308
- 完成：`scoopc::resolve::scopes` 引入 `ThisContext` 栈并在类型体成员/扩展函数体内解析 `this`；member fun/属性 init/accessor 解析时额外注入主构造参数作用域；新增 resolve fixtures `tests/fixtures/resolve/class_member_this_and_ctor_param_ok.scoop`（pass）与 `tests/fixtures/resolve/this_outside_receiver_is_error.scoop`（fail）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0314 [DONE] Resolver：收集 `typealias` 并纳入 type 命名空间（为 sysroot 标准别名铺路）
- 描述：把 `typealias Name = Type` 作为一种 type-level symbol 纳入索引与 import 环境，使得 `Byte/UIntPtr` 等别名能被当作类型引用。
- 目标：resolve 阶段只做“名字可见性/冲突检测”；不做 alias 展开与循环检测（交给 typecheck）。
- 验收：新增 resolve fixture：`typealias Byte = UInt8; fun f(x: Byte): Byte { x }` 可解析；同名 typealias 与 struct/class 冲突时报错并定位两个声明。
- 依赖：T0251、T0301、T0308
- 完成：补齐/更新 resolve fixtures：`typealias_is_type_symbol_ok.scoop`（pass）与 `typealias_conflicts_with_nominal_type_is_error.scoop`（duplicate_definition）；`cargo run -p scoop -- test` 与 `cargo test --all` 通过。

### T0315 [DONE] Resolver：import alias 绑定与冲突规则（Appendix B.7）
- 描述：把 `import foo.bar.Baz as Qux` 引入的 alias 纳入 import table，并参与 type/value 名字解析与冲突检查。
- 目标：先只支持文件级 alias；同名 alias 与本地顶层声明/其他 import 冲突时报错。
- 验收：新增 resolve fixture：通过 alias 成功引用类型/函数；alias 冲突时报稳定错误码。
- 依赖：T0254、T0303、T0308
- 完成：`ImportTable::build` 增加 alias 冲突检查（与顶层声明/其它 import）；新增 resolve fixtures（alias ok + alias conflict）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0316 [DONE] Resolver：class 初始化阶段作用域（property initializer / `init` / secondary constructor）
- 描述：为属性初始化表达式、`init` block、secondary constructor 建立正确作用域，固定 `this`、主构造参数、已声明成员的可见性边界。
- 目标：先只做名字解析与作用域；初始化顺序与必经 delegation 规则留给 typecheck。
- 验收：新增 resolve fixture：`init` 中可引用 `this` 与构造参数；非法前向引用报错并定位。
- 依赖：T0256、T0257、T0313
- 完成：`scoopc::resolve::scopes` 补齐 class `init { ... }` 与 secondary constructor 的值名字解析（`this` + 主构造参数 + 次构造参数）；在 property initializer / init block 中引入初始化阶段“可见 value members”约束并新增诊断 `scoop::resolve::forward_reference`；新增 resolve fixtures `tests/fixtures/resolve/class_init_block_this_and_ctor_param_ok.scoop`、`tests/fixtures/resolve/class_init_block_forward_ref_is_error.scoop`、`tests/fixtures/resolve/class_secondary_ctor_this_and_params_ok.scoop` 与单测覆盖；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0317 [DONE] Resolver：`object` / `companion object` 的名字解析与成员访问（Appendix B.9）
- 描述：把 `object` / `companion object` 纳入符号表，并支持 `Obj.member`、`ClassName.member`（经 companion）这类最小解析规则。
- 目标：先只做 name resolution；单例初始化与 codegen 留给后续。
- 验收：新增 resolve fixture：top-level object 成员可解析；class companion 成员可通过 `ClassName.member` 解析；缺 companion 时给出清晰错误。
- 依赖：T0258、T0302、T0301
- 完成：`Index` 记录 `companion_objects` 与 `object_types`；未命名 `companion object` 使用隐式名 `Companion` 并纳入索引与成员表；member access 解析扩展支持 `Obj.member` 与 `TypeName.member`（经 companion）并在缺 companion 时给出稳定诊断 `scoop::resolve::missing_companion_object`；新增 resolve fixtures（object ok / companion ok / missing companion fail）；`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0318 [DONE] Resolver：overload set 收集（顶层 / 成员 / 扩展 / 构造函数）
- 描述：把当前“同名函数唯一”模型升级为候选集合模型：允许同名可重载函数、成员函数、扩展函数、secondary constructors 共存，并为后续 typecheck 决议保留声明头信息。
- 目标：resolve 阶段只负责“收集候选，不做最终决议”；真正冲突留给签名比较与 typecheck。
- 验收：新增 resolve fixture：两个同名不同参数列表的函数可共存；同名不同参数的 constructors / extensions 可被收集为 overload set。
- 依赖：T0301、T0302、T0248、T0257
- 完成：`scoopc::resolve::Index` 的 fun 命名空间从单一 `Symbol` 升级为 `Vec<FunOverload>`（保留 receiver/params/return/effects 等声明头）；新增 `constructors` overload set（primary + secondary）；`resolve::scopes`/`resolve::imports` 按“任一 overload 可见即匹配”适配；新增 resolve fixtures `tests/fixtures/resolve/overload_*` 与单测覆盖；`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0319 [DONE] Resolver：调用点/构造点候选收集，替代“唯一 callee”假设
- 描述：把 `Call(Ident)`、成员调用、构造调用从“直接绑定到唯一 fun symbol”升级为“绑定到候选集合 + 调用形状（args/names/receiver）”。
- 目标：先不做 most-specific；同一调用点只要存在多个候选就保留到 typecheck，再由后续任务决议。
- 验收：新增 resolve fixture：同名两个函数在调用点保留候选集合而不是提前报歧义；构造调用同理。
- 依赖：T0318、T0311、T0310、T0209
- 完成：在 `scoopc::ast` 为 `ValueIdent/MemberIdent` 增加 `call: Option<ResolvedCall>`，引入 `ResolvedCall/CallCandidate/CallShape/CallArgShape` 用于记录候选集合与调用形状；在 `scoopc::resolve::scopes` 中实现 `resolve_call_site`，为 `Call(Ident)` 收集顶层函数候选与构造候选（不再在多候选时报 `ambiguous_call`，留给后续 typecheck），并为成员调用写回候选与调用形状；更新 resolve multi fixture `tests/fixtures/resolve_multi/ambiguous_call/use.scoop` 为 pass；新增 resolver 单测覆盖“多 fun 候选保留/多 ctor 候选保留”；`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0320 [DONE] Resolver：`where` 子句中的类型参数与约束引用解析
- 描述：把 `where` 子句纳入 resolve 流程：约束左侧的类型参数要绑定到当前声明的泛型参数，约束右侧的类型引用要按现有 type/import 规则解析。
- 目标：先只支持“当前声明上的 type params + 普通 TypeRef”这一层；约束满足性、冲突与循环诊断留给 typecheck。
- 验收：新增 resolve fixture：`fun <T> f(x: T): T where T: Show` 中 `T` 与 `Show` 都能正确解析；未声明的类型参数名或未导入的约束类型报错。
- 依赖：T0260、T0309、T0308
- 完成：`check_file_headers` 阶段补齐 `FunDecl/TypeDecl` 的 `where_clause` 解析：约束左侧校验 type param scope，右侧复用 `TypeRef` 解析；新增稳定诊断 `scoop::resolve::unresolved_type_param`；新增 resolve fixtures `tests/fixtures/resolve/where_clause_*` 覆盖 pass/未声明类型参数/未解析约束类型；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0321 Resolver：跨包可见性规则（`public/internal/private`）（拆分为子任务）
- 描述：在 source package / `.cone` 依赖边界上固定可见性规则：同 cone、跨 cone、下游依赖分别能看到哪些声明，并把诊断统一到稳定错误码。
- 目标：优先把 resolver 的“可见性判定”做成可接入 `.cone` 的形态；不引入 friend module 或自定义可见性层级。
- 验收：见子任务（T0321a/T0321b）。
- 依赖：T0306

### T0321a [DONE] Resolver：引入 cone 边界并实现 internal 跨 cone 不可见（source-only fixtures）
- 描述：在 resolver 的可见性判断里引入“cone（编译包）”边界：`internal` 仅 cone 内可见，`public` 可跨 cone，`private` 文件内可见；并在 fixtures 中新增可模拟依赖 cone 的 runner 与用例。
- 目标：只实现 resolver/fixtures 侧 cone 边界；不实现 `.cone` 归档读取与 API 注入（留给 T1105/T0321b）。
- 验收：新增 `tests/fixtures/resolve_cone/<case>/`（每个 case 含 2 个 cone 子目录），下游 cone 引用上游 cone：
  - `public` 声明可见（pass）
  - `internal/private` 在跨 cone 场景下被拒绝（fail，稳定错误码 `scoop::resolve::not_visible`）
  - 错误信息能指出声明所在包（例如包含 `lib.` 前缀）
  - `cargo test --all` 与 `cargo run -p scoop -- test` 通过
- 依赖：T0306、T0307
- 完成：`scoopc::resolve` 引入 `ConeId` 并在可见性判定中将 `internal` 收敛为“仅 cone 内可见”；新增 `Index::build_with_cones`/`IndexedFile` 以支持多 cone index 构建；`scoop test` 新增 `resolve_cone` runner（按 `<case>/<cone>/` 目录模拟依赖边界）；新增 fixtures `tests/fixtures/resolve_cone/cross_cone_visibility/*` 覆盖 public pass / internal+private fail；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

> 注：T0321b/T0322 依赖 `.cone` 读取（T1105），已移动到 T1105 之后以保持依赖顺序。

---

## T04：类型系统（阶段 3：先类型检查再优化）

### T0401 [DONE] Type 表示：建立 `scoopc::ty` 模块（TypeId/TypeKind）
- 描述：引入内部类型表示（区分引用/值类型），并支持 builtin（Any/Option/Nothing/Unit 以及内建整数族 Int/UInt/IntN/UIntN 等）。
- 目标：先只建数据结构与打印；不做推断/求解。
- 验收：新增单测：构造若干 Type 并格式化输出；`cargo test -p scoopc` 通过。
- 依赖：T0010
- 完成：新增 `crates/scoopc/src/ty/mod.rs`（`TypeId`/`TypeKind`/`TypeStore` + builtin 与 `Display`）；单测覆盖 builtin/组合类型格式化与 ref/value 分类；`cargo test -p scoopc` 通过。

### T0402 [DONE] 从 sysroot 收集“内建类型/效果”的类型信息
- 描述：基于 sysroot AST 建立 type env（Any/Option/Raise），为后续 typecheck 提供起点。
- 目标：先只读取声明头（名字 + kind + 泛型参数个数），不做方法体。
- 验收：新增单测：加载 sysroot 后能查询到 `scoop.core.Option` 的泛型参数数量为 1。
- 依赖：T0010、T0401
- 完成：新增 `scoopc::typecheck::TypeEnv`（`from_sysroot/extend_from_file/type_param_count` 等）并在构建时从 sysroot AST 收集所有类型符号（含 effect）；单测 `typecheck::type_env::tests::sysroot_type_env_contains_option_arity` 覆盖 `scoop.core.Option` 的 arity=1；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0403 [DONE] `TypeRef` → `Type` lowering（支持 Path/Tuple/Nullable）
- 描述：把 AST 的 `TypeRef` 解析到内部类型（已 resolve 的前提下）。
- 目标：先做“存在性 + arity 检查”；不做 variance/star projection。
- 验收：新增 typecheck fixture：`fun f(x: Option<Any>): Any {}` 通过；`Option<Any, Any>` 报 arity 错误（新错误码）。
- 依赖：T0011、T0402
- 完成：实现 `crates/scoopc/src/typecheck/lower.rs` 的 `TypeRef` lowering（`Path`/`Tuple`/`Nullable`）与泛型 arity 检查，新增稳定诊断 `scoop::typecheck::type_arity_mismatch`；新增 typecheck fixtures `tests/fixtures/typecheck/option_any_ok.scoop`、`tests/fixtures/typecheck/option_arity_mismatch.scoop` 回归；`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0404 [DONE] 类型检查 pass：仅检查顶层声明头（fun/val/type）签名合法
- 描述：实现 `typecheck::check_file_headers`，不进入函数体。
- 目标：先把“类型环境 + 错误诊断”跑通；不要求表达式 AST 完整。
- 验收：新增 `tests/fixtures/typecheck/`：至少 2 个 pass + 2 个 fail；在 `scoop test` typecheck phase 下回归。
- 依赖：T0101、T0403
- 完成：新增 `crates/scoopc/src/typecheck/headers.rs` 实现顶层/类型体成员声明头的最小约束（参数/属性/构造参数/顶层 val/var 的类型注解检查，pattern binding 暂报错），并在 `crates/scoop/src/fixtures/mod.rs` 的 typecheck phase 中作为前置检查执行；新增 fixtures `tests/fixtures/typecheck/top_level_val_with_type_ok.scoop`、`tests/fixtures/typecheck/top_level_val_missing_type_is_error.scoop`、`tests/fixtures/typecheck/fun_param_with_type_ok.scoop`、`tests/fixtures/typecheck/fun_param_missing_type_is_error.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0405 [DONE] 表达式类型检查 v0：字面量（Int/String/Bool/Unit）
- 描述：为 `Expr::IntLit/StringLit/...` 推导类型。
- 目标：先把 builtin 类型补到 sysroot（或在 compiler 内建）；不做数值提升。
- 验收：新增 typecheck fixture：`val x = 1` 推导为 Int（若支持推断）；或要求注解 `val x: Int = 1`。
- 依赖：T0206、T0401、T0418
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 中为 `Int`/`String`/`Unit` 字面量与 `true/false`（Bool）推导 builtin 类型，并新增 typecheck fixture `tests/fixtures/typecheck/literals_ok.scoop` 覆盖；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0406 [DONE] 表达式类型检查 v0：变量引用（局部/参数/顶层）
- 描述：对 resolve 后的 ident 引用给出类型。
- 目标：先不支持 forward reference（或明确规则）；错误信息指向引用处。
- 验收：typecheck fixture：`fun f(x: Any) { val y = x }` 通过；`val y = missing` 报未定义符号（若 resolve 已报则这里不重复/或只报一次）。
- 依赖：T0305、T0405
- 完成：typecheck phase 先运行 `resolve::check_file_bodies` 写回 `ValueIdent.resolved`，并在 resolver 中对 `true/false` 做字面量 special-case；`scoopc::typecheck::expr` 增加对 `ExprKind::Ident` 的类型推导（Local/TopLevel），并在函数体内对局部 `val/var` initializer 做最小推导与注册；新增 fixture `tests/fixtures/typecheck/value_ident_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0407 [DONE] 表达式类型检查：函数调用（无重载、按名称唯一解析）
- 描述：对 `Call(callee, args)` 做参数数量检查与类型匹配。
- 目标：先只支持调用“已解析到的 fun symbol”；不支持默认参数/命名参数。
- 验收：typecheck fixture：调用参数个数不匹配时报错（含错误码）；参数类型不匹配时报错并指出 arg span。
- 依赖：T0209、T0305、T0406
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 中实现 `infer_call_expr_type`（仅支持顶层 fun ident 调用；检查 arity 与逐参数类型可赋值）；新增诊断 `scoop::typecheck::call_arity_mismatch` / `scoop::typecheck::call_arg_type_mismatch`；新增 fixtures `tests/fixtures/typecheck/call_ok.scoop`、`tests/fixtures/typecheck/call_arity_mismatch_is_error.scoop`、`tests/fixtures/typecheck/call_arg_type_mismatch_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0408 [DONE] 表达式类型检查：成员访问 `a.b`（仅 struct 字段）
- 描述：先实现 value type `struct` 的字段访问类型检查（spec §2.3.1）。
- 目标：不支持 class/interface vtable；只支持直接字段。
- 验收：新增 typecheck fixture：定义 struct `Point(val x: Int)` 并访问 `p.x` 通过；访问不存在字段报错。
- 依赖：T0210、T0401、T0404
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 实现 `ExprKind::MemberAccess` 的类型推导：依赖 resolver 写回的 `MemberIdent.resolved`（仅支持 `ResolvedMemberRef::Value`）并通过 `collect_struct_field_types`（主构造参数 + type body property）收集字段类型；新增 fixtures `tests/fixtures/typecheck/member_access_struct_field_ok.scoop`、`tests/fixtures/typecheck/member_access_missing_field_is_error.scoop`、`tests/fixtures/typecheck/member_access_non_field_is_error.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0409 [DONE] 声明类型：struct（仅字段，不含方法）
- 描述：typecheck 阶段收集 struct 字段列表、检查重复字段、类型合法。
- 目标：先限制字段全是 `val` 且需要类型注解；不支持默认值。
- 验收：typecheck fixture：struct 字段重复时报错；字段类型未解析时报错。
- 依赖：T0202、T0404
- 完成：实现 `crates/scoopc/src/typecheck/structs.rs` 的 `check_file_struct_decls`（递归处理 nested struct；检查重复字段名；禁止 `var` 字段与默认值）；并在 typecheck fixtures runner（`crates/scoop/src/fixtures/mod.rs`）中作为 typecheck phase 的前置检查执行；新增/补齐 fixtures：`tests/fixtures/typecheck/struct_duplicate_field_is_error.scoop`、`tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop`、`tests/fixtures/typecheck/struct_field_default_value_not_supported_is_error.scoop`、`tests/fixtures/typecheck/struct_field_unresolved_type_is_error.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0410 [DONE] 值类型：tuple 与 Unit（spec §2.3.3）
- 描述：把 tuple 类型与 tuple 表达式加入类型系统；`Unit` 视为 0 元 tuple。
- 目标：先只支持 `(A, B)` 类型与 `(a, b)` 表达式；不支持解构。
- 验收：typecheck fixture：`val t: (Int, Int) = (1, 2)` 通过；元素类型不匹配报错。
- 依赖：T0211、T0405
- 完成：已支持 `()`/`(a, b)` 表达式与 `(A, B)`/`()` 类型注解；空 tuple 统一映射到 builtin `Unit`；新增 fixtures `tests/fixtures/typecheck/tuple_literal_ok.scoop`、`tests/fixtures/typecheck/tuple_literal_type_mismatch_is_error.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0411 [DONE] Nullability：`T?` 作为 `Option<T>` 语法糖（spec §2.4）
- 描述：在 lowering 阶段把 `Nullable(TypeRef)` 映射到 `Option<...>`。
- 目标：先只做类型层映射；运行期表示后续 codegen 决定。
- 验收：typecheck fixture：`val x: Int?` 等价于 `Option<Int>`；`val y: Any?` 也可。
- 依赖：T0403、T0402
- 完成：parser 在 `crates/scoopc/src/parser/types.rs` 解析 `T?` 为 `ast::TypeRef::Nullable`；typecheck lowering 在 `crates/scoopc/src/typecheck/lower.rs` 将 `TypeRef::Nullable` desugar 为 `Option<T>`（`TypeStore::ty_option`）；新增 fixtures `tests/fixtures/typecheck/nullable_sugar_to_option_ok.scoop` 回归；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0412 [DONE] Cast 语义：`as`/`as?` 的类型规则（spec §4.4）
- 描述：实现 `as`/`as?` 的类型检查规则；运行期失败行为由 effect/RuntimeError 后续落地。
- 目标：先只做静态规则（可 cast/不可 cast）；不做 smart cast。
- 验收：typecheck fixture：`x as T` 类型为 `T`；`x as? T` 类型为 `T?`（即 Option<T>）。
- 依赖：T0213、T0411
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 为 `ExprKind::Cast` 实现类型推导（`as`→`T`、`as?`→`Option<T>`），并通过 `is_cast_allowed` 限制当前阶段仅允许引用类型之间的 cast；新增 fixtures `tests/fixtures/typecheck/cast_as_and_asq_ok.scoop`（pass）与 `tests/fixtures/typecheck/cast_value_to_ref_is_error.scoop`（fail）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0413 [DONE] `is`/`!is` + smart cast（val 场景）（spec §4.3）
- 描述：实现 flow-sensitive 类型收窄：`if (x is T) { x /* as T */ }`。
- 目标：先只支持不可变 `val` 与参数；不支持 `var` 与复杂控制流合流。
- 验收：typecheck fixture：在 `if` then 分支内使用 `x` 视为 `T`；在 else 分支保持原类型。
- 依赖：T0213、T0214、T0406
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 中实现最小 smart cast：识别 `if (x is T)` / `if (x !is T)` 条件，并仅对“稳定绑定”（参数 + `val`）在对应分支内把 `locals[decl_span]` 收窄为目标类型；`ExprKind::TypeCheck` 自身类型为 `Bool`；fixtures 回归：`tests/fixtures/typecheck/smart_cast_is_and_notis_ok.scoop`（pass）、`smart_cast_is_else_branch_not_narrowed_is_error.scoop`（fail）、`smart_cast_var_not_allowed_is_error.scoop`（fail）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0414 [DONE] `when` 类型规则：分支 LUB（spec §14.6）
- 描述：为 `when` 表达式计算结果类型（各分支类型的 LUB）。
- 目标：先只支持简单类型：相同类型则返回该类型，否则 fallback 到 `Any`（后续再做真正 LUB）。
- 验收：typecheck fixture：`when { ... }` 各分支 Int/Int → Int；Int/String → Any（或报错，按设计）。
- 依赖：T0215、T0405
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 实现/补齐 `ExprKind::When` 的最小 LUB：分支类型一致→该类型；不一致→`Any`；忽略 `Nothing` 分支；并避免在推导为 `Any` 时短路（保证后续分支与穷尽性检查仍会执行）；新增回归 fixture `tests/fixtures/typecheck/when_lub_mixed_to_any_missing_else_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0415 [DONE] 值类型更新：`with` 表达式类型检查与 path 校验（spec §2.6）
- 描述：检查 `with` 的 base 必须是 struct/tuple/enum（按设计）；path 必须存在且 RHS 类型匹配。
- 目标：先只支持 struct 字段更新；嵌套 path 可分后续任务。
- 验收：typecheck fixture：`p with { x: 1 }` OK；`p with { missing: 1 }` 报错并指向 path。
- 依赖：T0216、T0409、T0408
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 中实现 `infer_with_update_expr_type`：递归 typecheck base；限制 base 必须是 `struct` 名义值类型；校验更新项 path 不重复且不存在包含关系；校验字段存在性与 RHS 类型匹配；并支持嵌套路径（中间段字段类型必须为 `struct`）。新增 fixtures：`tests/fixtures/typecheck/with_update_struct_field_ok.scoop`、`with_update_unknown_field_is_error.scoop`、`with_update_field_type_mismatch_is_error.scoop`、`with_update_duplicate_path_is_error.scoop`、`with_update_overlapping_paths_is_error.scoop`、`with_update_nested_path_ok.scoop`、`with_update_nested_path_type_mismatch_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0416 [DONE] 变量绑定规则：`val/var` 赋值与重定义检查（spec §9）
- 描述：typecheck 阶段检查 `var` 可赋值、`val` 不可再次赋值；同一作用域重复定义报错。
- 目标：先只覆盖 block 内；不涉及闭包捕获与跨块。
- 验收：typecheck fixture：`val x = 1; x = 2` 报错；`var x = 1; x = 2` 通过。
- 依赖：T0227、T0304、T0405
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 增加语句层赋值检查：仅允许对 resolver 写回的局部 `var` 赋值（`ExprKind::Assign` + `ResolvedValueRef::Local` 且 decl span 位于 `mutable_bindings`）；对 `val`/参数赋值报 `scoop::typecheck::assignment_target_not_mutable`。同一作用域重复定义由 resolver 的 block scope 统一报 `scoop::resolve::duplicate_definition`。新增/补齐 fixtures `tests/fixtures/typecheck/val_reassign_is_error.scoop`、`tests/fixtures/typecheck/var_reassign_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0417 [DONE] 基础控制流语句：`return`（函数内）
- 描述：解析并类型检查 `return expr?`，并校验返回类型。
- 目标：先不支持 non-local return（spec §7.3）；只支持普通函数。
- 验收：typecheck fixture：返回类型不匹配时报错；`return` 在非函数体报错（若 parser 允许则 typecheck 报）。
- 依赖：T0226、T0404、T0405
- 完成：parser 在 `crates/scoopc/src/parser/stmt.rs` 支持 `return`/`return expr`；typecheck 在 `crates/scoopc/src/typecheck/expr.rs` 校验返回值类型、`Unit` 返回与“非函数体 return”诊断；fixtures 覆盖 `return_type_mismatch_is_error`、`return_value_required_is_error`、`return_in_lambda_is_error`、`return_unit_no_value_ok`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0418 [DONE] sysroot：补齐内建标量类型（整数体系 + 标准别名 + Bool/String/Unit/Nothing）（spec §2.3.4 / runtime §3）
- 描述：在 sysroot 中提供“内建标量类型的可见声明”，包括：
  - word-sized：`Int` / `UInt`（随 target 指针宽度变化）
  - fixed-width：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 其他最小基石：`Bool`、`String`、`Unit`、`Nothing`
- 目标：只做“声明层”：类型名/可见成员最小化；不要求标准库实现齐全；不引入任何运行期行为。
- 验收：新增 resolve fixture：`import scoop.core.*` 后引用上述类型与别名都可解析；`scoop test` 通过。
- 依赖：T0010、T0011、T0251、T0314
- 完成：在 `sysroot/core.scoop` 补齐标量类型声明与标准别名；新增/补齐 resolve fixture `tests/fixtures/resolve/sysroot_scalar_types_ok.scoop` 覆盖 `import scoop.core.*` 可解析；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0419 [DONE] sysroot：补齐 `RuntimeError` 与相关枚举值（spec §5.7 / §4.4 / Appendix B.3.3）
- 描述：新增 `enum RuntimeError { ClassCastFailed, NullAssertionFailed, ... }`（按 spec）并确保可在 Raise<RuntimeError> 中使用。
- 目标：先只定义错误枚举；不实现打印/堆栈。
- 验收：新增 resolve fixture：引用 `RuntimeError.NullAssertionFailed` 可解析；typecheck fixture：`Raise<RuntimeError>` 类型合法。
- 依赖：T0418、T0402
- 完成：`sysroot/core.scoop` 已定义 `enum RuntimeError { NullAssertionFailed, ClassCastFailed }`；fixtures 覆盖 `RuntimeError.NullAssertionFailed`（`tests/fixtures/resolve/sysroot_runtime_error_value_ref_ok.scoop`）与 `Raise<RuntimeError>`（`tests/fixtures/typecheck/raise_runtime_error_type_ok.scoop`）；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0420 [DONE] 类型关系：`Nothing` 作为 bottom type（spec §2.1/§5.7）
- 描述：在类型系统中实现 `Nothing <: T`（对任意 T），用于 `Raise.raise`、`return`、不可达分支等。
- 目标：先只实现 `Nothing` 子类型规则；不实现完整子类型系统。
- 验收：typecheck fixture：`fun f(): Any { return fail() }`（其中 `fail(): Nothing`）允许 `Nothing` 兼容 `Any` 返回；并覆盖 `Nothing` 作为调用实参可赋值给任意参数类型。
- 依赖：T0419
- 完成：`crates/scoopc/src/typecheck/expr.rs` 的 `is_type_assignable` 已支持 `Nothing <: T`；新增 typecheck fixture `tests/fixtures/typecheck/nothing_is_bottom_type_ok.scoop` 覆盖 `return` 与 call arg；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0421a [DONE] `!!`：not-null assertion 的类型规则（Appendix B.3.3）
- 描述：`x!!`：若 `x: T?`（即 `Option<T>`），则结果类型为 `T`；若 `x` 非 nullable 则报错。
- 目标：先只实现 typecheck 的静态类型规则；不引入 required effects 检查；不实现运行期语义。
- 验收：typecheck fixtures：
  - `val x: Int?; val y: Int = x!!` 通过
  - `val x: Int; val y: Int = x!!` 报错（错误码稳定）
- 依赖：T0212、T0411、T0406
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 为 `ExprKind::NotNullAssert` 增加类型推导：要求操作数为 `Option<T>` 并返回 `T`；新增诊断 `scoop::typecheck::not_null_assert_operand_not_nullable`；新增 fixtures `tests/fixtures/typecheck/not_null_assert_ok.scoop` 与 `tests/fixtures/typecheck/not_null_assert_operand_not_nullable_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0422 [DONE] `?.` safe-call 与 `?:` Elvis 的类型规则（Appendix B.3.1/3.2）
- 描述：`x?.m()` 返回 `R?`；`x ?: y` 的结果类型为 `T`（若 y: T）。
- 目标：先只覆盖 Option<T>（nullable sugar）；不引入真正的 null 值。
- 验收：typecheck fixture：`val y: Int? = x?.len()` 合法；`val z: Int = x ?: 0` 合法。
- 依赖：T0229、T0411、T0407
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 实现 safe-call 的 receiver 检查与返回值 `Option` 包装（`Call(SafeMemberAccess)` → `infer_member_call_expr_type(..., safe=true)` 返回 `Option<Ret>`；字段访问 `receiver?.field` 返回 `Option<FieldTy>`），并实现 Elvis `?:` 的类型规则（`Option<T> ?: T` → `T`，rhs 需可赋值给 T）；新增 fixture `tests/fixtures/typecheck/safe_call_and_elvis_ok.scoop` 覆盖 `x?.len()` 与 `n ?: 0`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0423 [DONE] struct literal 的类型检查（字段存在性/类型匹配）
- 描述：检查 `Point { x: 1, y: 2 }`：字段必须存在、不可重复、类型匹配、必填字段覆盖规则（按设计）。
- 目标：先只支持所有字段都必须提供的模式；默认值/可选字段后置。
- 验收：typecheck fixture：缺字段/多字段/重复字段都报错并定位到字段名或逗号位置。
- 依赖：T0224、T0409、T0405
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 实现 `infer_struct_lit_expr_type`：校验 struct 类型、字段存在性/重复、初始化值类型可赋值，并强制必须显式提供所有字段（缺字段尽量指向 `}`）。新增/补齐 fixtures：`tests/fixtures/typecheck/struct_lit_ok.scoop`、`tests/fixtures/typecheck/struct_lit_unknown_field_is_error.scoop`、`tests/fixtures/typecheck/struct_lit_duplicate_field_is_error.scoop`、`tests/fixtures/typecheck/struct_lit_missing_fields_is_error.scoop`、`tests/fixtures/typecheck/struct_lit_field_type_mismatch_is_error.scoop`、`tests/fixtures/typecheck/struct_lit_not_struct_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0424 [DONE] `with`：嵌套 path 与并行求值语义（spec §2.6）
- 描述：支持 `p with { a.b: v }` 的嵌套更新，并保证 RHS 基于“原值并行求值”（无顺序依赖）。
- 目标：先只实现 typecheck 侧的规则与必要诊断；真正 lowering 放到 IR 阶段单独任务。
- 验收：typecheck fixture：嵌套字段类型不匹配时报错；同一字段多次更新报错或明确覆盖规则（需决定）。
- 依赖：T0415
- 完成：该任务内容已在 T0415 的实现中覆盖：`infer_with_update_expr_type` 支持 `a.b.c` 嵌套 path，并以“禁止重复/包含 path”的静态约束来保持并行语义；fixtures：`tests/fixtures/typecheck/with_update_nested_path_ok.scoop`、`with_update_nested_path_type_mismatch_is_error.scoop`、`with_update_duplicate_path_is_error.scoop`、`with_update_overlapping_paths_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0425 [DONE] 声明类型：enum（rich enum）类型表示与收集（spec §2.3.2）
- 描述：在 type env 中加入 enum variant 信息（tag + payload types），并检查重复 variant/字段。
- 目标：先只支持 enum variant（无方法/属性）；niche 优化后置。
- 验收：typecheck fixture：enum 重复 variant 名报错；variant 字段类型未解析报错。
- 依赖：T0236、T0404
- 完成：在 `crates/scoopc/src/typecheck/type_env.rs` 中收集 enum variants（tag + payload fields），并在构建 type env 阶段检测重复 variant/字段；fixtures 覆盖重复 variant（`tests/fixtures/typecheck/enum_duplicate_variant_is_error.scoop`）、重复字段（`tests/fixtures/typecheck/enum_variant_duplicate_field_is_error.scoop`）与字段类型未解析（`tests/fixtures/typecheck/enum_variant_field_unresolved_type_is_error.scoop`）；新增单测 `typecheck::type_env::tests::sysroot_type_env_collects_option_variants`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0426 [DONE] 枚举构造表达式：`Some(x)` 的类型检查（spec §4）
- 描述：把 `Some(x)` 解析/绑定为某个 enum variant 构造，并检查参数数量与类型。
- 目标：先只支持同名唯一的 variant；重名/重载后续处理。
- 验收：typecheck fixture：`val o: Option<Int> = Some(1)` 通过；`Some()` 参数数不对时报错。
- 依赖：T0240、T0311、T0425
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 的 `infer_call_expr_type` 中把“未 resolve 的 `Call(Ident)`”当作 enum variant ctor 候选处理；通过 `TypeEnv::find_enum_variants_named` 要求同名唯一，否则报 `scoop::typecheck::ambiguous_enum_variant_ctor`；对唯一候选检查参数数量（`scoop::typecheck::enum_variant_ctor_arity_mismatch`）与参数类型（`scoop::typecheck::enum_variant_ctor_arg_type_mismatch`），并做最小泛型推断（从 payload 字段为直接 type param 的位置推断 `T`，缺失时报 `scoop::typecheck::enum_variant_ctor_type_arg_not_inferred`）。fixtures：`tests/fixtures/typecheck/enum_variant_ctor_some_ok.scoop`、`tests/fixtures/typecheck/enum_variant_ctor_arity_mismatch_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0427 [DONE] `when`：variant pattern 与 tuple pattern 的类型检查（spec §4）
- 描述：对 pattern 进行类型约束：variant pattern 仅用于 enum；tuple pattern 仅用于 tuple；绑定变量进入分支作用域。
- 目标：先不做穷尽性；先只做“每个分支内部类型正确”。
- 验收：typecheck fixture：`when(opt){ Some(x)->x; None->0 }` 通过；把 Some 用在非 enum 上时报错。
- 依赖：T0243、T0426、T0410
- 完成：新增 `crates/scoopc/src/typecheck/when_pat.rs` 的 `infer_when_pat_bindings`，为 tuple/variant pattern 做最小类型约束并收集 binder；并在 `crates/scoopc/src/typecheck/expr.rs` 的 `ExprKind::When` 中把 binder 注入 arm 局部环境；fixtures：`tests/fixtures/typecheck/when_variant_pattern_binds_ok.scoop`、`when_variant_pattern_not_enum_is_error.scoop`、`tests/fixtures/typecheck/when_tuple_pattern_binds_ok.scoop`、`when_tuple_pattern_not_tuple_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0428 [DONE] `when`：穷尽性检查（enum/Bool/Option）与 else 规则（spec §4.1）
- 描述：对可穷尽类型要求覆盖所有 variant（或允许 else）；非穷尽类型必须有 else/_。
- 目标：先只支持 enum 与 Bool 与 Option<T>；嵌套组合后续。
- 验收：typecheck fixture：缺少 None 分支时报错；覆盖完整且仍写 else 时产生 warning（先可仅记录 warning，不必 fixtures 断言）。
- 依赖：T0427
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 实现 `check_when_exhaustiveness`：支持 enum/Bool/Option 的穷尽性检查与缺失分支诊断；对非穷尽类型强制 `else`/`_`/bind catch-all；穷尽时仍写 `else` 记录 warning。fixtures：`tests/fixtures/typecheck/when_option_missing_none_is_error.scoop`、`when_int_missing_else_is_error.scoop`、`when_bool_missing_false_is_error.scoop`、`when_enum_missing_variant_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0429 [DONE] pattern guard：带 `if` 的分支视为非穷尽（spec §4.1/§4）
- 描述：当某个分支带 guard 时，穷尽性检查应要求 else/_（或把该分支不计入覆盖）。
- 目标：先只实现规则；不做路径敏感分析。
- 验收：typecheck fixture：`Some(x) if x>0 -> ...` 场景缺 else 时报错。
- 依赖：T0428
- 完成：在 `crates/scoopc/src/typecheck/expr.rs` 的 `check_when_exhaustiveness` 中过滤带 guard 的分支：不计入 variant 覆盖集合，也不视为 catch-all；fixtures：`tests/fixtures/typecheck/when_guard_missing_else_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0430 [DONE] destructuring `val`：tuple/struct pattern 绑定（spec §4.2、§9）
- 描述：实现 `val (a,b)=expr`、`val Point { x, y } = expr` 的类型检查与绑定，并强制 `var` 不允许。
- 目标：先只支持 tuple/struct；enum destructuring 可复用 when pattern 后续再补。
- 验收：typecheck fixture：`var (a,b)=...` 报错；绑定变量类型正确；字段重命名后变量名类型正确。
- 依赖：T0244、T0410、T0409
- 完成：新增 `crates/scoopc/src/typecheck/val_pat.rs` 实现 `val` 解构 pattern 的最小类型检查（tuple/struct，支持 tuple `..` rest、struct 字段重命名与缺字段诊断），并在 `crates/scoopc/src/typecheck/expr.rs` 的 `check_local_val_decl_exprs` 接入 bindings 注入局部类型表；新增 fixtures `tests/fixtures/typecheck/destructuring_tuple_ok.scoop` 与 `tests/fixtures/typecheck/destructuring_struct_rename_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0431 [DONE] 属性 v0：class 属性声明头与 backing field 规则（spec §10.1）
- 描述：实现 class 属性的类型检查：默认 getter/setter、`field` 可见性、是否生成 backing field 的判定。
- 目标：先只做静态规则与诊断；不做 codegen。
- 验收：typecheck fixture：setter 中引用 `field` 合法；未生成 backing field 的 computed property 禁止引用 `field`。
- 依赖：T0234、T0404
- 完成：resolver 在 class 属性 accessor scope 内注入隐式局部绑定 `field`（只在 accessor 内可见，便于后续语义检查）；typecheck 新增 `properties` 检查：`val` 属性禁止 setter、computed 属性（无 initializer 且无默认 accessor）引用 `field` 报错（`scoop::typecheck::field_used_without_backing_field`）；fixtures：`tests/fixtures/typecheck/class_property_field_in_setter_ok.scoop`、`tests/fixtures/typecheck/class_property_computed_field_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0432 [DONE] 属性 v1：value type（struct/enum）仅允许 getter-only computed（spec §10.2）
- 描述：对 struct/enum 中的属性限制：禁止 setter；禁止 backing field。
- 目标：先只在 typecheck enforce；parser 仍可解析。
- 验收：typecheck fixture：struct 内 `var` 属性或 setter 报错；getter-only 通过。
- 依赖：T0431、T0409、T0425
- 完成：在 `crates/scoopc/src/typecheck/properties.rs` 增加值类型属性规则：struct/enum 中属性不允许 `var`（`scoop::typecheck::value_type_property_must_be_val`）；computed 属性（声明了 getter）不允许 initializer（`scoop::typecheck::value_type_property_initializer_not_allowed`）；同时沿用 `val_property_setter_not_allowed` 禁止 setter。并将入口统一为 `check_file_properties`（class + value type）。新增 fixtures：`tests/fixtures/typecheck/struct_computed_property_getter_only_ok.scoop`、`struct_property_setter_not_allowed_is_error.scoop`、`enum_computed_property_getter_only_ok.scoop`、`enum_computed_property_initializer_not_allowed_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0433 [DONE] 扩展属性：必须 computed（无 backing field）（spec §10.3）
- 描述：实现 extension property 的规则：不能有 initializer/field；编译模型为静态 getter/setter。
- 目标：先只做静态检查；lowering 到函数在 IR 阶段做。
- 验收：typecheck fixture：`val String.lastChar get() = ...` 通过；写 initializer 报错。
- 依赖：T0233、T0234
- 完成：新增顶层 AST `ExtensionPropertyDecl`（`ast::Item::ExtensionProperty`），并在 parser 中支持 `val/var ReceiverType.name: Type get()/set()` 语法；resolver phase 2 支持进入 extension property 的 initializer/accessor 并注入隐式 `field` 绑定以保持诊断一致；typecheck 增加扩展属性静态规则（必须 computed：要求 getter；`var` 需 setter；禁止 initializer；禁止 `field`），并新增错误码：`scoop::typecheck::extension_property_*`；fixtures：`tests/fixtures/typecheck/extension_property_getter_only_ok.scoop`、`extension_property_initializer_not_allowed_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0434a [DONE] 委托属性：语法 + 最小静态规则（spec §10.4）
- 描述：在属性声明中支持 `val/var name: T by expr`，并在 typecheck 中检查 delegated property：
  - 仅允许出现在 class（struct/enum 禁止）
  - delegate 需要存在 `getValue`；`var` 还需要存在 `setValue`（可先只检查方法名存在性）
- 目标：只做静态规则与诊断；不生成 `$delegate` 字段；不检查 `PropertyMeta` 参数类型（留给后续任务）。
- 验收：typecheck fixtures：
  - struct/enum 里写 `by` 报错
  - class 里写 `by` 且 delegate 定义 `getValue/setValue` 通过
  - 缺少 `getValue`/`setValue` 报错
- 依赖：T0234、T0431
- 完成：AST `PropertyDecl` 新增 `delegate: Option<Expr>` 字段；parser 支持 `val/var name: T by expr`（把 `by` 作为上下文关键字识别，且与 initializer/accessors 语法互斥）；resolver 在属性初始化语境内解析 delegate expr（并对 delegated property 不注入 `field`）；typecheck 增加 delegated property 静态规则：struct/enum 禁止、class 要求 delegate 类型存在 `getValue`，`var` 还要求 `setValue`（当前仅检查方法名存在性，`PropertyMeta`/签名检查留给 T0434b/T1208）。fixtures：`tests/fixtures/typecheck/delegated_property_*`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0434b [DONE] 委托属性：对接 `PropertyMeta` 并升级为签名检查（spec §10.4）
- 描述：在 typecheck 中把 delegated property 的规则升级为“签名检查”：
  - `getValue(thisRef: T, property: PropertyMeta): V`
  - `setValue(thisRef: T, property: PropertyMeta, value: V)`（仅 `var`）
  - 并为后续 lowering 记录必要信息（但不在本任务生成 `$delegate` 字段与转发函数）。
- 目标：仍以静态检查为主；完整 lowering 见 T1210。
- 验收：typecheck fixture：`getValue` 第二参不是 `PropertyMeta` 报错；`var` 缺 `setValue` 或参数不匹配报错。
- 依赖：T0434a
- 完成：sysroot 新增 `scoop.core.PropertyMeta`（当前阶段仅占位字段）；typecheck 委托属性升级为签名检查：`getValue/setValue` 需匹配 `PropertyMeta` 与属性类型（`thisRef` 允许 `Any`），并新增 mismatch fixtures；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0435 [DONE] 函数类型（含 receiver + effects）的类型表示与子类型规则（spec §7.5、§5.8）
- 描述：在 `ty` 中加入 FunctionType：参数/返回/receiver/effect row，并定义最小子类型关系（参数逆变/返回协变 + effect row containment）。
- 目标：先只支持无泛型的函数类型；完整子类型与推断后续补齐。
- 验收：新增 typecheck fixtures：`tests/fixtures/typecheck/function_type_subtyping_ok.scoop` 通过；`function_type_effect_row_not_contained_is_error.scoop` 与 `function_type_param_variance_is_error.scoop` 产生稳定错误码 `scoop::typecheck::return_type_mismatch`。
- 依赖：T0219、T0401、T0608
- 完成：`crates/scoopc/src/ty/mod.rs` 新增 `EffectRow`/`FunctionType` 与显示格式；`crates/scoopc/src/typecheck/lower.rs` 支持 `TypeRef::Function` lowering（含 effect row 项必须为 `effect` 的最小静态检查，缺省 effect 为 `Pure`）；`crates/scoopc/src/typecheck/expr.rs` 的 `is_type_assignable` 增加函数子类型规则（参数逆变、返回协变、effects containment）。fixtures：新增 `tests/fixtures/typecheck/function_type_*`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0436 [DONE] 扩展函数：静态分发与 receiver 作为第一个参数（spec §7.4）
- 描述：typecheck 阶段将 extension fun 视为普通函数（receiver 第一个参数），并实现最小分发规则（member 优先）。
- 目标：先不支持同名多个 extension 的重载；歧义时报错。
- 验收：typecheck fixture：`fun Any.id(): Any { this }` 可被调用 `x.id()`；解析到 extension。
- 依赖：T0233、T0312、T0407
- 完成：typecheck 阶段将扩展函数降糖为“receiver 作为第一个参数”的普通顶层函数签名（`crates/scoopc/src/typecheck/expr.rs`），并在函数体内为扩展 receiver 注入隐式 `this` 绑定（resolver 将 `this` 解析到 receiver 的 decl span）；`receiver.member()` / `receiver?.member()` 调用按扩展候选进行类型检查，且当同名扩展函数存在多个候选时在 typecheck 报歧义（新错误码 `scoop::typecheck::ambiguous_call`）。新增 fixture：`tests/fixtures/typecheck/extension_fun_receiver_this_ok.scoop`。`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0437 [DONE] 泛型：声明处变型 `in/out` + star projection（spec §3.2~§3.3 / Appendix B.4）
- 描述：在 parser/type system 中支持 `in T`/`out T` 与 `*`，并实现最小合法性检查。
- 目标：先只解析并存储 variance/star；子类型规则可先限制为“只对引用类型参数生效”（按 spec）。
- 验收：typecheck fixture：`interface ReadOnlyProperty<in T, out V>` 可解析并 typecheck；非法 variance 位置报错。
- 依赖：T0249、T0401
 - 完成：`ty::TypeKind` 新增 `Param` 用于表示 type parameter；`TypeEnv` 记录每个类型符号的 `type_param_variances`；`TypeLowering` 支持 lowering `T` 与 `*`（`*` 暂 lowering 为 `Any`），并在 type decl 上实现 Kotlin-like 的最小变型位置检查（新错误码 `scoop::typecheck::variance_position_violation`）；`ExprTypeError::is_type_assignable` 支持名义类型的声明处变型子类型（仅当对应 type args 都是引用类型时生效）。新增 fixtures：`tests/fixtures/typecheck/variance_read_only_property_ok.scoop`、`variance_out_param_used_in_param_position_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0438 [DONE] 声明类型：class 的最小类型检查（字段/构造参数/方法头）
- 描述：实现 class：主构造参数作为字段（`val/var`）与成员方法头解析后的类型收集。
- 目标：先不实现继承/override；先把“类有成员”这件事跑通。
- 验收：typecheck fixture：`class User(val name: String) { fun get(): String { name } }`（按语法）通过。
- 依赖：T0248、T0203、T0404
 - 完成：AST `Param` 新增 `kind: Option<ValKind>` 并在 parser 主构造参数列表记录 ctor param 的 `val/var` 前缀；resolver `Index` 将 class ctor 的 `val/var` 参数注入 value namespace（支持 `this.x` 成员访问）；typecheck `expr` phase 递归进入 class 成员方法体，注入 `this` 与 ctor params 的局部类型表并把 class 字段纳入 member value 类型表；新增 fixture `tests/fixtures/typecheck/class_ctor_param_field_and_member_fun_ok.scoop`；更新 parse AST goldens：`tests/fixtures/parse/annotation_class_basic.ast`、`tests/fixtures/parse/type_member_nested_type.ast`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0439 [DONE] 继承与 override：open/abstract/sealed + override 检查（Appendix B.2）
- 描述：实现最小规则：class 单继承；override 必须显式；被覆盖成员需 open/abstract；sealed 限制同编译单元。
- 目标：先只做静态检查；vtable/codegen 后置。
- 验收：typecheck fixture：override 缺失时报错；override 目标不是 open时报错；sealed 跨文件继承时报错（需多文件单元，见 T0307）。
- 依赖：T0438、T0245、T0307
 - 完成：resolver `Index::Symbol` 增加 `ModifierSet`（open/abstract/sealed/override）以支持跨文件的继承语义查询；typecheck 新增 `inheritance` pass：class 单继承检查（多个基类构造调用报错）、继承 final class 报错（需 `open/abstract/sealed`）、sealed 跨文件直接继承报错、override 必须显式、只能 override `open/abstract` 成员（member fun 做按参数个数的最小匹配以避免把重载误判为 override）；fixtures runner 新增 `tests/fixtures/typecheck_multi/<case>/` 支持，并新增 fixtures：`tests/fixtures/typecheck/override_missing_is_error.scoop`、`override_target_not_open_is_error.scoop`、`superclass_not_open_is_error.scoop` 与多文件 `tests/fixtures/typecheck_multi/sealed_cross_file_inheritance_is_error/`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0440 [DONE] interface：多实现与默认方法的限制策略（spec §2.2.2）
- 描述：实现 interface 声明收集与实现列表检查；默认方法可先允许但不要求 codegen。
- 目标：先只在 typecheck 检查签名一致性；冲突规则后续。
- 验收：typecheck fixture：class 实现 interface 并提供方法通过；缺少方法时报错。
- 依赖：T0438、T0439
 - 完成：resolver `FunOverload` 记录 `has_body`（区分 interface 抽象方法 vs 默认方法）；typecheck 新增 `interfaces` pass：
   - class/object：实现列表中允许多个 interface；并检查 interface 的抽象方法必须被实现；
   - 默认方法（带 body）不要求实现（先不要求 codegen）；
   - 误用（implements 非 interface / 对非 class 做 ctor call）会报错（新错误码：`scoop::typecheck::supertype_not_interface`、`scoop::typecheck::supertype_ctor_call_not_class`）。
   新增 fixtures：`tests/fixtures/typecheck/interface_impl_ok.scoop`、`interface_missing_member_is_error.scoop`、`interface_default_method_not_required_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0441 [DONE] Boxing：值类型装箱到 `Any`/interface（spec §2.5）
- 描述：实现“语义正确”的 boxing：当值类型被当作 `Any`/interface 使用时生成 box（类型系统层先建模）。
- 目标：先只在 typecheck 允许/禁止；真正分配与布局留给 codegen/runtime。
- 验收：typecheck fixture：`val a: Any = 1`（若 Int 是值类型）通过；`val i: IFoo = Point(...)`（若实现）按规则通过/报错。
- 依赖：T0418、T0405、T0440
 - 完成：`TypeEnv` 额外收集 nominal type 的 direct supertypes（FQN），`ExprTypeError::is_type_assignable` 扩展：
   - `T <: Any`（value types 通过 boxing，上转到 `Any`）；
   - nominal ref types 支持沿 supertypes 的最小上转（class 继承 / interface 实现与继承）；
   - nominal value types 在目标为 interface 时允许 boxing。
   同时把顶层/局部 `val` initializer 的检查从“严格相等”升级为 `is_type_assignable`，并让 `as/as?` 支持 value → Any/interface 的显式 boxing。新增 fixtures：`tests/fixtures/typecheck/boxing_value_to_any_ok.scoop`、`boxing_value_to_interface_ok.scoop` 与 fail case `boxing_value_to_interface_missing_impl_is_error.scoop`。`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0442 [DONE] 语句/循环的类型检查：`while`/`break`/`continue`
- 描述：检查 while 条件为 Bool；break/continue 必须在循环内；循环体类型规则明确（Unit）。
- 目标：先不支持 label；不支持 for。
- 验收：typecheck fixture：`while(1){}` 报错；`break` 在函数顶层报错；合法 while 通过。
- 依赖：T0228、T0405
 - 完成：typecheck 增加 loop depth 上下文：`while` 条件必须可赋给 `Bool`（新错误码 `scoop::typecheck::while_condition_not_bool`）；`break/continue` 必须位于循环体内（新错误码 `scoop::typecheck::break_not_in_loop` / `scoop::typecheck::continue_not_in_loop`）。新增 fixtures：`tests/fixtures/typecheck/while_condition_not_bool_is_error.scoop`、`break_not_in_loop_is_error.scoop`、`continue_not_in_loop_is_error.scoop`、`while_break_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0443 [DONE] 赋值类型检查：lhs 可写性（var）与类型匹配（spec §9）
- 描述：实现 `x = y`：x 必须是 var 绑定或可写属性；y 类型必须可赋给 x。
- 目标：先只支持局部 var 与字段/属性（若已实现）；复合赋值后置。
- 验收：typecheck fixture：给 val 赋值报错；给 var 赋值但类型不匹配报错（指向 rhs span）。
- 依赖：T0227、T0416、T0406
 - 完成：在 `crates/scoopc/src/typecheck/expr.rs` 扩展 `ExprKind::Assign` 的语句层检查：
   - lhs：支持局部 `var` 绑定（复用 `mutable_bindings`）与成员访问 `this.x`（class ctor `var` 参数 / `var` 属性）；
   - rhs：使用 `is_type_assignable` 检查 `rhs <: lhs`，不匹配时报新错误码 `scoop::typecheck::assignment_type_mismatch` 并定位到 rhs span。
   新增 fixtures：`tests/fixtures/typecheck/var_reassign_type_mismatch_is_error.scoop`、`tests/fixtures/typecheck/class_var_property_reassign_ok.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0444 [DONE] `inline` 与 non-local return 的语义门禁（spec §7.2/§7.3）
- 描述：实现最小检查：只有 inline 函数的 lambda 参数允许 non-local return；其余场景报错。
- 目标：先只做静态限制，不做实际 inlining 优化。
- 验收：typecheck fixture：非 inline lambda 中 `return` 报错；inline 场景允许（具体语法按设计）。
- 依赖：T0245、T0226、T0222
 - 完成：在 `crates/scoopc/src/typecheck/expr.rs` 为顶层函数签名记录 `is_inline`，并在表达式语句递归中新增 call/lambda 的 non-local return 门禁：
   - 默认：lambda body 中 `return` 仍报错（错误码保持 `scoop::typecheck::return_not_in_function_body`）；
   - 例外：当 lambda 作为 inline 函数调用的“函数类型参数实参”时，允许其 body 内出现 `return`（按外层函数返回类型做检查）。
   新增 fixtures：`tests/fixtures/typecheck/return_in_inline_lambda_ok.scoop`、`tests/fixtures/typecheck/return_in_non_inline_lambda_arg_is_error.scoop`；并更新 `return_in_lambda_is_error.scoop` 的说明。`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0446 [DONE] typealias：别名展开与循环检测（最小实现，支撑 sysroot 标准别名）（Appendix B.10）
- 描述：在 typecheck 的 `TypeRef → Type` lowering 阶段支持 `typealias`：把别名引用展开为其底层类型，并检测循环别名（直接/间接）。
- 目标：先只支持同包/同编译单元内的别名；跨包可见性与导出规则后续与 Cone 联动；错误信息需指出循环链路中的至少两个声明点。
- 验收：typecheck fixture：`typealias Byte = UInt8; val b: Byte = 1` 通过（或至少到签名检查通过）；构造 `typealias A=B; typealias B=A` 报循环别名错误（新错误码）。
- 依赖：T0314、T0403、T0404
 - 完成：在 `crates/scoopc/src/typecheck/lower.rs` 支持 `TypeSymbolKind::TypeAlias`：
   - lowering 时在别名声明处文件的 package/import 规则下展开 RHS（支持 sysroot 标准别名）；
   - 引入循环检测与缓存，新增错误码 `scoop::typecheck::cyclic_type_alias`，诊断至少标注两个别名声明点。
   新增 fixtures：`tests/fixtures/typecheck/typealias_byte_ok.scoop`、`tests/fixtures/typecheck/typealias_cycle_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0447 [DONE] 整数/布尔：一元与二元运算的类型规则（含位运算与移位）（spec §2.3.4）
- 描述：为内建整数类型实现运算符的静态规则：
  - 算术：`+ - * / %`
  - 比较：`== != < <= > >=`
  - 位运算：`& | ^ ~`
  - 移位：`<< >>`（仅整数；shift count 类型规则需固定）
- 目标：先只支持“同类型输入→同类型输出”的规则（不做数值提升/混合宽度运算）；不引入溢出检查（运行期语义留给 codegen 按 spec wrap-around/shift mask 落地）。
- 验收：typecheck fixture：`val x: UInt8 = 1; val y = x << 3` 通过；`val z = true << 1` 报错并定位到操作符。
- 依赖：T0252、T0211、T0405、T0407、T0418
 - 完成：在 `crates/scoopc/src/typecheck/expr.rs` 为 `ExprKind::Unary/Binary` 实现 Bool/整数运算符类型规则：算术/比较/位运算与移位（shift count 固定为 `Int`）、以及 `&&/||`；并允许整数字面量被上下文整数类型吸收（initializer/call args/赋值/二元同型规则）。新增错误码 `scoop::typecheck::unary_op_operand_type_mismatch` 与 `scoop::typecheck::binary_op_operand_type_mismatch`（均定位到操作符 span）。新增 fixtures：`tests/fixtures/typecheck/int_bool_ops_ok.scoop`、`bool_shift_is_error.scoop`；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0448 [DONE] class 初始化模型：property initializer / `init` / secondary constructor 规则（Appendix B.2.2）
- 描述：实现 class 初始化相关的静态规则：属性初始化表达式、多个 `init` block、secondary constructor body 的类型检查与顺序约束。
- 目标：先固定 Kotlin-like 初始化顺序与最小限制；复杂继承链与 effect 细节后续补齐。
- 验收：typecheck fixture：`init` 中引用未就绪成员时报错；secondary constructor 非法 delegation 报错；合法初始化顺序通过。
- 依赖：T0256、T0257、T0316、T0406
 - 完成：在 `crates/scoopc/src/typecheck/expr.rs` 扩展 class 初始化相关的表达式检查：
   - class 属性 initializer（`= expr`）进入最小 typecheck，并复用 `initializer_type_mismatch` 错误码；
   - `init { ... }` 与 secondary ctor body 进入 block/stmt 递归 typecheck（禁止 `return`）；
   - 当 class 存在主构造器时，secondary ctor 必须显式 `: this(...)`，否则报新错误码：
     - `scoop::typecheck::secondary_ctor_delegation_required`
     - `scoop::typecheck::secondary_ctor_delegation_must_be_this`
   新增 fixtures：
   - `tests/fixtures/typecheck/class_init_order_ok.scoop`
   - `tests/fixtures/typecheck/class_init_forward_reference_is_error.scoop`（由 resolver 报 `scoop::resolve::forward_reference`）
   - `tests/fixtures/typecheck/class_secondary_ctor_delegation_missing_is_error.scoop`
   - `tests/fixtures/typecheck/class_secondary_ctor_delegation_super_is_error.scoop`
   `cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0449 [DONE] enum 完整布局语义：niche 优化 / oversized variant boxing / disparity lint（spec §2.3.2）
- 描述：在类型系统层固定 rich enum 的布局选择规则：何时使用 niche optimization、何时对 oversized variant 自动 boxing、何时发出 size disparity lint。
- 目标：先把规则、诊断与 type metadata 固定下来；具体低层布局由 codegen 落地。
- 验收：typecheck fixture：`Option<RefType>` 命中 niche 优化路径；oversized variant 触发 boxing/lint（warning 可先记录不强制 golden）。
- 依赖：T0425、T0418
- 完成：
  - 新增 `crates/scoopc/src/ty/layout.rs`：定义 `TargetLayout`/`TypeLayout`/`EnumLayout` 等布局元数据形状（含 niche domain、tag type、variant boxing 标记）。
  - 新增 `crates/scoopc/src/typecheck/layout.rs`：实现 best-effort 布局计算与策略选择：
    - `Option<RefType>` / `Option<Bool>` niche 优化（支持 nested niche：外层使用 `0x1` 等非法值）；
    - rich enum：tag 类型按 variant 数量选择；当 size disparity 显著时对最大 variant 自动 boxing，并通过 `tracing::warn!` 发出 lint warning。
  - 在 typecheck pipeline 末尾接入 `check_file_type_layouts`，让 `scoop test` 会计算元数据并输出 lint（不影响 pass/fail）。
  - 新增 fixtures：
    - `tests/fixtures/typecheck/option_ref_niche_ok.scoop`
    - `tests/fixtures/typecheck/enum_oversized_variant_boxing_warn_ok.scoop`
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过（`scoop test` 会对 oversized enum 输出一条 warning）。

### T0450 [DONE] pattern rest `..`：类型检查与绑定规则（spec §4.2）
- 描述：实现 `..` 的静态规则：出现位置、只能出现一次、与 tuple/struct/variant pattern 的匹配关系，以及 rest 不引入绑定。
- 目标：先只支持 tuple/struct；variant positional rest 若语法允许则一并纳入，否则后续扩展。
- 验收：typecheck fixture：`val (x, ..) = t` 通过；多个 `..`、非法位置、对非解构类型使用 `..` 报错。
- 依赖：T0255、T0427、T0430
- 完成：
  - `when` pattern：新增 `ast::WhenPat::Rest`，parser 支持 tuple/variant pattern 内的 `..`（仅一次且必须为最后一个元素/参数）。
  - typecheck：tuple/variant rest 允许“前缀匹配 + 忽略剩余元素/字段”；新增错误码：
    - `scoop::typecheck::when_tuple_pat_too_short`
    - `scoop::typecheck::when_variant_pat_too_short`
  - resolver：`..` 不引入绑定。
  - fixtures：新增 typecheck fixtures 覆盖：
    - `val (x, ..) = t` 通过（tuple rest）
    - `val (x, ..) = 1` 报 `val_tuple_pat_not_tuple`
    - `when` tuple/variant rest 的 pass 与 too-short fail
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0451 [DONE] 委托属性：标准接口与标准 delegates 表面（spec §10.4）
- 描述：补齐 delegated property 的静态表面：`ReadOnlyProperty` / `ReadWriteProperty` 接口规则，以及 `scoop.delegates` 中 `lazy` / `observable` / `vetoable` / map-backed delegate 的最小声明面。
- 目标：先只固定签名与类型规则；具体库行为与线程安全语义后续任务补齐。
- 验收：resolve/typecheck fixture：`val x by lazy { ... }`、`var x by observable(...)`、map-backed delegate 均能过签名检查；缺少 `getValue/setValue` 报错。
- 依赖：T0434、T1208
- 完成：
  - sysroot：
    - 新增 `sysroot/delegates.scoop`：声明 `ReadOnlyProperty`/`ReadWriteProperty`、`LazyThreadSafetyMode`，以及 `lazy/observable/vetoable` 的最小签名表面。
    - 新增 `sysroot/collections.scoop`：声明最小 `Map<K, V>` 表面，并提供 `getValue`（用于 map-backed delegate）。
  - typecheck：
    - delegated property 的签名检查升级为“可跨文件解析”：通过 `TypeEnv` 获取声明处 `SourceFile` 与 import 上下文，避免用 use-site source slice 导致的误判。
    - 推导 delegate nominal type 的覆盖面扩展：
      - 构造调用 `Foo()`（原逻辑保持）
      - 顶层函数调用 `lazy { ... }` / `observable(...)`：使用返回类型的名义类型作为 delegate type
      - 标识符 `by data`：从 class 字段/ctor `val` 参数的类型注解推导 delegate type（用于 map-backed）
    - `getValue/setValue` 可来自 supertypes（例如 `ReadWriteProperty` 继承 `ReadOnlyProperty.getValue`）。
    - 对类型参数（如 `T`/`V`）做保守“通配”匹配，以支持标准泛型 delegates 的签名表面。
  - fixtures：新增 typecheck fixtures 覆盖标准 delegates：
    - `tests/fixtures/typecheck/delegated_property_lazy_ok.scoop`
    - `tests/fixtures/typecheck/delegated_property_observable_ok.scoop`
    - `tests/fixtures/typecheck/delegated_property_vetoable_ok.scoop`
    - `tests/fixtures/typecheck/delegated_property_map_backed_ok.scoop`
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0452 [DONE] `object` / `companion object`：类型规则与单例语义（Appendix B.9）
- 描述：实现 `object` / `companion object` 的类型检查规则：单例不可构造、成员访问类型正确、companion 可通过宿主类名访问。
- 目标：先只固定静态语义；初始化时机与 storage 留给 codegen/runtime。
- 验收：typecheck fixture：`object Foo` 不能像 class 一样调用构造；`ClassName.member` 在 companion 存在时通过；无 companion 时报错。
- 依赖：T0258、T0317、T0404
- 完成：
  - type env：收集顶层与嵌套 `object`（含未命名 companion → `Companion`）为 class-like nominal type，使 `Foo` 在表达式位置可拥有类型 `Foo`。
  - typecheck/expr：
    - 顶层 object 值引用：`Foo` → `Foo`
    - member access：支持 `Obj.member`/`ClassName.member` 的字段类型读取；支持 `TypeName.NestedObject` 作为 object 值并返回其名义类型
    - 调用门禁：新增诊断 `scoop::typecheck::object_not_constructible`，禁止 `Foo()` 构造 object
  - fixtures：
    - `tests/fixtures/typecheck/object_member_access_ok.scoop`
    - `tests/fixtures/typecheck/companion_member_access_ok.scoop`
    - `tests/fixtures/typecheck/companion_member_access_missing_is_error.scoop`
    - `tests/fixtures/typecheck/object_not_constructible_is_error.scoop`
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0453 [DONE] 通用函数重载解析 v1：按候选集合做决议
- 描述：对普通函数调用实现最小 overload resolution：按参数个数、位置/命名参数、可见性、形参可赋值关系筛选候选，并在唯一候选时完成绑定。
- 目标：先不做 most-specific 的复杂 tie-break；先覆盖“过滤后唯一”与“明确歧义”两类结果。
- 验收：typecheck fixture：`f(Int)` / `f(String)` 根据实参选中不同 overload；无匹配时报错；多候选同等匹配时报 `ambiguous_overload`。
- 依赖：T0319、T0407
- 完成：
  - typecheck/expr：
    - 顶层函数调用从“唯一候选”升级为最小重载决议：按 arity + 命名/位置实参映射 + `is_type_assignable` 过滤候选。
    - 新增稳定诊断：
      - `scoop::typecheck::no_matching_overload`：过滤后无候选；
      - `scoop::typecheck::ambiguous_overload`：过滤后仍有多个候选（当前阶段不做 most-specific）。
    - 单候选路径保持原有精确诊断（`call_arity_mismatch` / `call_arg_type_mismatch`），以避免回归。
  - fixtures：新增 typecheck fixtures 覆盖三类结果：
    - `tests/fixtures/typecheck/call_overload_select_by_arg_type_ok.scoop`
    - `tests/fixtures/typecheck/call_overload_no_match_is_error.scoop`
    - `tests/fixtures/typecheck/call_overload_ambiguous_is_error.scoop`
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0454 [DONE] 构造函数重载解析：primary / secondary constructors
- 描述：为 class 构造调用实现 overload resolution：primary constructor 与多个 secondary constructors 共同形成候选集合，按参数匹配与默认参数规则做决议。
- 目标：先只覆盖 class constructors；struct literal 命名字段构造仍走独立规则。
- 验收：typecheck fixture：同一 class 上多个 constructors 可按参数选中；无匹配和歧义都有稳定诊断。
- 依赖：T0257、T0318、T0453
- 完成：
  - resolve/index：暴露 `Index::cone_of_source`（crate 内）用于后续可见性过滤复用。
  - typecheck/lower：新增 `TypeLowering::lower_type_ref_in_decl_file`，按“声明处文件”的 package/import 规则 lowering ctor param `TypeRef`。
  - typecheck/expr：
    - 在 `Call(Ident)` 且 callee 未 resolve 时，使用 resolver 写回的 `CallCandidate::Constructor` 执行 class 构造调用的重载决议。
    - 支持默认参数：允许省略带默认值的形参，并在候选筛选中复用“位置/命名实参 → 形参槽位”的映射逻辑。
    - 仅覆盖 class constructors，并按 cone/file 规则过滤不可见构造器；无匹配/多匹配分别报 `no_matching_overload` / `ambiguous_overload`。
  - fixtures：新增 typecheck fixtures 覆盖 pass/no-match/ambiguous：
    - `tests/fixtures/typecheck/class_ctor_overload_select_ok.scoop`
    - `tests/fixtures/typecheck/class_ctor_overload_no_match_is_error.scoop`
    - `tests/fixtures/typecheck/class_ctor_overload_ambiguous_default_is_error.scoop`
  - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0455 [DONE] 扩展函数重载解析：member 优先 + receiver specificity
- 描述：在已有 extension 静态分发基础上，支持同名多个 extension overload，并固定优先级：member 胜出，其次按 receiver/参数更具体者胜出。
- 目标：先只覆盖同包/已导入 extension；跨包导入优先级与可见性复用现有规则。
- 验收：typecheck fixture：多个 extension overload 可按 receiver/参数类型选中；member 与 extension 同名时选 member；无唯一候选时报歧义。
- 依赖：T0436、T0453、T0312
 - 完成：
   - typecheck/expr：`receiver.member(...)` 扩展调用从“要求唯一 extension 候选”升级为重载决议：
     - 先按 receiver 可赋值关系 + 位置/命名实参映射 + 形参可赋值关系过滤候选；
     - 多候选时使用 most-specific tie-break：receiver 与每个实参的期望类型都更具体者胜出；
     - 无匹配时报 `no_matching_overload`；无唯一 most-specific 时报 `ambiguous_overload`；
     - 单候选路径保持旧的精确 mismatch 诊断，并补齐命名实参支持。
   - fixtures：新增 typecheck fixtures：
     - `tests/fixtures/typecheck/extension_overload_select_by_receiver_ok.scoop`
     - `tests/fixtures/typecheck/extension_overload_select_by_arg_type_ok.scoop`
     - `tests/fixtures/typecheck/extension_overload_ambiguous_is_error.scoop`
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0456 [DONE] enum variant 构造与 pattern 消歧：摆脱“同名唯一 variant”假设
- 描述：当不同 enum 存在同名 variant（如多个 `None` / `Some` 风格命名）时，variant 构造与 pattern 匹配应按期望类型或 `when` subject type 解析，而不是要求全局同名唯一。
- 目标：先只覆盖“存在明确期望类型/subject type”的场景；无足够上下文时允许保留歧义诊断。
- 验收：typecheck fixture：两个 enum 都有 `None` 时，在 `when (opt)` 中能正确解析到 `Option.None`；缺少上下文时给出歧义错误。
- 依赖：T0426、T0427、T0318
 - 完成：
   - typecheck：新增 `infer_expr_type_in_expected_context`，在存在期望类型时对同名 enum variant ctor 做消歧（仅在“全局同名候选 > 1”时生效，避免影响单候选语义）。
   - typecheck：在 initializer/return/assignment 与 struct literal/with-update 字段值的推导路径中传入期望类型，从而允许 `val opt: Option<Int> = Some(1)` 在存在其它同名 `Some` enum 时仍能解析为 `Option.Some`。
   - fixtures：
     - `tests/fixtures/typecheck/enum_variant_disambiguate_by_expected_and_subject_ok.scoop`：覆盖“构造按期望类型消歧 + `when` pattern 按 subject type 消歧”。
     - `tests/fixtures/typecheck/enum_variant_ctor_ambiguous_without_context_is_error.scoop`：覆盖“缺少期望类型时保留歧义诊断”。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0457 [DONE] 重载冲突诊断：重复签名、不可区分签名、默认参数导致的冲突
- 描述：在声明检查阶段诊断非法 overload：完全相同签名、仅返回类型不同、默认参数展开后不可区分等情况。
- 目标：先覆盖函数与 constructors；extension / effects 的细化规则后续复用。
- 验收：typecheck fixture：两条仅返回类型不同的函数声明报错；默认参数导致调用点永远歧义的 overload 报错。
- 依赖：T0318、T0453、T1305
- 完成：
  - typecheck：新增 `check_file_overload_conflicts` pass，在 `check_file_type_refs` 之后、`check_file_exprs` 之前执行。
  - 诊断：新增稳定错误码 `scoop::typecheck::overload_conflict`，覆盖：
    - 同签名重复/不可区分（返回类型不参与重载决议）；
    - 仅返回类型不同；
    - 默认参数导致在“位置调用 + 尾部 default 省略”下出现不可消歧的 arity 重叠（先做最小分析）。
  - fixtures：
    - 新增 `tests/fixtures/typecheck/overload_conflict_return_type_only_is_error.scoop`
    - 新增 `tests/fixtures/typecheck/overload_conflict_default_param_is_error.scoop`
    - 更新 `tests/fixtures/typecheck/class_ctor_overload_ambiguous_default_is_error.scoop`：从调用点歧义升级为声明处冲突
    - 调整 ctor 相关 fixtures，避免“主构造器与次构造器同签名”导致的 now-illegal 声明（保持原测试目标不变）：
      - `tests/fixtures/typecheck/class_init_order_ok.scoop`
      - `tests/fixtures/typecheck/class_secondary_ctor_delegation_missing_is_error.scoop`
      - `tests/fixtures/typecheck/class_secondary_ctor_delegation_super_is_error.scoop`
  - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0458 [DONE] 泛型约束：`where` 子句的语义检查与满足性
- 描述：在类型检查阶段实现 `where` 子句：验证约束目标必须是当前声明的类型参数，约束本身合法，并在实例化/调用时检查实参是否满足约束。
- 目标：覆盖 spec 已定义的约束形式；不再把 `where` 仅当作语法占位。
- 验收：typecheck fixtures：满足约束的泛型调用通过，不满足时报清晰错误；冲突或重复约束能被诊断。
- 依赖：T0260、T0320、T0404、T0437
 - 完成：
   - typecheck：新增 `check_file_where_clauses` pass，对 `where` 子句做语义检查：
     - 约束目标必须属于**当前声明**的 type params（禁止 member fun 借用外层 type params）。
     - 重复约束诊断（稳定错误码 `scoop::typecheck::duplicate_where_constraint`）。
     - 多重 class-like 上界冲突诊断（稳定错误码 `scoop::typecheck::conflicting_where_constraints`）。
   - typecheck lowering：`TypeEnv` 收集类型声明的 `where` 约束；在 `TypeRef` lowering 的名义类型实例化处检查约束满足性（稳定错误码 `scoop::typecheck::where_constraint_not_satisfied`）。
   - 兼容：当 type args 仍为 `TypeKind::Param`（例如泛型声明内部的 `Box<T>`）时，把约束视作假设并跳过满足性检查（更完整的约束传播留给推断阶段）。
   - fixtures：
     - `tests/fixtures/typecheck/where_clause_satisfies_bound_ok.scoop`
     - `tests/fixtures/typecheck/where_clause_not_satisfied_is_error.scoop`
     - `tests/fixtures/typecheck/where_clause_duplicate_constraint_is_error.scoop`
     - `tests/fixtures/typecheck/where_clause_conflicting_class_bounds_is_error.scoop`
     - `tests/fixtures/typecheck/where_clause_target_not_in_current_decl_is_error.scoop`
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0459 [DONE] `when`：穷尽性检查补齐嵌套组合覆盖
- 描述：把穷尽性检查从“单层 enum/Bool/Option”推进到嵌套组合：tuple of enums、enum payload 中再含 enum/Bool/Option、以及多维组合覆盖。
- 目标：先覆盖有限且可枚举的组合；不做无限域或路径敏感分析。
- 验收：typecheck fixtures：`when ((opt, flag))`、嵌套 `Some((x, None))` 等场景在完整覆盖时通过，缺失某一组合时报错。
- 依赖：T0428、T0429、T0410
 - 完成：
   - typecheck：新增 `typecheck::when_exhaustiveness`，用“有限例子集合（example set）”递归枚举 Bool/Option/enum/tuple 的构造器组合，并用 arm patterns（忽略 guard）检查覆盖；对不可分析类型保持 `else/_/bind` 兜底规则。
   - fixtures：
     - `tests/fixtures/typecheck/when_tuple_exhaustive_option_bool_ok.scoop`
     - `tests/fixtures/typecheck/when_tuple_non_exhaustive_option_bool_missing_combo_is_error.scoop`
     - `tests/fixtures/typecheck/when_option_payload_nested_option_exhaustive_ok.scoop`
     - `tests/fixtures/typecheck/when_option_payload_nested_option_missing_combo_is_error.scoop`
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0460 [DONE] destructuring `val`：enum/variant payload 解构绑定
- 描述：把 variant pattern 复用到 `val` 解构绑定位置，允许对 enum payload 做绑定与类型检查，而不只支持 tuple/struct。
- 目标：先覆盖与 `when` pattern 同构的 variant payload 解构；不新增第二套独立语义。
- 验收：typecheck fixtures：在允许的绑定语法下对 `Some(x)` / `Result.Ok(v)` 形式解构成功；对非匹配 variant 或非法位置给出稳定诊断。
- 依赖：T0430、T0427、T0456
 - 完成：
   - ast/parser：
     - `Pattern` 增加 `Variant { path, args }`，并支持 `Name(...)`/`Enum.Variant(...)` 解析；
     - `val` 声明解析新增 `looks_like_variant_pattern_ahead` 分支以消歧 `val Name(...) = ...`。
   - resolve：`val` pattern 引入的局部绑定收集支持递归进入 variant args。
   - typecheck：`val_pat` 复用 `when` 的 enum variant destructuring 规则，新增稳定诊断：
     - `scoop::typecheck::val_variant_pat_not_enum`
     - `scoop::typecheck::val_variant_pat_unknown_variant`
     - `scoop::typecheck::val_variant_pat_arity_mismatch`
     - `scoop::typecheck::val_variant_pat_too_short`
     - `scoop::typecheck::val_variant_pat_enum_mismatch`
   - fixtures：
     - `tests/fixtures/typecheck/destructuring_val_variant_payload_ok.scoop`
     - `tests/fixtures/typecheck/destructuring_val_variant_not_enum_is_error.scoop`
     - `tests/fixtures/typecheck/destructuring_val_variant_unknown_variant_is_error.scoop`
     - `tests/fixtures/typecheck/destructuring_val_variant_enum_mismatch_is_error.scoop`
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

---

## T05：类型推断（阶段 4：约束生成与求解，逐步扩展）

### T0501 [DONE] 推断框架：引入 constraint 表示与求解器骨架（spec §14.9）
- 描述：为 `infer` 建立数据结构：`τ1 <: τ2`、相等、未知类型变量。
- 目标：先只实现相等约束与简单 unify；subtyping 后续任务。
- 验收：新增单测：`T = Int`、`T = String` 冲突时报错；`cargo test -p scoopc` 通过。
- 依赖：T0401
 - 完成：
   - 新增 `scoopc::infer` 模块：`InferVarId`/`InferTerm`/`Constraint`/`InferError` 与最小 `Solver`（union-find + concrete binding）。
   - 新增单测覆盖 “同一推断变量被绑定为两个不同 `TypeId` 时返回 `TypeConflict`”；`cargo test -p scoopc` 通过。

### T0502 [DONE] 局部变量推断：`val x = expr` 推断 x 类型（spec §14.3）
- 描述：当缺少类型注解时，从 initializer 推断类型。
- 目标：先只对字面量/简单表达式生效；复杂情况可要求注解并报“推断失败”。
- 验收：infer fixture：`val x = 1` 推断为 Int；`val x = if (...) 1 else 2` 也可。
- 依赖：T0405、T0501
 - 完成：
   - fixtures：实现 `infer` phase（当前复用 typecheck pipeline），使 `tests/fixtures/infer/**` 可回归。
   - typecheck：补齐 `if` 表达式的最小结果类型推导（同类型分支；无 `else` → `Unit`；不一致暂 fallback 为 `Any`）。
   - fixtures：新增 2 个 infer 用例覆盖 `val x = 1` 与 `val x = if (cond) 1 else 2` 的局部推断。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0503 [DONE] `if`/`when` 分支类型的 LUB 推断（spec §14.6）
- 描述：在推断阶段计算分支合并类型（先简化规则）。
- 目标：先支持相同类型与 Any fallback；后续再做真正 LUB/union。
- 验收：infer fixture：`val x = if (c) 1 else 2` → Int；`if (c) 1 else "s"` → Any。
- 依赖：T0414、T0502
 - 完成：
   - infer fixtures：补齐分支合并的回归用例（含 pass/fail）：
     - `tests/fixtures/infer/local_val_if_mixed_types_falls_back_to_any.scoop`
     - `tests/fixtures/infer/local_val_when_same_type_is_inferred.scoop`
     - `tests/fixtures/infer/local_val_when_mixed_types_falls_back_to_any.scoop`
   - typecheck：把 `Any` fallback 的 TODO 标号从 T0503 更新到 T0514（真正 LUB/union 任务）
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0504 [DONE] lambda 参数类型下推（spec §14.7.2）
- 描述：在调用点已知函数类型时，把参数类型下推到 lambda 参数。
- 目标：先只支持单参数 lambda；不支持多段推断链。
- 验收：infer fixture：`fun takes(f: (Int) -> Int) {}` + `takes { x -> x }` 通过并推断 x 为 Int。
- 依赖：T0219、T0222、T0501
 - 完成：
   - typecheck：在 `infer_expr_type_in_expected_context` 中实现 lambda expected type 传播：
     - 当 expected type 为 `(T) -> R`（无 receiver、单参数）时，把 `T` 下推到 lambda 形参；
     - 在 lambda body 语境注入形参类型并推导 body 类型作为返回类型（最小实现）。
   - typecheck：普通调用与扩展调用的“单候选路径”在检查实参时改为按形参类型做 expected-context 推导，
     使 `takes { x -> x + 1 }` 这类调用能正确推断 lambda 形参类型。
   - fixtures：新增 `tests/fixtures/infer/lambda_param_type_is_propagated_from_expected.scoop` 回归用例。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0505 [DONE] 泛型实参推断 v0（spec §14.5）
- 描述：从调用参数推断泛型实参（例如 `id(1)` 推断 `T=Int`）。
- 目标：先只做单个类型参数；不处理 variance 与 star projection。
- 验收：infer fixture：`fun id<T>(x: T): T {}` + `val a = id(1)` 推断返回 Int。
- 依赖：T0218、T0502
 - 完成：
   - typecheck：收集顶层函数签名时注入 `fun.type_params`（lowering 为 `TypeKind::Param`），并在签名中保留 type params 列表用于调用点实例化。
   - typecheck：在“单候选调用路径”实现泛型实参推断与 substitution：
     - 顶层普通调用：`f(args...)`
     - 扩展调用：`receiver.f(args...)` / `receiver?.f(args...)`
     - 推断策略：仅单一类型参数；按参数形状递归收集相等约束；lambda 预收集阶段的 `Any` 占位不参与推断。
   - typecheck：函数体类型检查时 push/pop `fun.type_params`，使 `return x: T` 等位置可正常 lowering。
   - infer fixtures：新增 `tests/fixtures/infer/generic_type_arg_is_inferred_from_call_arg.scoop` 回归用例。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0506 [DONE] 子类型约束与 subtype unification（spec §14.8）
- 描述：实现 `τ1 <: τ2` 约束的求解（先覆盖 Any/Option/tuple/function 的一小部分）。
- 目标：先不追求完整 Kotlin 子类型；先服务推断与错误信息。
- 验收：infer fixture：把 `Int` 赋给 `Any` 通过；把 `Any` 赋给 `Int` 失败并给出清晰诊断。
- 依赖：T0501、T0418
 - 完成：
   - infer：`Solver` 支持 `Constraint::Subtype`，并为推断变量引入 lower/upper bounds（最小实现：无共同超类型时 lower bounds LUB 退化到 `Any`）。
   - infer：实现最小 `is_subtype_of`（`Nothing <: T`、`T <: Any`、`Option/tuple/function` 的结构性递归，含函数参数逆变/返回协变与 effect subset）。
   - tests：新增 infer 单测覆盖 Any upcast / Option / tuple / function 的子类型规则与 bounds 求解。
   - fixtures：新增 infer fixtures：
     - `tests/fixtures/infer/subtyping_int_to_any_is_ok.scoop`
     - `tests/fixtures/infer/subtyping_any_to_int_is_error.scoop`
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0507 [DONE] 返回类型推断（spec §14.6）
- 描述：当函数缺少返回类型注解时，从 `return`/最后表达式推断返回类型。
- 目标：先只支持单一 return 路径；多路径合并后续。
- 验收：infer fixture：`fun f(){ 1 }` 推断为 Int（或要求显式 `: Int`，需决定并固定规则）。
- 依赖：T0226、T0502
 - 完成：
   - typecheck：为未标注 `return type` 的函数引入最小返回类型推断：
     - 无 top-level `return`：尝试从函数体最后一条“表达式语句”的类型推断返回类型；
     - 单一 top-level `return` 且位于函数体最后：从 `return` 的值推断返回类型；
     - 其它情况暂不推断，保持旧行为（默认视为 `Unit`，多路径合并留到后续任务）。
   - typecheck：推断成功后回写顶层函数签名表，使得同文件后续调用点能看到推断后的返回类型。
   - fixtures：新增 `tests/fixtures/infer/fun_return_type_is_inferred_from_tail_expr.scoop` 回归用例。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

> 注：T0508～T0515（effect row 推断/诊断/联动）依赖 T0604（required effects 检查）。
> 为保证 TODO 顺序“首个 `[TODO]` 可直接实现”，已将这些任务移动到 T0604 之后。

---

## T06：效果系统（阶段 5：先静态，再逐步落地运行时）

### T0601 [DONE] Parser：`effect` 声明体内的操作签名（spec §5.2）
- 描述：在 effect type body 内解析 `fun op(args): Ret` 列表，并区分 effect operation 与普通方法。
- 目标：先不解析实现体（operation 应无 body）；不支持默认实现。
- 验收：parse fixture：`effect Raise<E> { fun raise(error: E): Nothing }` 能解析出 operation 列表。
- 依赖：T0203、T0218
 - 完成：
   - AST：为 `FunDecl` 增加 `kind: FunDeclKind`，并在 effect body 内将 `fun` 标记为 `EffectOp`（用于区分 operation 与普通方法）。
   - parser：effect body 内解析 operation 签名；若意外出现 `{ ... }` body，则记录诊断并跳过该分组（不支持默认实现）。
   - fixtures：新增 parse fixture `tests/fixtures/parse/effect_op_decl_basic.scoop` + AST golden；更新 `type_member_nested_type` golden 覆盖 effect nested type 场景。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0602 [DONE] Typecheck：effect operation 的类型规则与命名空间
- 描述：把 effect 看作“可 perform 的接口”，operation 生成对应的 perform signature。
- 目标：先只支持 sysroot 的 `Raise`；不做 effect polymorphism。
- 验收：typecheck fixture：`Raise.raise(e)`（按语法）能通过（或暂以 `perform Raise.raise(e)` 为准，按 spec）。
- 依赖：T0601、T0404
 - 完成：
   - resolve/index：为 `resolve::FunSig` 补齐 `kind: FunDeclKind`，让 resolver/typecheck 能区分 effect operation 与普通 member fun。
   - resolver：允许 `EffectName.op` 直接解析到 effect body 内的 operation（不经 companion object），并写回 `MemberIdent.resolved = Fun { fqn }`。
   - typecheck：为 member call 增加 effect operation 特判，lower operation 的签名并复用既有的单一 type param 推断（`Raise<E>`）以支持 `Raise.raise(e)` 调用。
   - fixtures：新增 `tests/fixtures/typecheck/effect_op_raise_call_ok.scoop` 回归用例。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0603 [DONE] Parser：函数/函数类型上的 effect row `/ RowExpr`（spec §5.8、§7.5）
- 描述：在声明与类型位置支持 `/ Pure` 与 `/ E1+E2`。
- 目标：RowExpr 先只支持 `Pure`、单个 effect 名、`+` 并集。
- 验收：parse fixture：`fun f(): Int / Pure { ... }`（或无 body）能解析。
- 依赖：T0219
 - 完成：
   - fixtures：新增 parse fixture `tests/fixtures/parse/fun_decl_effect_row_basic.scoop` + AST golden，覆盖函数声明上的 `/ Pure`、`/ E1+E2` 与括号形式。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0604 [DONE] Typecheck：required effects 检查（spec §14.7.1）
- 描述：当函数体 perform 了 effect，但未在函数签名 row 中声明，也未被 handle，则报错。
- 目标：先只覆盖 `Raise`；先不实现 handler，允许“显式声明 row”。
- 验收：effects fixture：`fun f() { Raise.raise(e) }` 在 `/ Pure` 下失败；在 `/ Raise<...>`（按语法）下通过。
- 依赖：T0602、T0603
 - 完成：
   - typecheck：在函数体 typecheck 期间收集 effect op call（当前为 `Raise.raise(...)`），并与函数签名的 effect row 做包含性检查；未写 `/ RowExpr` 时默认视为 `Pure`。
   - expr stmt：call 作为表达式语句时也会对 effect op call 做最小调用检查，以便记录 required effects（避免被“语句层不完整 typecheck”跳过）。
   - diagnostics：新增稳定错误码 `scoop::typecheck::required_effect_not_declared`。
   - fixtures：更新 `tests/fixtures/typecheck/effect_op_raise_call_ok.scoop` 增加显式 row；新增 required effects pass/fail fixtures 覆盖 `/ Pure` 与 `/ (Raise<Int>)`。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0508 [DONE] effect 推断 v0：private/internal 可推断 row，public 默认 Pure（PLAN §6.1）
- 描述：实现 effect row 的推断入口：public 函数默认 `/ Pure`（强制），private/internal 可推断。
- 目标：先只覆盖单文件；跨包 API 后续与 Cone 联动。
- 验收：effects fixture：public 函数 perform Raise 时报错；private 函数可推断出需要 Raise（或在 dump-hir 中可见）。
- 依赖：T0604、T0245
 - 完成：
   - typecheck：在 `/ RowExpr` 缺省时按可见性选择策略：public 强制 `Pure`，private/internal 从函数体内收集到的 performed effects 推断 effect row。
   - fixtures：新增 public 缺省 row 的 compile-fail，以及 private 缺省 row 的 compile-pass 回归用例。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0509 [DONE] effect row 参数 `eff` 推断（spec §14.7.3 / §3.4）
- 描述：实现 `eff E = Pure` 这种 row type parameter 的推断/实例化规则。
- 目标：先只支持默认值与 `E1+E2` 实参；不做高阶 row 运算。
- 验收：effects fixture：`fun <eff E = Pure> ... / E` 在调用点省略 row 参数可推断为 Pure。
- 依赖：T0250、T0603、T0508
 - 完成：
   - resolve：允许在 effect row 中引用 `eff` 参数名（例如 `/ E`），避免把 `E` 当作普通类型去解析。
   - typecheck/lower：引入 effect row 参数作用域，将 `/ E` 展开为其绑定的 row（默认值缺省为 `Pure`）。
   - typecheck/expr：从 lambda body 收集到的 performed effects 推断 `E`，并把被调用函数的 required effects 计入外层函数体的 performed effects。
   - typecheck/overloads：重载冲突检查阶段在 lowering 签名时同样注入 `eff` 参数绑定，避免 `E` 未解析错误。
   - fixtures：新增 `eff` 默认 Pure、从 lambda 推断、以及“推断后外层缺少效果声明”的 fail 用例覆盖。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0510 [DONE] 推断失败诊断：最小可读解释（spec §14.7.4）
- 描述：把“推断失败”映射到具体 span，并给出最小解释（期望类型/实际类型/约束来源）。
- 目标：先覆盖常见：参数不匹配、分支类型不一致、lambda 参数推断失败。
- 验收：新增 infer-fail fixtures：每个错误都断言错误码 + 关键提示子串。
- 依赖：T0005、T0501
 - 完成：
   - diagnostics：泛型实参推断冲突 `generic_type_arg_inference_conflict` 增强为“带约束来源”，并把 primary span 定位到产生冲突的实参；
   - diagnostics：`if` 在 expected type 语境下对 then/else 分支分别做可赋值检查，新增稳定错误码 `scoop::typecheck::if_branch_type_mismatch`；
   - diagnostics：lambda 缺少 expected function type 且参数无类型注解时，新增稳定错误码 `scoop::typecheck::lambda_param_type_not_inferred`；
   - fixtures：新增 3 个 infer-fail 用例断言错误码 + `约束来源` 子串 + 精确位置；
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0511 [DONE] use-site `eff` row 参数：默认值与显式实参推断（spec §3.4 / §14.7.3）
- 描述：当类型或调用点使用 `Type<eff Row>` 时，支持 effect row 默认值、显式实参和由上下文/lambda body 反推的 row 参数实例化。
- 目标：先覆盖“单个 row 参数 + 默认值 + 简单并集”的场景；高阶 row 约束后续。
- 验收：effects/infer fixture：`Disposable` 省略 use-site row 时默认到 `Pure`；显式 `Disposable<eff Async>` 可参与调用检查。
- 依赖：T0253、T0509、T0603
 - 完成：
   - type env：为 `TypeSymbol` 增加 `eff_param`（name/default/span/decl_file），供 use-site 默认值按声明处上下文计算。
   - type lowering：支持 `Type<eff Row>`：
     - 显式 `eff Row` lowering 为 `EffectRow`；
     - 省略时使用声明处默认值（缺省为 `Pure`）；
     - 将 effect row 实参纳入 `NominalType` identity，并在不支持的类型上给出稳定诊断。
   - ty/display/assignable：`NominalType` 新增 `eff` 字段并在显示中输出 `<eff ...>`；`is_type_assignable` 与推断结构遍历按 `eff` 区分。
   - fixtures：新增 `tests/fixtures/infer/effects/*` 覆盖默认 `Pure` 与 receiver mismatch 诊断。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0512 [DONE] overload resolution 与泛型/默认参数/命名参数/row 推断联动
- 描述：让 overload resolution 不只依赖显式类型，还能与泛型实参推断、默认参数、命名参数、effect row 参数、receiver function type 一起求解候选。
- 目标：先覆盖“先过滤候选，再对剩余候选尝试推断”的两阶段策略；更激进的联合求解后续再优化。
- 验收：infer fixture：泛型 overload 能按实参推断出正确候选；默认参数与命名参数参与后可消除歧义；effect row 也能影响候选选择。
- 依赖：T0453、T0454、T0455、T0505、T0509
 - 完成：
   - typecheck/expr：顶层函数调用与扩展函数调用的多候选重载决议升级为“两阶段”：
     - 先做候选过滤：args→params 映射（支持默认参数 + 命名参数）；
     - 再对剩余候选尝试：泛型实例化（T0505）、lambda expected-context 推导（T0504）、`eff` row 推断与 substitution（T0509）。
   - typecheck/expr：单候选路径同样接入默认参数映射（当前只做可用性/类型检查；默认值表达式的补齐语义留给后续任务 T1305）。
   - diagnostics：多候选路径在选出唯一候选后，会按实例化后的 type args 与 `eff` row 记录 call required effects（避免之前“重载命中但 effects 未记账”的漏报）。
   - fixtures：新增 `tests/fixtures/infer/overload_resolution_inference_defaults_and_effects_ok.scoop` 覆盖：
     - 泛型候选可在多候选中通过实参推断被选中；
     - 命名参数 + 默认参数共同参与候选可用性判定；
     - `eff` row 推断可影响候选筛选（lambda effects 与固定 effect row 的可赋值关系）。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0513 [DONE] 最具体候选（most specific candidate）与歧义诊断
- 描述：在多个候选都可行时，实现 Kotlin-like most-specific candidate 规则，并给出稳定、可解释的歧义诊断。
- 目标：先覆盖参数更具体、receiver 更具体、非默认参数优先等常见规则；完全等价时保留歧义错误。
- 验收：infer fixture：`f(1)` 在 `f(Int)` / `f(Any)` 中选 `f(Int)`；无明显更具体候选时报 `ambiguous_overload`，错误信息列出候选签名。
- 依赖：T0512、T0457
 - 完成：
   - typecheck/expr：direct call 的多候选重载决议引入 most-specific 选择：
     - 参数类型按 `is_type_assignable` 做 strict-more-specific 判定；
     - tie-break：默认参数使用更少者优先（“非默认参数优先”）。
   - typecheck/expr：扩展函数（`receiver.member(...)`）多候选同样接入默认参数 tie-break。
   - diagnostics：`ambiguous_overload` 错误信息附带候选签名列表（稳定排序）。
   - fixtures：新增 infer pass/fail 用例覆盖 most-specific 与歧义候选列表断言。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0514 [DONE] 分支类型合并：真正的 LUB / 受限 union 构造与化简
- 描述：替换当前 `if/when` 的 “相同类型否则回退到 Any” 规则，引入真正的最小上界计算，并在必要时构造受限 union/公共超类型结果。
- 目标：先覆盖最常见层级：继承关系、nullability、`Nothing`、enum/tuple/函数类型的可比较情况；保证结果稳定可解释。
- 验收：infer fixtures：`if (c) child else parent` 推断为 `Parent`；`if (c) Int else String` 不再无脑退化，按既定 union/LUB 规则给出结果或稳定诊断。
- 依赖：T0414、T0503、T0506
 - 完成：
   - ty：新增受限 union 类型表示 `A | B | ...`（稳定排序/去重/展平，`Nothing` 消去，`Any` 吸收）。
   - typecheck：新增 `branch_merge` 模块，实现 if/when 分支类型合并：
     - 优先使用子类型关系与名义继承的公共超类型（避免过早退化为 `Any`）；
     - 结构类型覆盖 `Option`/tuple/function 的“可比较情况”；
     - 无合适公共超类型时构造 union 作为保守精化。
   - typecheck/expr：`if` 与 `when` 的结果类型推断改为统一走 `branch_merge::merge_branch_result_type`。
   - fixtures：
     - 更新 `infer/*mixed_types*`：不再断言 `Any`，改为断言 `Int | String`；
     - 新增 `infer/if_branch_lub_inheritance_ok.scoop` 覆盖 Child/Parent → Parent。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0515 [DONE] effect row 推断 v1：高阶 row 约束、泛型 row 变量与归一化
- 描述：把 effect row 推断从“默认值 + 简单并集”推进到更完整的求解：支持泛型 row 变量参与约束、必要的高阶 row 运算，以及规范化后的等价比较。
- 目标：让 row 推断可稳定服务于 overload resolution、receiver function type 与 higher-order APIs，而不是停留在简单集合拼接。
- 验收：effects/infer fixtures：高阶函数把 row 变量透传给返回函数类型时仍可推断；等价但书写顺序不同的 row 表达式不会导致误判。
- 依赖：T0509、T0511、T0608
 - 完成：
   - typecheck/expr：当 `<eff E>` 被用于返回函数类型（`...: (...) -> T / E`）时，调用点推断出的 `E` 会回填到 call result 的 function type effects（直调与扩展调用均覆盖），避免返回类型停留在声明处默认 `/ Pure`。
   - fixtures：新增 `tests/fixtures/infer/effects/eff_row_higher_order_return_infers_ok.scoop` 覆盖“高阶返回透传”。
   - fixtures：新增 `tests/fixtures/infer/effects/eff_row_order_is_normalized_ok.scoop` 覆盖 row 表达式顺序归一化（`Foo+Bar` ≡ `Bar+Foo`）。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0605 [DONE] Parser：`handle { ... } with { ... }`（spec §5.4）
- 描述：解析 handle 表达式与 handler arms（先支持 non-resuming `->` 一种 arm）。
- 目标：先不实现 `-> resume` 与 `, k ->`；arm body 可用 block 表达式。
- 验收：parse fixture：最小 handle 示例可解析；语法错误能恢复到下一个 arm。
- 依赖：T0214、T0207
 - 完成：
   - ast：新增 `ExprKind::Handle { body, arms, finally }` 与 `HandleArm/HandleOp/HandleBinder` 语法建模（non-resuming `->`）。
   - parser：支持 `handle { ... } with { ... }` 解析；arm head 解析为 `Effect.op(binders...)`（binder 支持可选 `: Type`）。
   - parser：arm 级错误恢复：在单个 arm 语法错误后同步到下一个 `Effect.op(...) ->` 起始继续解析，避免级联。
   - parser：`-> resume { ... }` 与 `, k ->` 形态当前报错（保持 TODO 目标“不实现”）。
   - fixtures：新增 parse pass fixture + AST snapshot；新增 parse fail fixture 覆盖“两处错误但可恢复到第二个 arm”。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0606 [DONE] Typecheck：handler arms 的类型规则（non-resuming）
- 描述：校验 arm 的参数类型、返回类型、以及 handle 表达式整体类型。
- 目标：先只实现 “处理 Raise” 的 try/catch 等价形式；不实现 continuation 类型。
- 验收：effects fixture：`handle { ... } with { Raise.raise(e) -> ... }` 类型正确；错误 arm 返回类型不匹配时报错。
- 依赖：T0605、T0604
 - 完成：
   - typecheck/expr：支持 `ExprKind::Handle` 的类型检查与结果类型推导（non-resuming `->`）。
   - typecheck/expr：handler 捕获 required effects：handle body 内 performed 的 effect 若被匹配 arm 捕获，则不向外层传播（/ Pure 下可通过）。
   - typecheck/expr：handler arm head 解析 effect op 签名并校验 binder arity/类型（对齐 effect op 形参类型）。
   - diagnostics：新增稳定错误码 `scoop::typecheck::handle_arm_return_type_mismatch`（arm body 返回类型不一致）。
   - fixtures：新增 typecheck pass/fail fixtures 覆盖：
     - `Raise.raise` 在 handle 中被捕获后不再要求声明 effect；
     - handler arm 返回类型不匹配时报错。
   - 验收：`cargo test --all` 与 `cargo run -p scoop -- test` 通过。

### T0607 [DONE] 语法糖：`try/catch/finally` → `handle`（spec §5.7）
- 描述：在 parser 层支持 `try { } catch (e: T) { } finally { }` 并 lowering 到 handle AST。
- 目标：先只支持单个 catch；finally 可选；不支持多 catch。
- 验收：parse fixture：try/catch/finally 可解析并 lowering；typecheck fixture：对应的 Raise 处理不触发 required effects。
- 依赖：T0605、T0606
 - 完成：
   - ast：`Ident` 支持“合成文本”（用于语法糖节点不来自源文本的场景）。
   - parser/expr：支持 `try { ... } catch (e: T) { ... } finally { ... }` 并 lowering 为 `ExprKind::Handle`（固定捕获 `scoop.core.Raise.raise`）。
   - typecheck：`TypeLowering::resolve_type_path_fqn` 与 handle arm callee 解析支持读取合成 Ident 文本，保证 lowering 后可正常解析到 sysroot effect op。
   - fixtures：
     - parse：`tests/fixtures/parse/try_catch_finally_lowering.scoop` + AST snapshot；
     - typecheck：`tests/fixtures/typecheck/try_catch_catches_required_effects_ok.scoop`。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

> 注：以下两项编号属于 T04（类型系统/类型检查），但依赖效果系统的 required effects + try/catch lowering（T0604/T0607），因此放在此处以保证 TODO 的依赖顺序（避免在 effect 体系落地前被选中执行）。

### T0421b [DONE] `!!`：required effect `Raise<RuntimeError>`（Appendix B.3.3）
- 描述：`x!!`：触发运行期 null assertion，要求 `Raise<RuntimeError>`（除非被 handle/try 处理）。
- 目标：先只实现静态 required effects；运行期行为后续由 effect/runtime 落地。
- 验收：effects fixture：在 `/ Pure` 的函数里使用 `x!!` 报 required effect；在 try/catch 内通过。
- 依赖：T0421a、T0419、T0604、T0607
 - 完成：
   - typecheck/expr：`ExprKind::NotNullAssert` 记录 performed effect `Raise<RuntimeError>`（静态 required effects）；并在表达式语句位置同样触发（避免遗漏）。
   - fixtures：新增
     - `tests/fixtures/typecheck/not_null_assert_required_effect_missing_is_error.scoop`（Pure 函数内使用 `!!` 报错）
     - `tests/fixtures/typecheck/not_null_assert_required_effect_in_try_catch_ok.scoop`（try/catch 捕获后通过）
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0445 [DONE] `as` 失败语义：要求 `Raise<RuntimeError>`（spec §4.4）
- 描述：当使用不安全 cast `x as T` 时，编译器应把其失败语义建模为 `Raise.raise(RuntimeError.ClassCastFailed)`，因此要求 `Raise<RuntimeError>` 除非被 handle/try 捕获。
- 目标：先只做静态 required-effects 检查；运行期失败触发 raise 的 codegen 后置（与 T0614/T0818 联动）。
- 验收：effects fixture：在 `/ Pure` 函数中使用 `as` 报 required effect；在 try/catch 内通过。
- 依赖：T0412、T0419、T0604、T0607
 - 完成：
   - typecheck/expr：`ExprKind::Cast` 在 `as` 分支记录 performed effect `Raise<RuntimeError>`（静态 required effects）。
   - typecheck/expr：表达式语句位置同样会 typecheck `Cast`，避免 required-effects 收集遗漏。
   - fixtures：新增
     - `tests/fixtures/typecheck/cast_as_required_effect_missing_is_error.scoop`（Pure 函数内使用 `as` 报错）
     - `tests/fixtures/typecheck/cast_as_required_effect_in_try_catch_ok.scoop`（try/catch 捕获后通过）
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0608 [DONE] RowExpr 静态语义：`Pure`/`+`/默认 effect/containment（spec §5.8）
- 描述：实现 effect row 的语义：并集、空行 `Pure`、默认 effect 规则、以及 `R1 ⊆ R2`（subeffecting）的最小判定。
- 目标：先把 row 当作“集合”；不实现高级归一化；泛型 row 变量后续任务补。
- 验收：effects fixture：`/ Pure` 可赋给 `/ Pure`；`/ Raise` 不能赋给 `/ Pure`；`/ Pure` 可视为 `/` 空行。
- 依赖：T0603
 - 完成：
   - ty：为 `EffectRow` 的集合归一化与 `R1 ⊆ R2` 判定补齐单测覆盖（Pure/containment）。
   - fixtures：新增 `tests/fixtures/typecheck/function_type_effect_row_default_pure_ok.scoop` 覆盖“省略 `/ RowExpr` 等价于 `/ Pure` 空行”。
   - fixtures：结合既有
     - `tests/fixtures/typecheck/function_type_subtyping_ok.scoop`（`Pure ⊆ R`）
     - `tests/fixtures/typecheck/function_type_effect_row_not_contained_is_error.scoop`（`R ⊄ Pure`）
     完成回归矩阵。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0609 [DONE] effect polymorphism：`eff` row 参数与 overriding 规则（spec §5.9）
- 描述：支持 `<eff E = Pure>` 这类 row 参数，并实现 overriding：`R_over ⊆ R_base`。
- 目标：先只对 member override 做静态检查；不做动态 dispatch。
- 验收：effects fixture：override 方法的 row 超集时报错；row 子集允许。
- 依赖：T0509、T0608
 - 完成：
   - typecheck：新增 pass `override_effects`，对 class/object 的 `override fun` 与 interface 抽象方法实现执行 effect row containment 检查（`R_over ⊆ R_base`），并对 receiver 的 use-site `Type<...>` 与 `Type<eff ...>` 做 substitution 后再比较。
   - typecheck/lower：补齐 `lower_effect_row_expr_in_decl_file_with_scopes`，支持在声明处文件上下文同时注入 type param 与 effect row param 绑定。
   - fixtures：新增 4 个用例覆盖 class override 与 interface impl 的 pass/fail（含 `Disposable<eff E>` 的实例化替换）。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0610 [DONE] Program boundary：entry point 必须 `Pure!`（闭合纯；spec §5.10）
- 描述：定义 entry point（例如 `fun main()`）并强制其 effect row 为 `Pure!`（闭合）。entry point 允许在函数体内使用 effects，但必须在 entry point 内部被处理（不能把未处理 effect 泄漏到 program boundary）。
- 目标：先只检查 `main`；多 entry point（库）后续。
- 验收：effects fixture：`main` 里 perform Raise（未处理）时报错；`main` 使用 try/catch 处理后通过。
- 依赖：T0604、T0607
 - 完成：
   - typecheck/expr：对 entry point（顶层 `fun main()`）强制 declared effect row 为 `Pure!`，禁止 internal/private 的 effect row 推断；显式声明 non-Pure effect row 时给出稳定诊断 `scoop::typecheck::entry_point_must_be_pure`，显式声明 open `/ Pure` 时给出“需要闭合 `Pure!`”的稳定诊断（见 T0627）。
   - fixtures：新增
     - `tests/fixtures/typecheck/entry_point_main_internal_unhandled_raise_is_error.scoop`
     - `tests/fixtures/typecheck/entry_point_main_internal_try_catch_ok.scoop`
     - `tests/fixtures/typecheck/entry_point_main_explicit_non_pure_effect_row_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0611 [DONE] Continuation 类型建模（spec §5.5）
- 描述：在类型系统中加入 `Continuation<T, /E>`（或等价表示），并把 `resume(value)` 的类型规则固定下来。
- 目标：先只建类型与 typecheck 规则；不做 codegen。
- 验收：新增 typecheck fixture：`k: Continuation<Int, /Pure>` 的参数/返回类型检查正确；`resume` 多次调用的静态限制可先不做。
- 依赖：T0609、T0435
 - 完成：
   - sysroot：新增 `scoop.core.Continuation<T, eff E = Pure>` 声明，作为 `Continuation<T, /E>` 的等价表示（`E` 为 required effects）。
   - typecheck/expr：支持 `k.resume(value)` 的内建类型规则：检查 `value` 可赋值到 `T`，并把 `E` 计入 required effects（safe-call 返回 `Option<Unit>`）。
   - typecheck/lower：把 `Continuation` 纳入隐式 builtin type 名称映射（允许在 type position 直接写 `Continuation`）。
   - fixtures：新增
     - `tests/fixtures/typecheck/continuation_type_and_resume_pure_ok.scoop`
     - `tests/fixtures/typecheck/continuation_resume_required_effect_missing_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。
### T0624 [DONE] effect rows：use-site `Type<eff Row>` 的实例化与检查（spec §3.4 / §5.8）
- 描述：在类型检查阶段支持 `Type<eff Row>` 的显式实参与默认化，并与 overriding、required effects、subeffecting 联动。
- 目标：先只支持单个 row 参数；语法合法性由 parser 先行保证。
- 验收：effects fixture：`Disposable<eff Async>` 调用需要 Async；`Disposable` 省略时默认 `Pure`；非法多 `eff` clause 报错。
- 依赖：T0253、T0609、T0511
 - 完成：
   - typecheck/assignable：名义类型的 `eff` row 参数参与 subeffecting：`Type<eff R1> <: Type<eff R2>` 当且仅当 `R1 ⊆ R2`。
   - typecheck/expr：`eff` row 参数推断扩展为“默认值 + lambda body + `Type<eff E>` 实参类型”联合推断，并在调用点把实例化后的 row 回填到形参/返回类型。
   - typecheck/expr：修复/补齐 substitution：名义类型 `NominalType.eff` 在 type arg substitution 时保持并参与递归替换（例如 `Raise<T>` 出现在 row terms）。
   - typecheck/expr：扩展函数重载决议中，候选的 `receiver_ty` 使用每个候选自身的实例化结果（避免错误把第一个候选的 receiver 用于所有候选的 specificity 比较）。
   - fixtures：新增/更新
     - `tests/fixtures/typecheck/eff_row_param_infer_from_nominal_missing_is_error.scoop`
     - `tests/fixtures/typecheck/eff_row_param_infer_from_nominal_ok.scoop`
     - `tests/fixtures/parse/effect_row_param_duplicate_fail.scoop`
     - `tests/fixtures/infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0626 [DONE] 闭合 effect row 语法解析：`/ R!`（spec §5.8.4）
- 描述：在 lexer/parser 中支持 effect row 的 `!` 后缀，将其解析为 `RowExpr::Closed(inner)`（或在现有 `RowExpr` 上加 `closed: bool` 字段）。`!` 的优先级低于 `+`，作用于整个 row，不与最后一个 effect 单独绑定。
- 目标：只做解析与 AST 表示；类型检查语义留给 T0627。不修改现有 open row 的解析行为。
- 验收：新增 parse pass fixtures：`fun f(): Unit / Pure!`、`fun f(): Unit / IO+State!`、`fun f(): Unit / (IO)!`；新增 parse fail fixture：`fun f(): Unit / !IO`（前缀形式不支持）。`scoop test` 通过。
- 依赖：T0219
 - 完成：
   - AST：`EffectRowExpr` 新增 `closed: bool`，并通过自定义 `Debug` 保持 open row 的 AST snapshot 输出不变。
   - parser：`parse_effect_row_expr` 支持后缀 `!`，并保证其优先级低于 `+`（作用于整个 row）。
   - fixtures：
     - pass：`tests/fixtures/parse/effect_row_closed_suffix.scoop` + `tests/fixtures/parse/effect_row_closed_suffix.ast`
     - fail：`tests/fixtures/parse/effect_row_closed_prefix_not_allowed_fail.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0627 [DONE] 闭合 effect row 类型检查语义（spec §5.8.4）
- 描述：实现闭合行的额外约束：对于 `fun f(...): R / E!`，编译器需确认所有来源的 effect（包括 callback 参数透传的 effect）不能逃逸函数边界——即 body 内所有路径的 effect 必须被 handle 或满足 `⊆ E`，且不存在自由 row 变量使 effect 集合扩大。
- 目标：先只覆盖 `Pure!`（最常见情况，等价”无任何 effect 逃逸”）；泛型 row 变量与闭合行的交互（`E!` 与 `<eff E>` 组合）后续任务补齐。
- 验收：effects fixture：`fun main(): Unit / Pure!` 内未处理的 `Raise` 报错；使用 `try/catch` 包裹后通过；open row `/ Pure` 在相同情况下报错信息不同（提示”需要闭合 row”）。
- 依赖：T0626、T0608、T0610
 - 完成：
   - typecheck：entry point 显式写 open row `/ Pure` 时，给出“需要闭合 row `Pure!`”的稳定诊断（`scoop::typecheck::entry_point_must_be_closed_pure`）。
   - fixtures：新增
     - `tests/fixtures/typecheck/entry_point_main_closed_pure_unhandled_raise_is_error.scoop`
     - `tests/fixtures/typecheck/entry_point_main_closed_pure_try_catch_ok.scoop`
     - `tests/fixtures/typecheck/entry_point_main_open_pure_needs_closed_row_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0628 RowExpr 高级语义（拆分为子任务）
- 描述：补齐 row 语义层而不只靠推断兜底：定义 row 表达式的规范化、等价判定、泛型 row 变量的合法出现位置，以及 spec 允许的高阶 row 运算。
- 目标：给 typecheck / infer / overload resolution 一个统一的 row 代数基础；不再把复杂 row 视为“字符串相等”或简单集合并。
- 备注：该任务涉及 typecheck/overload/推断多个路径，跨度较大。为保证每步“可单独实现 & 单独验证”，拆分为以下子任务。

### T0628a [DONE] RowExpr：支持 `E + ...` 的实例化/推断（函数类型 `/ Row` + use-site `Type<eff Row>`）
- 描述：把当前只识别 `(...)->T / E` 与 `Type<eff E>` 的最小实现扩展到 `E + R` 形式：调用点可从 lambda effects / `Type<eff ...>` 实参中推断 `E`，并把 `E` 的实例化结果回填到期望类型与返回类型中（避免默认值导致的误判）。
- 目标：仍只支持单一 `eff` 变量；只处理 `+` 并集（集合语义）；先覆盖“形参类型的顶层 function type / 顶层 nominal type”两类位置。
- 验收：新增 infer/effects fixtures：
  - `(...)->T / (E + R)`：lambda 产生的 effects 若超出 `R`，会被推断进 `E`，且调用通过；
  - `Type<eff (E + R)>`：从实参类型提取 row 约束时，会扣除 `R` 后再推断进 `E`，且调用通过；
  - `E + R` 与 `R + E` 书写顺序不同不影响推断与可赋值检查（归一化）。
- 依赖：T0608、T0609、T0515
 - 完成：
   - typecheck/expr：把签名中“引用 `eff` 变量”的识别从仅支持 `E` 扩展到 `E + R`：
     - 对函数类型 `(...)->T / Row` 记录 base row（移除 `E` 后的常量项）；
     - 对 use-site `Type<eff Row>` 同样记录 base row（移除 `E` 后的常量项）。
   - typecheck/expr：调用点推断 `E` 时按 `found ⊆ (E + base)` 提取最小增量 `found - base`，并把实例化后的
     `E + base` 回填到期望类型与返回类型中（覆盖 direct call 与 member/extension call 两条路径）。
   - fixtures：新增
     - `tests/fixtures/infer/effects/eff_row_fn_type_e_plus_base_infers_ok.scoop`
     - `tests/fixtures/infer/effects/eff_row_nominal_eff_arg_e_plus_base_infers_ok.scoop`
     覆盖函数类型与名义类型两类 `E + R` 形式，并验证顺序归一化不影响推断。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0628b [DONE] RowExpr：`E + ...` 在嵌套类型中的替换与逃逸诊断
- 描述：把 `E + R` 的实例化/替换从“顶层参数类型”扩展到更一般的嵌套位置（tuple/union/Option/多层 function type），并补齐“row 变量逃逸/不良组合”的稳定诊断（例如 closed row 与泛型 row 的交互）。
- 目标：实现一个可复用的“在 TypeId 里替换 row”遍历器；避免在多个 call-site 路径里复制逻辑。
- 验收：新增 typecheck fixtures 覆盖“嵌套类型中的 `E + R` 仍可正确实例化/检查”；新增至少 1 个 fail fixture 覆盖稳定错误码。
- 依赖：T0628a
 - 完成：
   - typecheck：新增 `scoopc::typecheck::eff_row_subst`，基于 `EffRowVarSubstPlan` 对 `TypeId` 做结构化重建，
     支持在 tuple/Option/多层 function type/nominal args 中把 `E + base` 替换为调用点的 `E_arg + base`。
   - typecheck：调用点统一入口 `instantiate_eff_row_var_in_sig_types`，避免 direct call / extension call / overload paths 重复替换逻辑。
   - diagnostics：`TypeLowerError` 新增 `scoop::typecheck::closed_effect_row_contains_row_var`，禁止闭合 row（`E!`）引用 row 变量。
   - fixtures：新增
     - `tests/fixtures/typecheck/eff_row_param_nested_e_plus_base_subst_ok.scoop`
     - `tests/fixtures/typecheck/closed_effect_row_contains_row_var_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0629 Program boundary（拆分为子任务）
- 描述：把 program boundary 从“仅检查 `main` 必须 `Pure!`”扩展到库导出入口、多 entry point、host callback / embedded 入口等场景，并固定哪些边界必须显式为 `Pure!`。
- 目标：先覆盖可静态识别的入口面；不引入运行时动态扫描。
- 备注：该任务依赖 build/link 与多包消费链路（T1107），跨度较大。为保证每步“可单独实现 & 单独验证”，拆分为以下子任务。

### T0629a [DONE] entry point 识别：cone-aware + 多 entry point 稳定规则
- 描述：在多 cone 编译单元中，仅将“consumer cone”的 `fun main` 视为 entry point；其它 cone 中同名 `main` 作为普通函数处理（不强制 `Pure!`）。
- 目标：先只做静态判定（基于 cone id）；不引入 `Cone.toml` 或 CLI 选择 entry 的参数。
- 验收：新增 typecheck + cone fixtures：
  - 同一用例内 app/lib 两个 cone 都有 `main` 时，仅 app 的 `main` 受 entry point `Pure!` 规则约束；
  - library cone 的 `public` API 若含未处理 effect（例如漏写 `/ Raise<...>`）会在 typecheck 阶段被拒绝。
- 依赖：T0610、T0321a（cone 可见性基础设施）
 - 完成：
   - resolve：新增 `Index::consumer_cone()`，用于在多 cone 编译单元中稳定识别 consumer cone。
   - typecheck：entry point 判定增加 cone 过滤，仅 consumer cone 的 `main` 视为 entry point。
   - fixtures：`scoop test` 新增 `typecheck_cone` runner，并新增用例 `tests/fixtures/typecheck_cone/program_boundary_multi_entry_points/**` 覆盖多 entry point 与库 public API 的 effect 门禁。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

> 注：T0629b 依赖多包 build/link（T1107），已移动到 T1107 之后以保持 TODO 顺序可执行。

### T0631 [DONE] 语法糖：多 `catch` arms 与匹配顺序
- 描述：把 `try/catch/finally` 从单个 `catch` 扩展到多个 `catch` arm，并固定匹配顺序、不可达分支诊断与 lowering 结果。
- 目标：先覆盖异常/错误类型匹配顺序；不做复杂模式匹配。
- 验收：新增 parse/typecheck fixtures：多个 `catch` 可解析且按书写顺序匹配；被前面更宽类型吞掉的 `catch` 会报不可达。
- 依赖：T0607、T0606、T0419
 - 完成：
   - parser：`try/catch/finally` 解析支持多个 `catch`，并 lowering 为单个 `ExprKind::Handle { arms: ... }`（顺序与源码一致）。
   - sysroot：`Raise<in E>`（声明处变型），为“更宽 catch 覆盖更窄 catch”的子类型关系提供基础。
   - typecheck：`handle` 捕获 performed effects 时按“可赋值/子类型”匹配（handled <: performed），并新增不可达 arm 诊断 `scoop::typecheck::handle_arm_unreachable`。
   - fixtures：
     - parse：新增 `tests/fixtures/parse/try_catch_multi_catch_lowering.scoop` + AST snapshot；
     - typecheck：新增 `try_catch_multi_catch_narrow_first_ok` 与 `try_catch_multi_catch_wide_first_unreachable_is_error`。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

### T0632 [DONE] 函数值擦除到 `Any`：仅允许 `Pure!`（effects 不可运行时保真）
- 描述：按 `SCOOP_FULL_SPEC.md` §7.5 “Effect erasure and casts to `Any`” 的规则，禁止把带 effects 的函数值（含 open `/ Pure`）赋给/转换为 `Any`，因为 effects 是纯编译期信息，运行时不携带也无法验证。
- 规则（静态）：
  - 只有函数类型 `(...)->R / Pure!` 允许：
    - 隐式上转：`val x: Any = f`
    - 显式 cast：`f as Any`
  - 其它任何函数类型（包括 `(...)->R / Pure` 与 `(...)->R / Raise<...>`）都必须在 typecheck 阶段报错。
- 验收：
  - 新增 typecheck fixtures：
    - pass：`val f: () -> Int / Pure! = { 1 }; val a: Any = f`
    - fail：`val f: () -> Unit / Raise<RuntimeError> = { perform Raise.raise(...) }; val a: Any = f`
    - fail：`val f: () -> Int / Pure = { 1 }; val a: Any = f`（open `/ Pure` 不允许）
  - 稳定诊断码：例如 `scoop::typecheck::fn_value_to_any_requires_closed_pure`（名字可不同，但必须稳定、可 fixtures 断言）。
  - `cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 依赖：T0627、T0608、T0451（Any/ref assignable 基础）
 - 完成：
   - ty：`FunctionType` 增加 `effects_closed`，并在 `TypeDisplay` 中输出 `... / Row!` 以区分 `/ Pure` 与 `/ Pure!`。
   - typecheck/lower：函数类型 lowering 时保留 row 的 `closed` 标记（缺省为 open）。
   - typecheck/expr：在 initializer / call args / return / cast 等“写入 Any”的位置加入门禁：
     - 仅允许 `(...)->R / Pure!`；
     - 其余函数类型返回稳定错误码 `scoop::typecheck::fn_value_to_any_requires_closed_pure`。
   - fixtures：新增 typecheck fixtures：
     - `tests/fixtures/typecheck/fn_value_to_any_closed_pure_ok.scoop`
     - `tests/fixtures/typecheck/fn_value_to_any_open_pure_is_error.scoop`
     - `tests/fixtures/typecheck/fn_value_to_any_effectful_is_error.scoop`
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop_tools -- spec-fixtures check` 通过。

---

## T07：IR 与单态化（阶段 6：为 LLVM 做准备）

### T0701 [DONE] HIR：定义已解析/已类型化的中间表示骨架
- 描述：新增 `scoopc::hir` 模块：表达式/语句/声明节点携带类型与解析后的 symbol 引用。
- 目标：先覆盖：fun、val、block、call、literal；其余节点可用 `Todo`/`Unimplemented` 占位。
- 验收：新增 `scoop dump-hir <file>`（可先打印 Debug）；对一个最小文件能输出 HIR。
- 依赖：T0404、T0305、T0207、T0002
 - 完成：
   - scoopc：新增 `hir` 模块（File/Item/Fun/Val/Block/Stmt/Expr）与最小 AST→HIR lowering（未覆盖节点用 `Todo`/`Any` 占位，避免 panic）。
   - scoop：新增 `dump-hir` 子命令，调用 `scoopc::hir::lower_for_dump` 并打印 HIR Debug。
   - tests：新增 `hir::lower::tests::lower_minimal_file_smoke` 覆盖最小程序 lowering。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-hir tests/fixtures/parse/hello.scoop` 通过。

### T0702 [DONE] AST → HIR lowering（声明头 + 简单函数体）
- 描述：实现从 AST 构造 HIR：把 `TypeRef` lower 为 `TypeId`，把 ident 绑定为 `SymbolId`。
- 目标：先只支持无控制流的函数体；不支持闭包捕获。
- 验收：新增 fixtures/hir 目录（或用 dump-hir 命令行 golden）；最小程序 lowering 不报错。
- 依赖：T0701
 - 完成：
   - hir：引入 `SymbolId`，并让 `Param`/`ValDecl`/`ValueRef` 携带稳定的符号标识（local decl span / top-level FQN）。
   - hir lowering：在 `Index` 语境下把 `TypeRef` lower 为 `TypeId`（含 builtin、tuple、nullable、function type、nominal 与 type params）。
   - fixtures：新增 `tests/fixtures/hir/minimal.scoop` + `tests/fixtures/hir/minimal.hir` golden，并在 `scoopc` 单测中做回归比对。
   - 验收：`cargo test --all`、`cargo run -p scoop -- dump-hir tests/fixtures/hir/minimal.scoop` 通过。

### T0703 [DONE] MIR：基本块 + 显式控制流骨架（为后续 finally/effect 做准备）
- 描述：定义 MIR 的 BB/terminator/locals；支持顺序执行与 return。
- 目标：暂不实现 if/when lowering；先把数据结构立起来。
- 验收：新增单测：手工构造 MIR 并验证 CFG 连通；或对最小 HIR lowering 生成 1 个 BB + return。
- 依赖：T0702
 - 完成：
   - scoopc：新增 `mir` 模块（Body/BasicBlock/Terminator/locals）与 CFG 校验辅助函数（reachable/unreachable/validate）。
   - scoop：`scoop test` 支持 `tests/fixtures/hir/**`（HIR lowering + `.hir` golden），避免该目录被当作“未实现 phase”而导致回归失败。
   - tests：新增单测覆盖 CFG 连通与非法 target 的稳定报错。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0612 [DONE] HIR/MIR：在 IR 中表达 `perform` 与 `handle`（不做 lowering）
- 描述：为 effect 调用与 handle 表达式添加 IR 节点，确保能从 AST lowering 到 HIR/MIR 并 dump。
- 目标：先只覆盖 non-resuming arm；不实现 unwinding/state machine。
- 验收：`scoop dump-hir`/`dump-ir` 能输出含 perform/handle 的 IR；新增 fixtures/hir 或 snapshot golden 覆盖。
- 依赖：T0605、T0702、T0703
 - 完成：
   - scoopc/hir：新增 `ExprKind::Perform`/`ExprKind::Handle`（含 `HandleExpr/HandleArm/HandleOp/HandleBinder` 与 `EffectOpRef`）。
   - scoopc/hir lowering：识别 effect op call（如 `Raise.raise(1)`）并 lower 为 `Perform`；`handle { ... } with { ... }` lower 为 `Handle`。
   - scoopc/mir：为后续 effect lowering 预留 `TerminatorKind::Perform`/`Handle` 与 `HandlerArm` 结构占位，并更新 CFG 校验/后继遍历逻辑。
   - fixtures：新增 `tests/fixtures/hir/handle_perform.scoop` + `.hir` golden；并在 `scoopc` 单测中加入 golden 回归。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-hir tests/fixtures/hir/handle_perform.scoop` 通过。

### T0704 [DONE] 单态化缓存键：`Symbol + type args + effect row args`（spec §3.1、PLAN §7.2）
- 描述：定义 MonomorphKey，并实现 Hash/Eq 与 debug 输出。
- 目标：先只对函数生效；不实现真实复制生成。
- 验收：新增单测：同一 key 去重；不同 type args key 不同。
- 依赖：T0701、T0401
 - 完成：
   - scoopc：新增 `monomorph` 模块，定义 `MonomorphSymbol/MonomorphKey`，实现 Hash/Eq/Debug。
   - tests：新增单测覆盖 key 去重与 type args 区分。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0705 [DONE] HIR：补齐控制流与语句节点（if/when/while/assign/return）
- 描述：把 parser/typecheck 已支持的控制流语法在 HIR 中建模出来（含 type 与 span）。
- 目标：先只覆盖：if/when/while/return/assign；for 后续。
- 验收：`scoop dump-hir` 对包含上述语法的文件能输出；无 `todo!()`/panic。
- 依赖：T0701、T0214、T0215、T0228、T0227、T0226
 - 完成：
   - scoopc/hir：新增 `ExprKind::If`/`ExprKind::When`、`StmtKind::While`/`StmtKind::Assign`，并引入 `WhenArm`/`WhenPat`。
   - hir lowering：`dump-hir` 的最小 lowering 现可生成上述节点（type 使用内建类型占位），并把 AST 的赋值“表达式语句”映射为 HIR `StmtKind::Assign`。
   - fixtures：新增 `tests/fixtures/hir/control_flow.scoop` + `.hir` golden，并在 `hir::lower` 单测中回归。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-hir tests/fixtures/hir/control_flow.scoop` 通过。

### T0706 [DONE] AST → HIR lowering：把 Stmt/Expr 降到 HIR（含符号绑定结果）
- 描述：实现 block 内语句/表达式 lowering（含局部绑定、赋值、return、调用、成员访问）。
- 目标：先不做 pattern lowering（when 先用简化分支表达）；pattern 后续任务补。
- 验收：新增 HIR snapshot fixtures：至少 1 个包含局部变量与调用；输出稳定。
- 依赖：T0705、T0305、T0443
 - 完成：
   - scoopc/hir：新增 `ExprKind::MemberAccess` + `MemberAccess/MemberRef`，用于承载 resolver 写回的成员绑定结果（字段/方法/扩展成员 FQN）。
   - scoopc/hir lowering：实现 `ast::ExprKind::MemberAccess` → HIR lowering（保留 member `span/name`，并将 `ResolvedMemberRef` 映射为带 `SymbolId` 的 `MemberRef`）。
   - fixtures：新增 `tests/fixtures/hir/member_access.scoop` + `.hir` golden 覆盖成员访问、成员调用与成员赋值；并加入 `hir_fixture_member_access_golden` 单测回归。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-hir tests/fixtures/hir/member_access.scoop` 通过。

### T0707 [DONE] MIR：cleanup/finally 的基本模型（为 try/finally 与 effect unwinding）
- 描述：在 MIR 中引入 cleanup block 或显式 drop/cleanup 机制，让 lowering 可以表达“无论如何都执行”的语义。
- 目标：先只支持 `try/finally`；不实现析构（语言尚无 drop）。
- 验收：新增单测：构造一个带 cleanup 的 MIR 并验证 CFG；后续可被 codegen 使用。
- 依赖：T0703
 - 完成：
   - scoopc/mir：新增 `UnwindAction` 与 `Terminator.unwind`；新增 `TerminatorKind::ResumeUnwind`；`BasicBlock` 增加 `is_cleanup` 标记；CFG 校验与可达性分析纳入 cleanup/unwind 边。
   - tests：新增 MIR 单测覆盖 cleanup edge 可达性与 CFG 校验。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0708 [DONE] MIR lowering：if/when → 基本块 + terminator
- 描述：把条件分支 lowering 为 CFG（br/switch），并在 merge 点管理临时变量。
- 目标：先只支持 expression-when（每分支一个表达式）；带 guard 的 pattern 后置。
- 验收：`--dump-ir`（或 dump-mir）对 if/when 示例输出多个 BB；并能被 codegen 接受。
- 依赖：T0706
 - 完成：
   - scoopc/mir：新增最小 MIR lowering（`crates/scoopc/src/mir/lower.rs`），支持 `if/when` 生成显式 CFG（`CondBr/Goto`）并在 merge 点写回临时 local。
   - scoopc/mir：扩展 MIR 数据结构：`TerminatorKind::CondBr`、`StatementKind::Assign`、最小 `Operand/Rvalue`，以及用于 dump/fixtures 的 `mir::File/FunDecl`。
   - scoop：新增 `scoop dump-mir <file>`；fixtures runner 支持 `tests/fixtures/mir/**` 并对 `.mir` golden 做回归比对。
   - fixtures：新增 `tests/fixtures/mir/if_when.scoop` + `if_when.mir`（包含 if/when 多 BB 的快照）。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过；`cargo run -p scoop -- dump-mir tests/fixtures/mir/if_when.scoop` 输出多基本块。

### T0709 [DONE] MIR lowering：while/break/continue
- 描述：把 while lowering 为 loop CFG，并正确处理 break/continue 跳转目标。
- 目标：先不支持 label；后续再补。
- 验收：新增 IR snapshot fixture：while 内 break/continue 的 CFG 正确（可用文本快照验证）。
- 依赖：T0706
 - 完成：
   - scoopc/hir：补齐 `StmtKind::Break/Continue`，并在 AST→HIR lowering 中生成对应节点（不再用 `Todo("break")`/`Todo("continue")` 占位）。
   - scoopc/mir lowering：实现 `while` 的 loop CFG 生成，并引入 loop 栈以支持 `break/continue` 正确跳转到 exit/cond block。
   - fixtures：新增 `tests/fixtures/mir/while_break_continue.scoop` + `.mir` golden 回归 CFG 形态。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-mir tests/fixtures/mir/while_break_continue.scoop` 通过。

### T0710 [DONE] 闭包与函数值：lambda → `{ env_struct, fn_ptr }`（PLAN §7.3）
- 描述：在 HIR/MIR 中引入 closure 表示，支持捕获变量布局与调用约定。
- 目标：先只支持不捕获 lambda（env 为空）；捕获后续子任务。
- 验收：新增 typecheck + IR fixture：把 lambda 赋给函数类型并调用；编译通过并能 codegen（后续与 T0810 联动）。
- 依赖：T0222、T0435、T0706
 - 完成：
   - scoopc/typecheck：支持调用局部函数值（function type）：`f(args...)`。
   - scoopc/hir：新增 `ExprKind::Closure`/`ClosureExpr`/`ClosureId`，并在 lowering 中把 AST lambda 降到 HIR。
   - scoopc/mir：新增 `Rvalue::MakeClosure`；MIR lowering 遇到 closure 时生成 `{ env=Unit, fn_ptr }`，并追加生成的 `$lambdaN` 函数。
   - fixtures：新增 `tests/fixtures/infer/function_value_call_ok.scoop`、`tests/fixtures/hir/closure_non_capture.*`、`tests/fixtures/mir/closure_non_capture.*` 回归用例。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test` 通过。

### T0711 [DONE] 捕获闭包：计算 capture set 并生成 env struct（PLAN §7.3）
- 描述：分析 lambda 体对外部局部变量的引用，生成 env struct，并在调用点传递 env 指针。
- 目标：先只支持捕获 `val`；捕获 `var`（可变捕获）后置或以 box 处理。
- 验收：新增 IR fixture：lambda 捕获外部 val 并使用；codegen/run-pass（后续）输出正确。
- 依赖：T0710、T0304
 - 完成：
   - scoopc/hir：`ClosureExpr` 增加 `captures: Vec<Capture>`，lowering 会计算 lambda 的 capture set（跳过嵌套 closure），并按声明位置稳定排序。
   - scoopc/mir：引入最小 env tuple 表示：
     - 新增 `Rvalue::MakeTuple`/`Rvalue::TupleGet`；
     - closure 创建点：根据 capture set 构造 env tuple，并写入 `MakeClosure.env`；
     - closure 函数体：`$env` 参数类型改为对应的 tuple type，并在入口块把捕获字段解包到局部 local（写入 `SymbolId → LocalId`），使 body 内对捕获变量的 `VarRef` 可正常 lowering。
   - fixtures：新增 `tests/fixtures/hir/closure_capture_val.*` 与 `tests/fixtures/mir/closure_capture_val.*`；并更新 `tests/fixtures/hir/closure_non_capture.hir` 以适配新增字段。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-hir tests/fixtures/hir/closure_capture_val.scoop`、`cargo run -p scoop -- dump-mir tests/fixtures/mir/closure_capture_val.scoop` 通过。

### T0712 [DONE] 单态化（monomorphization）：生成具体实例 MIR（PLAN §7.2）
- 描述：对每个 `MonomorphKey` 生成专用实例（函数/类型），并缓存避免重复。
- 目标：先只对函数泛型实例化；类型泛型与 effect row 参数后置。
- 验收：新增 `tests/fixtures/codegen/monomorph_id_int.scoop`：`id(1)` 与 `id("s")` 生成两个实例（可用 dump-ir 验证）。
- 依赖：T0704、T0505、T0703
 - 完成：
   - typecheck：新增 `check_file_exprs_with_monomorph_keys` 入口，并在“泛型顶层函数调用”通过后记录 `MonomorphKey`（去重）。
   - scoopc/hir：新增 `lower_fun_with_type_bindings`，支持在已绑定 type params 的语境下降低单个函数（供 monomorph 生成实例复用）。
   - scoopc/mir：新增 `lower_hir_file_for_dump`，允许从既有 HIR 直接降低到 MIR（避免重复 parse/resolve）。
   - scoopc/monomorph：新增 `lower_for_dump`，执行 parse/resolve/typecheck 收集 keys，并生成实例 HIR→MIR（实例名：`fqn::<TypeArgs...>`）。
   - scoop：新增 `scoop dump-ir` 子命令，输出单态化实例 MIR 的 Debug 视图。
   - tests：新增 `monomorph_collects_two_instances_for_id` 单测；新增 `tests/fixtures/codegen/monomorph_id_int.scoop`（run-pass 尚未启用，EXPECT: fail，但可用于 `scoop dump-ir` 手动验证）。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-ir tests/fixtures/codegen/monomorph_id_int.scoop` 通过/可见 `id::<Int>` 与 `id::<String>` 两个实例。

### T0713 [DONE] effect lowering：把 perform/handle 降到 MIR（non-resuming 先占位）
- 描述：在 MIR 中表达 perform 与 handler boundary（先用占位 terminator），为 T0614 的 codegen 做准备。
- 目标：先只覆盖 Raise 与 try/catch；resume/continuation 后置。
- 验收：dump-mir 能看到 perform/handler 相关 terminator；无 panic。
- 依赖：T0612、T0703
 - 完成：
   - scoopc/mir lowering：HIR `ExprKind::Perform/Handle` 现在会生成 `TerminatorKind::Perform/Handle`；并为 `Perform` 标记 `unwind: Todo(...)`。
   - scoopc/mir lowering：`handle` 的 body/arms/finally 会被 lowering 到独立 basic blocks（当前不连接到主 CFG，仅用于 dump/fixtures 观察内部 `perform`）。
   - fixtures：新增 `tests/fixtures/mir/handle_perform.scoop` + `.mir` golden 回归用例。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop` 通过。

### T0714 [DONE] 捕获闭包：`var` 可变捕获的 boxing / aliasing / 写回
- 描述：在已有 capture set / env struct 基础上，支持捕获可变局部：为 `var` 引入 box 或等价可变单元，并固定读写别名与生命周期语义。
- 目标：先保证语义正确与 GC 可追踪；不急于优化为栈提升或逃逸分析。
- 验收：新增 IR/run-pass fixtures：lambda 内外都修改同一个 `var` 时结果一致；多个闭包共享同一捕获盒时行为稳定。
- 依赖：T0711、T0443
 - 完成：
   - scoopc/hir：`Capture` 增加 `mutable: bool`；HIR lowering 记录局部 `val/var` 的 mutability，并在 closure capture set 中把被捕获的 `var` 标记为 `mutable: true`。
   - scoopc/mir：
     - 引入内部 box 类型 `scoop.__CaptureBox<T>`；
     - Rvalue 新增 `CaptureBoxNew/Get/Set`；
     - 函数 lowering 预扫描任意深度的嵌套 closure captures：若某个 `var` 被捕获，则其存储方式升级为 box（声明处 `CaptureBoxNew`），并确保读写统一经由 `CaptureBoxGet/Set`；
     - closure env 捕获 box（而非值拷贝），从而多个 closure 共享同一捕获盒。
   - fixtures：新增 `tests/fixtures/{hir,mir}/closure_capture_var.*`；更新 `tests/fixtures/hir/closure_capture_val.hir` 以包含 `mutable` 字段。
   - 验收：`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop -- dump-mir tests/fixtures/mir/closure_capture_var.scoop` 通过。

---

## T08：LLVM 后端与链接（阶段 7：inkwell codegen）

### T0801 [DONE] 引入 inkwell（或等价 LLVM 绑定）并完成最小编译
- 描述：在 `scoopc` 新增 feature-gated `inkwell` 依赖，确保 workspace 在 CI 环境可构建。
- 目标：先只做到 `cargo build`；不生成任何 IR。
- 验收：`cargo build --all` 通过；若 CI 缺 LLVM，则需要在文档/CI 明确安装步骤或先使用 feature gate 默认关闭。
- 依赖：T0001
 - 完成：
   - `crates/scoopc/Cargo.toml`：新增 `llvm` feature（默认关闭），并以 optional 依赖引入 `inkwell`。
   - `README.md`：补充 LLVM 后端 feature 的开启方式与 `llvm-config` 依赖说明。
   - 验收：`cargo build --all`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check`、`cargo run -p scoop -- test` 通过。

### T0802 [DONE] 代码生成 v0：生成空 `main` LLVM module（可打印 IR）
- 描述：为一个最小 Scoop 程序生成 LLVM IR（哪怕只返回 0）。
- 目标：先不处理用户函数；先把 pipeline 与 target triple 跑通。
- 验收：新增 `scoopc` API 或 CLI `--emit-llvm` 能输出 `.ll`；对最小 fixture 可生成文件。
- 依赖：T0801、T0703
 - 完成：
   - `crates/scoopc/src/llvm/mod.rs`：新增最小 LLVM codegen：生成 module + `i32 @main()`（返回 0），并打印/写出 `.ll`。
   - `crates/scoopc/src/bin/scoopc.rs`：新增 `scoopc --emit-llvm <input.scoop> [-o <out.ll>]` 命令行，用于写出 LLVM IR 文件。
   - `crates/scoopc/Cargo.toml`：为 `scoopc` 二进制设置 `required-features = ["llvm"]`，避免无 LLVM 环境下默认构建失败。
   - 验收：
     - `cargo test --all`、`cargo run -p scoop -- test` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoopc --features llvm --bin scoopc -- --emit-llvm tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/overview_minimal_main.ll` 可生成 `.ll` 文件。

### T0803 [DONE] 目标机器与数据布局（target machine）初始化（PLAN §8.1）
- 描述：在 codegen 中按宿主平台初始化 target，设置 module data layout，并把 pointer size 等 target 信息暴露给后续类型映射（例如 `Int/UInt/UIntPtr` 的 word size）。
- 目标：先只支持 host；交叉编译后续。
- 验收：生成的 LLVM module 带 data layout；并可用 `llvm-as`（若有）验证（可选）。
- 依赖：T0802
 - 完成：
   - `crates/scoopc/src/llvm/target.rs`：新增 host target machine 初始化与 module（triple + data layout）配置；暴露 `HostTargetInfo`（含 pointer size/byte order）。
   - `crates/scoopc/src/llvm/mod.rs`：codegen 时调用 `configure_module_for_host`，并在单测中断言 `target datalayout =` 行存在。
   - 验收：
     - `cargo test --all` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoopc --features llvm --bin scoopc -- --emit-llvm tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/overview_minimal_main.ll` 生成的 `.ll` 含 `target datalayout =`；
     - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" llvm-as /tmp/overview_minimal_main.ll -o /tmp/overview_minimal_main.bc` 通过。

### T0804 [DONE] 生成 object 文件（`.o`）并落盘
- 描述：从 LLVM module 生成目标文件（object），为后续链接做准备。
- 目标：先只生成单个 `.o`；不做 LTO。
- 验收：新增 `scoop build --emit-obj`（或 `scoopc` 命令）；产出 `.o` 文件存在且非空。
- 依赖：T0803
 - 完成：
   - `crates/scoopc/src/llvm/target.rs`：新增 `host_target_machine()`，用于 object emission 获取 host target machine。
   - `crates/scoopc/src/llvm/mod.rs`：新增 `emit_minimal_main_obj_to_file()`（`.o` 落盘），并补齐单测断言产物非空。
   - `crates/scoopc/src/bin/scoopc.rs`：新增 `--emit-obj` 参数与默认输出扩展名 `.o`。
   - 验收：
     - `cargo test --all` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
     - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoopc --features llvm --bin scoopc -- --emit-obj tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/overview_minimal_main.o` 可生成 `.o` 且非空。

### T0805 [DONE] driver：实现 `scoop build <main.scoop> -o <bin>` 的“前端 + 产物路径”流程
- 描述：让 `scoop build` 至少能：读取文件 → parse/resolve/typecheck →（暂时）不 codegen 也能成功退出并准备输出路径。
- 目标：先把 CLI 与诊断体验打磨出来；codegen 后续任务接入。
- 验收：新增 fixtures（或集成测试）：`scoop build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/a` 返回 0。
- 依赖：T0404、T0002
 - 完成：
   - `crates/scoop/src/commands/build.rs`：新增 `scoop build`（前端 parse/resolve/typecheck）与输出路径父目录准备（仅创建目录，不写二进制）。
   - `crates/scoop/src/commands/mod.rs`：接入 `Command::Build` 分发。
   - `crates/scoop/src/cli.rs`：更新 `build` 命令帮助文本（明确当前阶段只做前端检查）。
   - `crates/scoop/Cargo.toml`：增加 dev-dependency `tempfile`，用于 build 子命令单测。
   - 验收：
     - `cargo test -p scoop` 通过（含 `commands::build` smoke test）；
     - `cargo run -p scoop -- build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/a` 返回 0。

### T0806 [DONE] 链接：把 `.o` 与 `scoop_runtime` 静态库链接为可执行文件
- 描述：实现最小链接器调用（可用 clang 或 `cc` crate）把 runtime 拉进来。
- 目标：先只支持 host 平台；暂不处理多文件/包。
- 验收：`scoop build ...` 产出可执行文件；运行后退出码正确（哪怕 main 空）。
- 依赖：T0804、T0014、T0805
 - 完成：
   - `crates/scoop/src/toolchain.rs`：新增最小 clang 链接封装 `link_obj_with_runtime()`，并提供单测（不依赖 LLVM）。
   - `crates/scoop/src/commands/build.rs`：在启用 `scoop` 的 `llvm` feature 时，调用 `scoopc::llvm::emit_minimal_main_obj_to_file()` 生成 `.o`，再通过 clang 链接为可执行文件。
   - `crates/scoop/Cargo.toml`：新增 `llvm` feature（转发到 `scoopc/llvm`），默认保持关闭以兼容未安装 LLVM 的环境。
   - 文档/CLI：更新 `README.md` 与 `crates/scoop/src/cli.rs`，说明 `scoop build` 需要启用 `--features llvm` 才会真正产出二进制。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoop --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/scoop_overview_minimal_main` 生成可执行文件并运行返回 0。

### T0807 [DONE] driver：实现 `scoop run <main.scoop>`（build + exec）
- 描述：在 build 成功后执行产物，并把 stdout/stderr 透传。
- 目标：先不做 sandbox；超时/退出码断言留给 fixtures。
- 验收：`scoop run tests/fixtures/spec_doctest/overview_minimal_main.scoop` 返回 0。
- 依赖：T0806
 - 完成：
   - `crates/scoop/src/commands/run.rs`：新增 `scoop run`（临时目录构建并执行，stdout/stderr 透传，退出码透传）。
   - `crates/scoop/src/commands/temp.rs`：抽取临时目录创建工具，供 `build/run` 复用。
   - `crates/scoop/src/commands/build.rs`：复用共享的临时目录工具（去重）。
   - `crates/scoop/src/commands/mod.rs`：接入 `Command::Run` 分发。
   - `crates/scoop/src/cli.rs`：更新 `run` 子命令帮助文本（提示需要 `--features llvm`）。
   - `README.md`：补齐 `scoop run` 使用示例。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `cargo run -p scoop -- run tests/fixtures/spec_doctest/overview_minimal_main.scoop` 给出“需要启用 LLVM”提示；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- run tests/fixtures/spec_doctest/overview_minimal_main.scoop` 返回 0。

### T0106b2 [DONE] run-pass fixtures：默认使用 `scoop run` 执行 + 增加 1 个可执行 fixture
- 描述：当 `scoop run`（T0807）可用后，fixtures runner 通过 `scoop run <fixture>` 真正执行 fixture，并断言 stdout。
- 目标：先只做 stdout；stderr/超时/退出码仍留给后续任务。
- 验收：新增 1 个 run-pass fixture（例如打印固定字符串）；`cargo run -p scoop -- test` 能编译并运行且通过。
- 备注：该任务本身属于测试体系，但其验收依赖完整执行链路（`scoop run` + link/runtime/codegen），因此放在这里以便按依赖顺序推进。
- 依赖：T0106b1、T0807
 - 完成：
   - `crates/scoop/src/fixtures/run_pass.rs`：run-pass phase 默认通过 `scoop run <fixture>` 子进程执行并捕获 stdout/stderr，与 golden 比对；未启用 `llvm` feature 时仅校验 golden 可读并跳过执行。
   - `crates/scoop/src/fixtures/mod.rs`：run-pass phase 接入 `run_pass::run_fixture()`。
   - `tests/fixtures/run-pass/minimal_main.scoop`：新增最小可执行 run-pass fixture（断言 stdout）。
   - `tests/fixtures/codegen/monomorph_id_int.scoop`：run-pass phase 已启用，更新为 `EXPECT: pass`（作为 smoke）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `cargo run -p scoop --features llvm -- test` 可真正执行 run-pass fixtures（需要 LLVM/llvm-config + clang）。

### T0111b [DONE] run-pass fixtures：`RUN-STDERR` golden（真实 fixtures 覆盖）
- 描述：在 `tests/fixtures/run-pass/**` 增加 fixtures 覆盖 `RUN-STDERR`：一个断言 stderr 输出正确，一个断言 stdout/stderr 同时输出且 stderr mismatch 时 runner 报错码稳定。
- 目标：保证 run-pass 的 stdout/stderr mismatch 诊断可稳定区分（便于长期回归）。
- 验收：新增 2 个 run-pass fixtures（1 pass + 1 fail）；`cargo run -p scoop -- test` 能稳定通过且在 fail case 下给出 `scoop::fixtures::run_stderr_mismatch`（或同等稳定错误码）。
- 依赖：T0106b2、T0111a、T0107
 - 完成：
   - `crates/scoop/src/fixtures/run_pass.rs`：未启用 `llvm` feature 时，对 `EXPECT: fail` 的 run-pass fixture 做“空输出模拟”，确保在 CI 下也能回归 stderr mismatch 的稳定错误码。
   - `tests/fixtures/run-pass/stderr_empty_ok.scoop` + `tests/fixtures/run-pass/stderr_empty_ok.stderr`：断言 stderr（空输出）golden 一致。
   - `tests/fixtures/run-pass/stderr_mismatch_distinguishable.scoop` + golden：同时断言 stdout/stderr，且 stderr mismatch 的错误码稳定为 `scoop::fixtures::run_stderr_mismatch`。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0112 [DONE] run-pass fixtures：`EXPECT-EXIT` / `TIMEOUT` 真正生效
- 描述：让 fixtures runner 对运行进程执行退出码断言与超时控制，并在超时、信号终止、非预期退出码时生成稳定诊断。
- 目标：先只覆盖单进程执行模型；不做沙箱与资源限额。
- 验收：新增 run-pass fixtures：一个断言非零退出码，一个断言超时失败；`scoop test` 能稳定区分“程序失败”和“fixture 断言失败”。
- 依赖：T0106b2、T0107
 - 完成：
   - `crates/scoop/src/fixtures/run_pass.rs`：run-pass 执行器支持 `EXPECT-EXIT` 与 `TIMEOUT`；新增超时/信号终止/退出码不匹配的稳定诊断，并保持“未启用 llvm 时可回归”的 fail 用例模拟逻辑。
   - `tests/fixtures/run-pass/exit_code_mismatch.scoop`：新增退出码 mismatch 的 fixtures 覆盖（稳定错误码 `scoop::fixtures::run_exit_code_mismatch`）。
   - `tests/fixtures/run-pass/timeout_should_fail.scoop`：新增超时 fixtures 覆盖（稳定错误码 `scoop::fixtures::run_exec_timeout`）。
   - `crates/scoop/src/fixtures/run_pass.rs`：新增单测覆盖 `EXPECT-EXIT` 通过/不匹配、超时、信号终止。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0108 [DONE] fixtures：支持环境变量开关（如 `SCOOP_GC_STRESS=1`）（PLAN §10.4）
- 描述：允许 fixture 通过 `// ENV: KEY=VALUE`（或统一用 `ARGS`）配置测试运行环境。
- 目标：先只支持设置环境变量；不做进程级 sandbox。
- 验收：新增 1 个 run-pass 单测：执行外部命令读取 env 并通过 stdout golden 比对；runner 能正确设置 env。
- 备注：当前用 run-pass 单测验证 env 注入；若未来要新增“`.scoop` 程序读取 env 并断言输出”的真实 run-pass fixture，需要后续 sysroot/env API（不在本任务范围）。
- 依赖：T0106b2、T0102
 - 完成：
   - `crates/scoop/src/fixtures/expectations.rs`：新增 `// ENV: KEY=VALUE` 指令解析并写入 `FixtureExpectation::env`（支持一行多个 `KEY=VALUE`，也支持多行重复声明）。
   - `crates/scoop/src/fixtures/run_pass.rs`：run-pass 执行前为子进程注入 env（`Command::env`）。
   - `crates/scoop/src/fixtures/run_pass.rs`：新增单测，通过执行 `sh` 读取 `$FOO` 来验证 env 注入生效（当前阶段 Scoop 程序侧尚无 env 读取/输出 API）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0808 [DONE] codegen v1：整数/布尔字面量 + 运算（含位运算/移位）+ return（spec §2.3.4）
- 描述：为最小表达式子集生成 LLVM IR：Int/Bool 字面量、算术/比较、位运算 `& | ^ ~`、移位 `<< >>`、return。
- 目标：按 spec 固定整数语义：
  - wrap-around（LLVM 默认不加 `nsw/nuw` 即可）
  - signed `>>` 用算术右移，unsigned `>>` 用逻辑右移
  - shift count 必须 mask（例如 `shift % bitWidth`），避免 LLVM 对超范围 shift 的 UB
- 验收：新增 run-pass fixture：位运算与移位得到稳定结果（含 `UInt8` 的 `>>`）；输出正确。
- 依赖：T0802、T0708、T0106b2
 - 完成：
   - `crates/scoopc/src/hir/mod.rs`：HIR 表达式增加 `Unary/Binary` 节点，承载 `!/-/~` 与常见二元运算。
   - `crates/scoopc/src/hir/lower.rs`：AST→HIR lowering 补齐一元/二元运算节点，并在 closure captures 的 declared/used 收集里递归处理新节点。
   - `crates/scoopc/src/llvm/codegen.rs`：实现 main v1 的 LLVM codegen（整数/布尔字面量、算术/比较/位运算/移位、shift mask、`val` 局部绑定、`return`/隐式返回）。
   - `crates/scoopc/src/llvm/mod.rs`：最小 module 仍生成 `i32 @main()`，但 body 改为调用 v1 codegen 并返回计算结果。
   - `tests/fixtures/run-pass/int_bitops_shift.scoop`：新增 run-pass fixture：覆盖 `& | ^ ~`、`<< >>`、shift count mask、`UInt8` 逻辑右移，并用 `EXPECT-EXIT` 断言结果稳定。
   - `tests/fixtures/codegen/monomorph_id_int.scoop`：补齐 `main`（返回 0），使其在启用 `llvm` 时可被 run-pass phase 真正执行（继续作为 smoke）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/llvm-config + clang）。

### T0809 [DONE] codegen v2：局部变量（alloca）与赋值
- 描述：把 HIR/MIR locals 映射到 LLVM alloca/load/store，支持 `var` 赋值更新。
- 目标：先只支持函数内局部；不实现逃逸分析。
- 验收：新增 run-pass fixture：`var x = 1; x = x + 1; return x` 退出码为 2（当前阶段用 exit code 断言）。
- 依赖：T0808、T0443
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：locals 统一降为 `alloca` + `load/store`；支持 `var` 声明与 `x = expr` 赋值语句（仅 local `var`）。
   - `tests/fixtures/run-pass/var_assign_basic.scoop`：新增 run-pass fixture，覆盖 `var` 赋值更新与读写回归。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/llvm-config + clang）。

### T0810 [DONE] codegen v3：函数调用 ABI（参数传递/返回值）
- 描述：支持调用用户定义函数与 sysroot/extern 函数（先按简单 C ABI）。
- 目标：先不支持可变参数/泛型实例化跨模块；只编译单模块。
- 验收：新增 run-pass fixture：定义 `add` 并调用；输出正确。
- 依赖：T0809、T0311
 - 完成：
   - `crates/scoopc/src/llvm/mod.rs`：在生成 LLVM module 时，收集 `main` 可达的顶层函数（基于 HIR `Call(TopLevel)` 扫描）并仅为这些函数声明/生成 LLVM function，避免未使用的泛型/占位签名影响 codegen。
   - `crates/scoopc/src/llvm/codegen.rs`：支持 `ExprKind::Call` 的最小 lowering（callee 必须是 `TopLevel` fun），按签名做参数 coercion，并生成 `call` 指令；同时支持为可达顶层函数生成 entry/params alloca 与返回值。
   - `tests/fixtures/run-pass/fun_call_add_basic.scoop`：新增 run-pass fixture：`add(1, 2)` 返回 3，使用 `EXPECT-EXIT` 断言结果。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoop --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/llvm-config + clang）。

### T0811 [DONE] codegen：值类型布局（struct）与字段访问（PLAN §8.2）
- 描述：为 struct 生成 LLVM struct type，支持按字段索引 GEP 访问。
- 目标：先只支持无 padding 调整（交给 LLVM layout）；不实现 repr 属性。
- 验收：新增 run-pass fixture：构造 struct、读取字段；输出（exit code）正确。
- 依赖：T0810、T0409
 - 完成：
   - `crates/scoopc/src/hir/{mod.rs,lower.rs}`：HIR 新增 `StructLit` 表达式节点；并在 lowering 阶段收集 `StructLayoutIndex`（struct FQN → 字段顺序/类型 FQN）供后端使用，同时保持 HIR/MIR fixtures 的 `TypeId` 稳定回归。
   - `crates/scoopc/src/llvm/codegen.rs`：支持：
     - 把 struct FQN 映射为 named LLVM struct type（opaque + set_body）；
     - struct literal 构造（`insertvalue` 组装 aggregate）；
     - 字段读取：对 `localStruct.field` 使用 `getelementptr`（struct GEP）+ `load`。
   - `tests/fixtures/run-pass/struct_field_access_basic.scoop`：新增 run-pass fixture，覆盖 struct literal + 字段读取并用 `EXPECT-EXIT` 断言结果（当前无 `print/println`）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/llvm-config + clang）。

### T0812 [DONE] codegen：tuple/Unit 的表示与传递（spec §2.3.3）
- 描述：为 tuple 生成 LLVM struct（或 aggregate），支持构造/解构访问（先用于表达式/赋值）。
- 目标：先只支持定长 tuple；不实现 variadic tuple。
- 验收：新增 run-pass fixture：`val t = (1,2); t._0 + t._1`（按语法）结果为 3（当前用 exit code 断言）。
- 依赖：T0811、T0410
 - 完成：
   - `crates/scoopc/src/hir/mod.rs`：HIR 新增 `ExprKind::TupleLit` 表达 tuple 字面量。
   - `crates/scoopc/src/hir/lower.rs`：实现 tuple literal lowering（`TypeStore::ty_tuple`），并补齐 closure capture 与调用扫描的遍历分支。
   - `crates/scoopc/src/typecheck/expr.rs`：支持 tuple 元素访问 `t._0` / `t._1`（resolver 无法写回成员 FQN 的场景）。
   - `crates/scoopc/src/llvm/codegen.rs`：实现 tuple LLVM struct type、tuple literal 构造（`insertvalue`）与元素访问（`struct_gep`/`extractvalue`）。
   - `crates/scoopc/src/llvm/mod.rs`：可达调用扫描支持遍历 tuple literal。
   - `tests/fixtures/run-pass/tuple_access_basic.scoop`：新增 run-pass fixture，覆盖 tuple literal + 元素访问并用 `EXPECT-EXIT` 断言结果。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/llvm-config + clang）。

### T0813 [DONE] codegen：rich enum（tagged union）最小布局（PLAN §8.2）
- 描述：为 enum 生成 `{tag, payload}` 表示（payload 用最大变体的 LLVM struct/union），支持构造与判别。
- 目标：先不做 niche 优化；先只支持小 payload。
- 验收：新增 run-pass fixture：构造 `Some(1)` 与 `None`，when 分支输出不同结果。
- 依赖：T0812、T0425、T0708
 - 完成：
   - `crates/scoopc/src/hir/{mod.rs,lower.rs}`：新增 `EnumLayoutIndex`（enum FQN → variants/tag/字段类型），并在 HIR lowering 中收集顶层非泛型 enum 的布局；同时用 `ExprKind::UnresolvedIdent` 保留 resolver 未绑定的标识符，便于后端在“期望类型语境”下处理 enum variant ctor/值。
   - `crates/scoopc/src/llvm/codegen.rs`：新增 enum 的最小 LLVM 表示 `{ i32 tag, iN payload }`（payload 为 word-sized int），支持：
     - enum variant ctor：`Some(1)`（需要期望类型为 enum）；
     - 0-参数 variant：`None()`（同样走 ctor 路径以绕开 resolver 对裸 `None` 的限制）；
     - `when`：按 tag 比较分发到各分支，并支持 `Some(v)` binder 解构读取 payload。
   - `tests/fixtures/run-pass/enum_rich_when_basic.scoop`：新增 run-pass fixture，覆盖 `Some(1)`/`None()` 构造 + `when` 分支，并用 `EXPECT-EXIT` 断言结果。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoopc --features llvm` 通过；
   - `cargo test -p scoop --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（含新 run-pass fixture；需要 LLVM/llvm-sys + clang）。

### T0813b [DONE] value-only enum：`enum E: Int { ... }`（C enum 同布局）（spec §2.3.2.1）
- 描述：实现 value-only enum（仅有枚举值、无 associated data）的完整链路：parser/typecheck/lowering/codegen。
- 目标：
  - 语法：`enum E: Int { A = 0, B = 1 }`（底层类型必须为整型标量）；
  - 语义：枚举类型保持名义类型，但 ABI/内存布局与底层整型完全一致；
  - 允许在 `@Extern` 签名与 `@CLayout` struct 中直接使用（不需要额外注解）。
- 验收：
  - 新增 parse fixture：底层类型 + 显式值列表可解析；
  - 新增 typecheck fixture：非整型底层类型报错（稳定错误码）；
  - 新增 run-pass fixture：构造 value-only enum、`when` 匹配分支并断言结果；可选追加：传入 `@Extern`（或 runtime helper）并验证 ABI 不变。
- 依赖：T0236、T1020
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：`EnumVariantDecl` 新增 `discriminant: Option<Expr>`，并保持 Debug 输出对旧 fixtures 兼容（仅在 Some 时打印）。
   - `crates/scoopc/src/parser/decls.rs`：enum variant 支持 `A = 0` 形式（解析判别值表达式）。
   - `crates/scoopc/src/typecheck/lower.rs`：对 value-only enum 做最小门禁：
     - 底层类型必须是整型标量（稳定错误码 `scoop::typecheck::value_only_enum_underlying_not_integral`）；
     - variant 不允许字段；判别值必须存在且为整型常量（IntLit / `-IntLit`）。
   - `crates/scoopc/src/typecheck/interfaces.rs`：对 enum 的 supertypes 做最小消歧，避免把 value-only enum 的底层类型当作 interface 报错。
   - `crates/scoopc/src/typecheck/type_env.rs`：value-only enum 不把底层类型写入 direct supertypes（避免错误子类型关系）。
   - `crates/scoopc/src/hir/{mod.rs,lower.rs}`：HIR side table 记录 enum repr（tagged-union vs value-only）与显式判别值。
   - `crates/scoopc/src/llvm/codegen.rs`：value-only enum codegen：
     - enum 运行期类型直接映射为底层整型；
     - `EnumName.Variant` 常量与 `when` 分派按判别值工作（含非连续判别值）。
   - `tests/fixtures/parse/enum_value_only_basic.scoop`：新增 parse fixture + golden。
   - `tests/fixtures/typecheck/enum_value_only_underlying_not_integral_is_error.scoop`：新增 typecheck fixture（错误码稳定）。
   - `tests/fixtures/run-pass/enum_value_only_when_basic.scoop`：新增 run-pass fixture（exit code 断言）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（需要 LLVM/clang 环境）。

### T0814 [DONE] codegen：`when` lowering（switch + pattern tests）
- 描述：把 `when`（至少 enum/Bool）降到 LLVM switch；tuple/struct pattern 用字段比较实现。
- 目标：先不支持 or-pattern/guard；后续再扩展。
- 验收：新增 run-pass fixture：when on Option/Bool 输出正确；缺 else 的错误在 typecheck 阶段已挡住。
- 依赖：T0813、T0428
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：`codegen_when_expr` 支持：
     - enum `when`：按 enum tag 生成 LLVM `switch`，并保持“按源码顺序”的首个匹配 arm 语义；
     - bool `when`：按 `true/false` 生成 LLVM `switch`；
     - tuple `when`：生成判别链，并对 tuple pattern 做字段相等比较（支持嵌套 tuple）。
   - `crates/scoopc/src/llvm/codegen.rs`：`bind_when_pat` 泛化为“按 subject 类型绑定”，并新增 tuple pattern 的 binder 绑定（`(1, x)` 绑定 `x`）。
   - `tests/fixtures/run-pass/when_switch_basic.scoop`：新增 run-pass fixture，覆盖 enum/bool/tuple 三类 `when`（用 `EXPECT-EXIT` 断言结果）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（含新 run-pass fixture；需要 LLVM/llvm-config + clang）。

### T0815 [DONE] runtime 集成：生成的 `main` 调用 `scoop_runtime_init`
- 描述：在入口函数里调用 runtime init（以及必要的 thread register）。
- 目标：先只在 main 调用一次；多线程后续再处理。
- 验收：链接后的程序运行不崩溃；可通过运行时 debug 输出确认 init 被调用（若启用）。
- 依赖：T0901、T0806
 - 完成：
   - `crates/scoopc/src/llvm/mod.rs`：生成的 `i32 @main()` 在执行 Scoop `fun main` 之前，会先声明并调用 `scoop_runtime_init()`（C ABI）。
   - `crates/scoopc/src/llvm/mod.rs`：更新 LLVM 单测，断言 IR 中包含对 `scoop_runtime_init` 的调用。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- run tests/fixtures/spec_doctest/overview_minimal_main.scoop` 通过（需要 LLVM/llvm-config + clang）。

### T0819 [DONE] driver：`--emit-llvm/--emit-obj/--emit-asm` 选项与 fixtures 支持（PLAN §9.3）
- 描述：在 `scoop build` 增加 emit 选项，并允许 fixtures 通过 `ARGS` 触发生成产物用于排查。
- 目标：先只支持单文件输出；不做多产物目录管理。
- 验收：新增 1 个 fixture：`// ARGS: --emit-llvm` 能生成 `.ll` 文件；`scoop test` 通过。
- 依赖：T0102、T0804
 - 完成：
   - `crates/scoop/src/cli.rs`：为 `scoop build` 增加互斥选项 `--emit-llvm/--emit-obj/--emit-asm`。
   - `crates/scoop/src/commands/build.rs`：引入 `BuildEmit/BuildOptions`，支持单文件输出 LLVM IR/object/asm；默认仍输出可执行文件。
   - `crates/scoopc/src/llvm/mod.rs`：新增 `emit_minimal_main_asm_to_file` 支持 `--emit-asm` 落盘。
   - `crates/scoop/src/fixtures/mod.rs`：新增 build phase（`tests/fixtures/build/**`），消费 `// ARGS: --emit-*` 并把产物写入 `target/fixtures/...`，同时断言产物存在且非空。
   - `tests/fixtures/build/emit_llvm_basic.scoop`：新增 fixture，通过 `// ARGS: --emit-llvm` 触发生成 `.ll`。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（会生成 `target/fixtures/build/emit_llvm_basic.ll`）。

### T0820 [DONE] sysroot：最小 I/O API（`print/println`）与字符串基础（spec §8）
- 描述：在 sysroot 声明最小 `print/println`（可标为 `@Extern` 或 `@Intrinsic`），并把 `String` 作为 reference type 的最小表面固定下来。
- 目标：先只声明 API；实现可在 runtime（C）中提供。
- 验收：resolve/typecheck fixture：`println("hi")` 可通过（至少到 typecheck）；未声明时报错。
- 依赖：T0418、T1001
 - 完成：
   - `sysroot/core.scoop`：新增 `print/println(String): Unit` 的最小声明。
   - `crates/scoopc/src/typecheck/expr.rs`：顶层函数调用在缺少“当前文件签名”时回退到 `Index` 查询（使 sysroot 顶层函数可在普通函数体中被调用并类型检查）。
   - `tests/fixtures/typecheck/println_string_ok.scoop`：新增回归用例覆盖 `print/println("hi")`。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0821 [DONE] runtime：最小字符串对象与 `scoop_println` 实现（C）
- 描述：实现 runtime 字符串承载（可先用 C 字符串包装）与打印函数，供 early run-pass 使用。
- 目标：先只支持 UTF-8 字面量与拼接后置；不实现完整 String API。
- 验收：clang 链接后调用 `scoop_println` 能输出（Scoop 侧 lowering/调用由后续 T0822 接入）。
- 依赖：T0820、T0106b2、T0902
 - 完成：
   - `runtime/c/scoop_runtime.c`：引入最小 `ScoopString` 承载与 `scoop_print/scoop_println`（stdout）。
   - `crates/scoop/src/toolchain.rs`：新增 clang + runtime 的 smoke test，断言 `scoop_println` 输出 `hi\\n`。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoop clang_can_link_object_with_runtime_and_println` 通过。

### T0822 [DONE] codegen：字符串字面量与调用 `println`（spec §8.1）
- 描述：把 `"..."` 与 raw string lowering 为 runtime 字符串对象（或常量指针），并生成对 `scoop_println` 的调用。
- 目标：先只支持纯字面量；插值字符串后续任务补。
- 验收：新增 run-pass fixture：`fun main(){ println(\"hello\") }` 输出 `hello`。
- 依赖：T0821、T0810
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：新增 `CgTy::String`（`scoop.core.String` → `*const ScoopString`），并实现字符串字面量 lowering：生成只读全局字节序列 + 栈上 `ScoopString { len, data }`。
   - `crates/scoopc/src/llvm/codegen.rs`：把 sysroot `scoop.core.print/println` 调用直接映射到 runtime `scoop_print/scoop_println`（C ABI）。
   - `tests/fixtures/codegen/println_string_literal.scoop`：新增 run-pass fixture，断言 `println("hello")` 的 stdout。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoopc --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0823 [DONE] f-string 插值：`f\"...{expr}...\"` 的 lowering（spec §8.2）
- 描述：实现插值字符串的 lowering：拆分为片段并拼接（或调用格式化 runtime），至少支持 `{Int}`/`{String}`。
- 目标：先不实现 `trimIndent`；先不做 locale/format spec。
- 验收：新增 run-pass fixture：`val s = f\"hi {name}\"; println(s)` 输出正确。
- 依赖：T0217、T0822、T0809
 - 完成：
   - `crates/scoopc/src/hir/*`：HIR 增加 `ExprKind::InterpolatedString` 与片段建模，并在 lowering 中保留 Text/Expr 分片。
   - `crates/scoopc/src/llvm/codegen.rs`：实现插值字符串 codegen：拼接 Text/Expr 片段到栈上 buffer，返回 runtime `ScoopString` 指针；`{Int}` 通过调用 runtime `scoop_format_{i64,u64}` 写入临时 buffer。
   - `runtime/c/scoop_runtime.c`：新增 `scoop_format_i64/scoop_format_u64`，用于最小整数 formatting（不引入堆分配依赖）。
   - `tests/fixtures/codegen/f_string_interpolation.scoop`：新增 run-pass fixture，覆盖 `{String}`/`{Int}` 插值并断言 stdout。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoopc --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0824 [DONE] tuple 字段访问语法对齐 spec：`._0` / `._1`（spec §2.3.3）
- 描述：补齐 tuple 字段访问的 lowering/codegen，并把相关 fixtures 与文档样例统一到 spec 语法 `t._0` / `t._1`。
- 目标：不修改既有任务定义；通过新增任务把语法差异显式收口。
- 验收：新增 run-pass fixture：`val t = (1,2); print(t._0 + t._1)` 输出 `3`；`t.0` 不作为合法 tuple 访问被接受。
- 依赖：T0812、T0210、T0410
 - 完成：
   - `sysroot/core.scoop`：新增 `print/println(Int)` overload，使 `print(t._0 + t._1)` 可通过前端检查。
   - `crates/scoopc/src/llvm/codegen.rs`：`print/println` 支持整数实参：调用 runtime `scoop_format_{i64,u64}` 格式化到栈上 buffer，再映射到 `scoop_print/scoop_println`。
   - `tests/fixtures/run-pass/tuple_access_print_sum.scoop`：新增 run-pass fixture，断言 stdout 为 `3`（含换行）。
   - `tests/fixtures/parse/tuple_access_numeric_member_not_allowed_fail.scoop`：新增 parse compile-fail fixture，确保 `t.0` 在 parser 阶段报错。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过。

### T0825 [DONE] codegen：`when` lowering 补齐 or-pattern / guard（spec §4.2）
- 描述：在已有 `when` lowering 基础上补齐 or-pattern 与 guard 的代码生成：or-pattern 共享后继块，guard 在匹配成功后再判定条件。
- 目标：先不追求最优 CFG；先保证语义正确与诊断稳定。
- 验收：新增 run-pass fixture：`A | B` 分支与 `pattern if cond` 分支都能得到正确结果。
- 依赖：T0814、T0429、T0450
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：`WhenPat` 增加 `Or { pats }` 变体。
   - `crates/scoopc/src/parser/expr.rs`：when pattern 支持解析 `A | B | C`。
   - `crates/scoopc/src/typecheck/expr.rs`：补齐 `when` 分支 guard 的类型检查（必须为 `Bool`）。
   - `crates/scoopc/src/typecheck/when_pat.rs` / `when_exhaustiveness.rs`：or-pattern 的绑定限制（当前不支持 binder）与穷尽性覆盖判定。
   - `crates/scoopc/src/hir/{mod.rs,lower.rs}`：HIR 接入 `WhenPat::Or`（含 closure capture 的 locals 收集遍历）。
   - `crates/scoopc/src/llvm/codegen.rs`：`when` 在出现 guard/or-pattern 时改用“链式判别 + guard fail 回落”的 CFG lowering。
   - `tests/fixtures/run-pass/when_or_pattern_and_guard_basic.scoop`：新增 run-pass fixture，断言 or-pattern 与 guard 分支行为（含 guard=false 回落）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoopc --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0826 [DONE] codegen：enum niche 优化 / oversized variant boxing / disparity lint（spec §2.3.2）
- 描述：为 rich enum 落实完整布局策略：能使用 niche 时消除显式 tag；oversized variant 自动 boxing；size disparity 明显时发出 lint warning。
- 目标：先覆盖 `Option<RefType>` 等高价值场景；更复杂嵌套 niche 后续可继续扩展。
- 验收：新增 codegen/run-pass fixture：`Option<RefType>` 正常工作；oversized variant case 通过并伴随 lint（warning 可先文本断言）。
- 依赖：T0449、T0813
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：`Option<T>` 在 codegen 侧接入 niche 表示（`None` 用非法值编码），并为 rich enum 补齐 oversized variant boxing（boxed payload 为指针）；`when` 支持对 boxed payload 解构绑定与 enum variant pattern 的 `..`（rest）。
   - `crates/scoop/src/fixtures/expectations.rs`：fixtures 指令新增 `RUN-STDOUT-CONTAINS`/`RUN-STDERR-CONTAINS`（子串断言），用于稳定断言含时间戳的 warning。
   - `crates/scoop/src/fixtures/run_pass.rs`：run-pass phase 支持 stdout/stderr 子串断言（与 golden 全文比对并存）。
   - `tests/fixtures/run-pass/option_ref_niche_basic.scoop`：新增 run-pass fixture，覆盖 `Option<String>`（niche）构造 + `when` 解构 + `println`。
   - `tests/fixtures/run-pass/enum_oversized_variant_boxing_basic.scoop`：新增 run-pass fixture，覆盖 multi-field oversized variant boxing，并用 `RUN-STDERR-CONTAINS` 断言 lint warning 关键子串。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - （可选：若本机安装 LLVM + llvm-config）`cargo run -p scoop --features llvm -- test` 通过并实际执行 run-pass fixtures。

### T0827 [DONE] `trimIndent()`：运行期 fallback 与字符串 API 对接（spec §8.4）
- 描述：当 `trimIndent()` 的接收者不是编译期常量时，生成普通运行期调用并接到最小字符串 API。
- 目标：先只支持最常见的 raw string 场景；不做额外格式化 API。
- 验收：新增 run-pass fixture：raw string 调用 `trimIndent()` 后输出去缩进结果；非 raw string 也可走同一路径。
- 依赖：T0822
 - 完成：
   - `crates/scoopc/src/resolve/scopes.rs`：resolver 对 `String.trimIndent()` 做内建放行（避免已知 String receiver 时误报 unresolved member）。
   - `crates/scoopc/src/typecheck/expr.rs`：以 intrinsic 形式固定最小类型规则：`String.trimIndent(): String`（编译期折叠留给 T1216）。
   - `crates/scoopc/src/llvm/codegen.rs`：为 `receiver.trimIndent()` 生成运行期调用 `scoop_string_trim_indent`，并返回 `ScoopString*`。
   - `runtime/c/scoop_runtime.c`：实现 `scoop_string_trim_indent`（`malloc` 分配输出，先不依赖 GC/`scoop_alloc`）。
   - `tests/fixtures/run-pass/string_trim_indent_basic.scoop`：新增 run-pass fixture 覆盖运行期 fallback 与二次调用（非 raw receiver）。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/string_trim_indent_basic.scoop` 输出与 golden 一致。

### T0828 [DONE] codegen：`object` / `companion object` 单例存储与成员访问（Appendix B.9）
- 描述：实现 `object` / `companion object` 的最小 codegen：单例存储、一次初始化、静态成员访问 lowering。
- 目标：先只覆盖单线程；线程安全初始化后续可由 runtime 原语增强。
- 验收：新增 run-pass fixture：多次访问同一 object 获得同一实例语义；`ClassName.member` 可访问 companion 成员。
- 依赖：T0452
- 完成：
  - `crates/scoopc/src/hir/mod.rs`：增加 `ObjectInitIndex/ObjectInitStep` side table，为后端提供 object/companion 的初始化顺序与属性元信息。
  - `crates/scoopc/src/hir/lower.rs`：在 lowering 阶段收集 object/companion 的属性 init 与 `init {}` 块（按源码顺序）。
  - `crates/scoopc/src/llvm/codegen.rs`：生成 module-local guard + init function（单线程 once），并在 `Foo.x`/`C.x` 访问点先调用 init，再读取全局 backing storage。
  - `crates/scoopc/src/llvm/mod.rs`：把 object init 索引注入 LLVM codegen。
  - `tests/fixtures/run-pass/object_companion_once_init_basic.scoop`：新增 run-pass fixture，覆盖 object/companion 的 once 初始化与 `ClassName.member` 静态访问。
- 验收：
  - `cargo test --all` 通过；
  - `cargo test -p scoopc --features llvm` 通过；
  - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

---

## T09：早期运行时（C + clang）（阶段 8：可执行与可观测）

### T0901 [DONE] runtime：补齐 `scoop_runtime_init` 的可观察行为（最小日志/断言）
- 描述：在 C runtime 初始化时设置最小全局状态，并可选输出 debug（受宏控制）。
- 目标：先不引入 GC；只让链接后的程序能调用 init 不崩溃。
- 验收：新增一个 Rust 集成测试（或小 C harness）调用 `scoop_runtime_init()` 通过；CI 通过。
- 依赖：T0014
 - 完成：
   - `runtime/c/scoop_runtime.c`：补齐 `scoop_runtime_init` 的一次初始化标记与可选 debug 日志（`SCOOP_RT_DEBUG`），并新增 `scoop_runtime_is_initialized/scoop_runtime_init_count` 供测试/调试观测。
   - `crates/scoop_runtime/build.rs`：显式声明 `rerun-if-changed`（runtime 源码位于 crate 目录之外），确保变更会触发重编译。
   - `crates/scoop_runtime/tests/runtime_init.rs`：新增 Rust 集成测试，验证 init 可调用、可重复调用且状态可观测。
 - 验收：
   - `cargo test --all` 通过。

### T0902 [DONE] runtime：实现 `scoop_alloc` 的最小可用版本（先用 `malloc`）
- 描述：把当前返回 0 的占位改为真正分配（暂时非 GC）。
- 目标：为后续 codegen 做最小保障；GC 语义后置。
- 验收：新增测试：调用 `scoop_alloc(16)` 返回非空；重复调用不崩溃。
- 依赖：T0901
 - 完成：
   - `runtime/c/scoop_runtime.c`：`scoop_alloc` 改为基于 libc `malloc` 的最小实现，并处理 `size=0` 与溢出场景（OOM 时返回 NULL）。
   - `crates/scoop_runtime/tests/alloc.rs`：新增集成测试，验证 `scoop_alloc(16)` 返回非空且可重复调用。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0817 [DONE] heap 分配：为 boxing/引用对象生成 `scoop_alloc` 调用（PLAN §9.1）
- 描述：在 codegen 中为 box/object 分配调用 runtime `scoop_alloc`，并写入最小对象头/类型描述指针（若已定义）。
- 目标：先只支持 boxing `Int`/简单对象；不实现移动 GC。
- 验收：新增 run-pass fixture：`val a: Any = 1` 运行不崩溃；并可通过调试打印确认对象非空。
- 依赖：T0902、T0441、T0810
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：新增 `CgTy::Ref`（`i8*`）用于承载 `Any` 等引用类型，并在 `Int -> Any` coercion 中调用 runtime `scoop_alloc` 进行 heap 装箱（对象头 type_desc 先写 `NULL`）。
   - `tests/fixtures/run-pass/boxing_int_to_any_basic.scoop`：新增 run-pass fixture 覆盖 `val a: Any = 1`（装箱）并运行期回归。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0903 [DONE] runtime：引入线程注册接口（占位）与 TLS 骨架
- 描述：提供 `scoop_thread_register/unregister`（先空实现），为 GC/effect TLS 铺路。
- 目标：API 稳定、可跨平台；实现可后置。
- 验收：链接通过；新增测试在主线程调用 register/unregister 不崩溃。
- 依赖：T0901
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 TLS 抽象宏（优先 `_Thread_local`）与 `ScoopThreadTls` 占位结构；实现 `scoop_thread_register/unregister`（幂等、空实现）并提供 `scoop_thread_is_registered` 便于测试观测。
   - `crates/scoop_runtime/tests/thread_registration.rs`：新增集成测试覆盖 register/unregister 的可调用性与幂等行为。
 - 验收：
   - `cargo test --all` 通过。

### T0904 [DONE] GC v0：mark-sweep 的数据结构骨架（不要求可用）
- 描述：定义 heap、object header、free list 等结构体与接口。
- 目标：先立结构与接口，具体算法/性能后续。
- 验收：C 编译通过（clang warnings as errors 若开启）；新增最小单测/运行时自检（可选）。
- 依赖：T0902
 - 完成：
   - `runtime/c/scoop_gc.{h,c}`：新增 mark-sweep GC 数据结构骨架（heap/object header/free list）与 `scoop_gc_heap_init/scoop_gc_self_check`。
   - `runtime/c/scoop_runtime.c`：引入 GC heap 单例并在 `scoop_runtime_init` 中初始化（不改变 `scoop_alloc` 语义）。
   - `crates/scoop_runtime/build.rs`：编译并链接 `scoop_gc.c`，同时增加 `rerun-if-changed`。
   - `crates/scoop/src/toolchain.rs`：clang 链接改为包含 `runtime/c/*.c`，避免新增模块后出现未定义符号。
   - `crates/scoop_runtime/tests/gc_self_check.rs`：新增 smoke test 覆盖 GC 自检符号可用性。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0905 [DONE] Shadow stack：定义 `GcFrame` 结构与 TLS 链（bootstrap roots；将由 T15 StackMap/statepoint roots 替换）
- 描述：在 runtime 中定义 `GcFrame { prev, roots[] }` 与 `current_frame` TLS。
- 目标：先不做扫描；只把数据结构与 push/pop API 做出来。
- 验收：新增测试：push/pop 两层 frame 后 `current_frame` 指针正确回退。
- 依赖：T0903
 - 完成：
   - `runtime/c/scoop_gc.h`：新增 `ScoopGcFrame`（prev + roots[]）与 shadow stack API 声明。
   - `runtime/c/scoop_runtime.c`：在线程 TLS 中维护 `gc_current_frame`，实现 `scoop_gc_frame_push/pop` 与 `scoop_gc_current_frame`。
   - `crates/scoop_runtime/tests/shadow_stack.rs`：新增集成测试覆盖两层 frame 的 push/pop 回退语义。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0816 [DONE] GC 接口：shadow stack 插桩（函数 prologue/epilogue）（bootstrap；将由 T15 StackMap/statepoint 管线替换）
- 描述：为包含 GC 引用的函数生成 `GcFrame` push/pop，并在需要处写 roots。
- 目标：先只支持单线程；先只插桩“明显活跃的引用局部变量”。
- 验收：新增 run-pass fixture：分配若干对象（先可用 malloc 代替 GC）并触发一次“伪 GC 扫描”（仅遍历 roots）不崩溃。
- 依赖：T0905、T0817
 - 完成：
   - `runtime/c/scoop_gc.h`：新增 `scoop_gc_debug_count_roots_current_thread` 声明（仅遍历 roots，不做真实 GC）。
   - `runtime/c/scoop_runtime.c`：实现 debug 扫描计数（带保守上限，避免破坏链表导致崩溃）。
   - `crates/scoop_runtime/tests/shadow_stack.rs`：新增单测覆盖 debug 扫描计数返回值。
   - `crates/scoopc/src/llvm/codegen.rs`：为含引用局部变量的函数生成 shadow stack frame push/pop，并把对应 locals 写入 roots slot；并把 sysroot debug 函数映射到 runtime 符号。
   - `sysroot/core.scoop`：声明 `__scoop_gc_debug_count_roots_current_thread(): Int`（fixtures 专用）。
   - `tests/fixtures/run-pass/gc_shadow_stack_instrumentation_basic.*`：新增 run-pass fixture 覆盖插桩与伪扫描。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0906 [DONE] effect runtime v0：TLS slot + flag（为 `->` handler 做准备）（PLAN §6.3.1）
- 描述：在 runtime 中增加 `__scoop_effect_active` 与 perform slot（结构体/union 先占位）。
- 目标：先不实现 dispatch；只提供 set/clear API。
- 验收：新增测试：set flag 后可读回；clear 后恢复初值。
- 依赖：T0903
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 TLS 符号 `__scoop_effect_active` 与 `__scoop_effect_perform_slot`（slot 结构占位），并提供 `scoop_effect_*` 的 set/clear/读回接口；在线程 unregister 时清空 TLS。
   - `crates/scoop_runtime/tests/effect_tls.rs`：新增集成测试覆盖 active flag 的 set/clear 幂等与 unregister 清理语义。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0613 [DONE] lowering step 1（部分）：定义 runtime ABI（perform slot + flag）并在 codegen 侧可调用
- 描述：固定 runtime C ABI（函数/全局符号名），codegen 能生成对其的读写调用。
- 目标：先只支持单个 slot 类型（例如指针/整型）；复杂 payload 后续。
- 验收：`--emit-llvm` 产物里包含对 runtime 符号的引用；链接阶段不报未定义符号。
- 依赖：T0906、T0804
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 perform slot 的最小读写 ABI（`scoop_effect_perform_slot_write_u64/read_*`）。
   - `sysroot/core.scoop`：新增 `__scoop_effect_*` 测试辅助 API 声明。
   - `crates/scoopc/src/llvm/codegen.rs`：把 sysroot `__scoop_effect_*` 映射到 runtime C 符号，并在 `--emit-llvm` 下生成调用。
   - `crates/scoop_runtime/tests/effect_tls.rs`：新增集成测试覆盖 slot 的写回/清空/unregister 清理语义。
   - `crates/scoopc/src/llvm/mod.rs`：新增单测断言 IR 包含 effect runtime 符号引用。
   - `tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop`：新增 run-pass fixture 覆盖“可编译+可链接+可执行”。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（包含该 fixture）。

### T0614 [DONE] lowering step 1（部分）：`Raise.raise` 的 flag-based unwinding（只支持最小示例）
- 描述：实现 `Raise.raise(e)`：写 perform slot + set flag + 早退；调用边界检查 flag 并向外传播；try/catch 在边界消费 slot。
- 目标：先只支持 `Raise` + `try/catch`（无 finally、无用户自定义 effect）；先不支持跨函数捕获复杂状态。
- 验收：新增 run-pass fixture：`try { Raise.raise(...) } catch { ... }` 能运行并输出预期；新增 compile-fail：未处理 Raise 报 required effects。
- 依赖：T0613、T0106b2、T0807
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：支持 HIR `Perform/Handle` 的最小 lowering：
     - `Raise.raise(e)` 写入 runtime perform slot（`op_tag + value`）并置位 flag；
     - 在 handler boundary 内跳到 catch；否则返回默认值向外传播；
     - 普通顶层函数调用返回后检查 flag，并按“最近 handler / 向外传播”规则 unwind。
   - `crates/scoopc/src/resolve/mod.rs`：修复 `type_path_to_fqn_in_file` 对合成 Ident 的处理（`Ident.text(...)`），使 try/catch lowering 的 `Raise` 目标能解析为 `scoop.core.Raise.raise`。
   - `tests/fixtures/run-pass/try_catch_raise_int_basic.scoop`：新增 run-pass 用例（stdout golden）回归 try/catch 捕获与跨函数传播。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0615 [DONE] lowering step 1（补齐）：`finally` 的清理语义（spec §5.7）
- 描述：确保 `finally` 在正常路径与 raise/unwind 路径都执行一次。
- 目标：先只支持 try/catch/finally；不支持 nested handler stack。
- 验收：新增 run-pass fixture：finally 中打印日志；无论 raise 与否都出现一次且顺序正确。
- 依赖：T0614、T0707
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：LLVM codegen 支持 `handle ... finally { ... }`：
     - body 正常结束与 catch 返回都会执行 finally；
     - catch 内再次发生 raise 时，会先执行 finally 再向外传播（不在本 handler 内清 flag/slot）。
   - `tests/fixtures/run-pass/try_catch_finally_raise_int_basic.scoop`：新增 run-pass fixture 覆盖 try/catch/finally 在 raise 与非 raise 两条路径的执行顺序。
   - `tests/fixtures/run-pass/try_catch_finally_raise_int_basic.stdout`：新增 stdout golden。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0616 [DONE] lowering step 2：`-> resume`（栈 state machine）（PLAN §6.3.2）
- 描述：把 handle body 分段、提升跨段 locals，并用 while-loop state machine 实现立即恢复。
- 目标：先只支持单个 perform 点；`resume` 必须恰好一次的检查可先只做运行期断言。
- 验收：新增 run-pass fixture：自定义 effect + `-> resume` 能恢复并继续执行；多次 resume 报错（运行期）。
- 依赖：T0615、T0703
 - 完成：
   - `crates/scoopc/src/parser/expr.rs`：支持 `handle ... with { Effect.op(...) -> resume { ... } }` 的语法解析，并标记 arm kind。
   - `crates/scoopc/src/ast/mod.rs`：AST handle arm 增加 `kind`（NonResuming / ImmediateResume），并自定义 `Debug` 尽量保持 parse snapshot 稳定。
   - `crates/scoopc/src/resolve/scopes.rs`：在 `-> resume` arm scope 内注入局部 `resume` 标识符（供 typecheck/codegen 侧识别与特判）。
   - `crates/scoopc/src/hir/mod.rs`、`crates/scoopc/src/hir/lower.rs`：HIR 侧建模 `HandleArmKind`，lowering 时为 resume 生成局部 `SymbolId`。
   - `crates/scoopc/src/typecheck/expr.rs`：为 `-> resume` arm 注入 `resume: (T) -> Unit` 的局部函数值类型（T 为 op 返回类型），并在 v0 阶段禁止同一 handle 混用 `->` 与 `-> resume` 两类 arm。
   - `crates/scoopc/src/llvm/codegen.rs`：实现单 perform 点的 while-loop state machine lowering；`resume(value)` one-shot 运行期断言（多次/未调用均 `exit(3)`）。
   - `tests/fixtures/run-pass/effect_resume_yield_int_basic.scoop`：新增 run-pass 覆盖自定义 effect + `-> resume` 恢复后继续执行。
   - `tests/fixtures/run-pass/effect_resume_double_resume_exit.scoop`：新增 run-pass 覆盖重复 resume 的 `EXPECT-EXIT: 3` 断言。
   - `tests/fixtures/parse/handle_expr_arm_recovery_two_errors.scoop`：更新 parse fail 断言，适配新的恢复路径。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0619 [DONE] async/await（作为 `Async` effect 的语法糖）（spec §5.7）
- 描述：解析并 typecheck `async/await`，lowering 到 effect perform/handle（或库函数）模型。
- 目标：先只实现单线程、无取消；spawn/结构化并发后续。
- 验收：新增 run-pass fixture：最小 async/await demo 输出正确；required effects 规则一致。
- 依赖：T0616、T0807
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `ExprKind::Async` / `ExprKind::Await` 语法节点。
   - `crates/scoopc/src/parser/expr.rs`：支持解析 `async { ... }` 与前缀 `await expr`。
   - `crates/scoopc/src/typecheck/expr.rs`：`await` 计入 `Async` required effects；`async` 捕获 `Async` 使其不向外传播（当前先只支持 `Int`）。
   - `crates/scoopc/src/hir/lower.rs`：`async` lowering 为 `handle` + `-> resume` 的同步 handler；`await` lowering 为 `Perform(scoop.core.Async.await)`。
   - `crates/scoopc/src/syntax/lexer.rs`：把 `await` 调整为“上下文关键字”（作为 ident token），以便 sysroot 声明 `fun await(...)`。
   - `sysroot/core.scoop`：加入内建 `effect Async` 的最小声明（`await(value: Int): Int`）。
   - `tests/fixtures/run-pass/async_await_minimal_int_basic.*`：新增 run-pass fixture + stdout golden 回归。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0620 [DONE] `spawn`：结构化并发最小模型（spec §5.7）
- 描述：实现 `spawn` 语法糖与 runtime 支持（join/取消语义先简化）。
- 目标：先只支持 join；取消后置。
- 验收：新增 run-pass fixture：spawn 两个任务并 join；输出顺序/值正确。
- 依赖：T0619
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：AST 新增 `ExprKind::Spawn` / `ExprKind::Join`。
   - `crates/scoopc/src/parser/expr.rs`：新增 `spawn { ... }` 与 `join expr` 的解析（上下文关键字）。
   - `crates/scoopc/src/resolve/scopes.rs`：scope checker 覆盖 spawn/join 递归检查。
   - `crates/scoopc/src/typecheck/expr.rs`：typecheck 接入 spawn/join，并把它们计入 `Async` performed effects（确保 `async { ... }` 可捕获）。
   - `crates/scoopc/src/hir/lower.rs`：lowering 把 spawn/join desugar 为 `scoop.core.__scoop_task_{spawn,join}_int` 调用（避免依赖 closure codegen）。
   - `sysroot/core.scoop`：新增内部 runtime helper 声明：`__scoop_task_spawn_int` / `__scoop_task_join_int`。
   - `runtime/c/scoop_runtime.c`：实现 `scoop_task_spawn_int` / `scoop_task_join_int`（one-shot join；当前不提供真实并行/取消）。
   - `crates/scoopc/src/llvm/codegen.rs`：把 sysroot helper 映射到 runtime C 符号，并处理 word ↔ i64/u64 cast。
   - `crates/scoop_runtime/tests/task_spawn_join.rs`：新增 runtime 集成测试覆盖 spawn/join roundtrip。
   - `tests/fixtures/typecheck/entry_point_main_spawn_join_async_ok.scoop`：新增 typecheck fixture 回归 entry point 纯度。
   - `tests/fixtures/run-pass/spawn_join_int_basic.*`：新增 run-pass fixture + stdout golden 回归输出顺序。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - （可选，需 LLVM）`PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过。

### T0622 [DONE] `Task<T>`：类型/库模型与 lazy 语义（spec §5.3 / §5.7）
- 描述：在 sysroot/type system 中引入 `Task<T>` 的最小模型，固定“懒执行直到 `await` 或显式启动”的语义，并为 `spawn/async` 共享同一任务抽象。
- 目标：先只固定类型面与基础语义；取消/结构化并发细节后续。
- 验收：effects/typecheck fixture：`val t: Task<Int> = ...` 合法；`await` 仅接受 `Task<T>`；未启动任务不要求立即执行。
- 依赖：T0611、T0820
 - 完成：
   - `sysroot/core.scoop`：新增 `Task<T>` 声明；把 `Async.await` 升级为 `fun <T> await(task: Task<T>): T`；调整 sysroot task helper 的签名为 `Task<Int>` 句柄。
   - `crates/scoopc/src/typecheck/lower.rs`：把 `Task` 纳入 implicit builtin type FQN 映射（允许写 `Task<Int>`）。
   - `crates/scoopc/src/typecheck/expr.rs`：`spawn` 返回 `Task<Int>`；`await/join` 仅接受 `Task<T>` 并返回 `T`，同时维持 `Async` required-effects 传播规则。
   - `crates/scoopc/src/hir/lower.rs`：`async { ... }` 的 handler arm 将 `await task` 同步 `join` 取回 `Int` 结果后再 `resume`（避免把 task 句柄透传为结果）。
   - `crates/scoopc/src/llvm/codegen.rs`：把 `Task<T>` 视为 word-sized `UInt` 句柄类型，并在 sysroot task intrinsics 上按 `uint64_t handle` 做有符号/无符号转换。
   - fixtures：
     - `tests/fixtures/typecheck/entry_point_main_spawn_join_async_ok.scoop`：升级为 `Task<Int>` + `await` 覆盖。
     - `tests/fixtures/typecheck/await_arg_not_task_is_error.scoop`：新增 compile-fail 覆盖 `await 1` 报错。
     - `tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`、`tests/fixtures/run-pass/spawn_join_int_basic.scoop`：升级为 `Task<Int>` 句柄，保持可运行回归。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T0623 [DONE] `async fun`：desugar 到 `fun ...: Task<T>`（spec §5.3 / §5.7）
- 描述：实现 `async fun foo(): T` 的签名与 lowering 规则：对外暴露 `Task<T>`，而不是 `T / Async`；`/ Async` 只存在于 Task 的计算上下文。
- 目标：先只覆盖函数声明与调用点；与 executor 的交互后续由 runtime 任务补齐。
- 验收：effects fixture：`async fun fetch(): Int` 的调用点类型为 `Task<Int>`；把它当作 `Int / Async` 使用时报错。
- 依赖：T0619、T0622
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `Modifier::Async`（语法层建模）。
   - `crates/scoopc/src/parser/decls.rs`、`crates/scoopc/src/parser/cursor.rs`：把 `async` 纳入 modifiers 解析/前瞻。
   - `crates/scoopc/src/resolve/mod.rs`：Index 侧把 `async fun` 的 return type 写为 `Task<T>`（跨文件调用点可见）。
   - `crates/scoopc/src/typecheck/expr.rs`：调用点签名返回 `Task<T>`；`async fun` 函数体内的 `Async` performed effects 不外泄到声明处 required effects。
   - `crates/scoopc/src/hir/lower.rs`：HIR 侧把 `async fun` 返回类型降为 task handle（`UInt`），并将 `return`/尾表达式包装为 `__scoop_task_spawn_int(...)`（保持 early-stage 可回归）。
   - fixtures：
     - `tests/fixtures/typecheck/async_fun_returns_task_ok.scoop`：pass 覆盖 `Task<Int>` 返回与 `await/spawn` 可用。
     - `tests/fixtures/typecheck/async_fun_used_as_value_is_error.scoop`：fail 覆盖误用为 `Int` 的诊断。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0818 [DONE] effect codegen：flag-based Raise/try-catch（对接 T0614）
- 描述：把 MIR 中的 perform/handle terminator 生成 LLVM IR，与 runtime slot/flag 交互，实现最小 Raise 处理。
- 目标：先只支持 `Raise<RuntimeError>` 与 try/catch；finally 在 T0615 补齐。
- 验收：run-pass fixtures：Raise 被 catch 捕获；未捕获时报错或退出（按设计）。
- 依赖：T0713、T0614
 - 完成：
   - `crates/scoopc/src/resolve/mod.rs`：为 enum 注入 0-参数（unit）variant 的 value symbol，使 `EnumName.Variant` 可被 resolver 写回（最小支持 `RuntimeError.NullAssertionFailed`）。
   - `crates/scoopc/src/resolve/scopes.rs`：member access receiver 推导支持 enum “值命名空间入口”，让 `EnumName.Variant` 走 `resolve_member_access_on_value_receiver` 路径。
   - `crates/scoopc/src/typecheck/expr.rs`：member access 对 enum unit variant 值做最小类型推断与 receiver 跳过，避免把 enum type name 当作普通顶层值推导而报错。
   - `crates/scoopc/src/llvm/codegen.rs`：
     - `Raise.raise` payload 编码支持 `RuntimeError`（把 enum tag 写入 slot），并在 catch 侧恢复 enum 值；
     - codegen 支持 `EnumName.UnitVariant` 的 member access 产出 enum 常量；
     - 修复 sysroot effect/task intrinsics 中 word/handle 类型变量名不一致导致的 `--features llvm` 编译失败。
   - fixtures：
     - `tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`：新增 run-pass 覆盖 `Raise<RuntimeError>` 捕获；
     - `tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.stdout`：新增 stdout golden；
     - `tests/fixtures/run-pass/spawn_join_int_basic.stdout`：修复多余空行导致的 stdout golden mismatch。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0630 [DONE] runtime ABI：perform slot 从单值扩展到复杂 payload
- 描述：把 effect runtime ABI 从“单个指针/整型 slot”扩展到可承载复杂 payload：多字字段、结构体/union 风格载荷、必要的对齐与判别信息。
- 目标：保持 non-resuming 路径的实现简单；先固定 ABI 形状，再让 lowering/codegen/runtime 共同接入。
- 验收：新增 IR/codegen/runtime fixtures：携带复合 payload 的 effect 可被正确写入、传播和读取；ABI 在同一 target 上稳定。
- 依赖：T0613、T0818、T0906
 - 完成：
   - `runtime/c/scoop_runtime.c`：
     - perform slot ABI 升级为 `op_tag + payload_len_words + payload_words[8]`（固定 offset/size 并加 `_Static_assert`）；
     - 新增多 word 读写 API：`scoop_effect_perform_slot_write_u64_2`、`scoop_effect_perform_slot_read_len_words`、`scoop_effect_perform_slot_read_u64_at`。
   - `sysroot/core.scoop`：新增多 word 测试辅助 intrinsics：`__scoop_effect_slot_write2`、`__scoop_effect_slot_read_len_words`、`__scoop_effect_slot_read_word`（保留单 word API 兼容）。
   - `crates/scoopc/src/llvm/codegen.rs`：
     - `Raise.raise` payload 升级为 2-word `(kind, value)` union 风格编码；
     - try/catch handler 边界读取 2 words + 运行期断言（len/kind），并清 flag/slot 后执行 arm（保持 flag-based unwinding 语义）。
     - sysroot effect intrinsics 映射新增多 word 读写入口，并补齐 runtime 符号声明。
   - 测试/fixtures：
     - `crates/scoop_runtime/tests/effect_tls.rs`：新增多 word 读写与越界读回归；
     - `crates/scoopc/src/llvm/mod.rs`：LLVM IR 单测更新为断言多 word ABI 符号；
     - `tests/fixtures/run-pass/effect_runtime_slot_abi_basic.scoop`：升级为多 word payload，用退出码断言读写结果。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0907 [DONE] runtime：类型描述（type descriptor）v0（trace bitmap/回调）
- 描述：定义 type descriptor 结构（大小、字段 trace 信息），供 GC 扫描对象内引用字段使用。
- 目标：先只支持 struct/box；interface/class 后续。
- 验收：新增 C/Rust 测试：构造一个 descriptor 并用它扫描一段内存（假设布局）不越界。
- 依赖：T0904
 - 完成：
   - `runtime/c/scoop_gc.{h,c}`：定义 `ScoopTypeDescriptor`（`size_bytes/trace_start_offset_bytes/trace_bitmap` 与可选 `trace_fn`），并实现 `scoop_gc_type_descriptor_trace`（按 `size_bytes` 裁剪扫描范围，避免越界）。
   - `crates/scoop_runtime/tests/type_descriptor.rs`：新增 Rust 集成测试：
     - bitmap 仅访问被标记的槽位；
     - guard page 断言：当 bitmap 远大于对象大小时，trace 仍不会越界读取。
   - `crates/scoop_runtime/Cargo.toml`：新增 `libc` dev-dependency 以支持 `mmap/mprotect` 测试。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0908 [DONE] runtime：对象头（object header）与最小 heap 对象布局
- 描述：定义 heap 对象头：指向 type descriptor、flags/size 等，配合 `scoop_alloc` 返回指针。
- 目标：先只实现非移动对象；不实现压缩。
- 验收：新增测试：alloc 后 header 字段可读写；对齐满足基本要求。
- 依赖：T0902、T0907
 - 完成：
   - `runtime/c/scoop_gc.h`：固化 `ScoopGcObjectHeader` 的关键字段偏移（`_Static_assert`），并补充对象布局约定（`scoop_alloc` 返回 header 起始地址，payload 紧随其后）。
   - `runtime/c/scoop_runtime.c`：`scoop_alloc` 统一初始化对象头（`next/type_desc/size/flags/mark`），并对 `size=0`/`size < header` 做保守处理（至少分配对象头大小）。
   - `crates/scoop_runtime/tests/object_header.rs`：新增集成测试，断言对象头字段默认值、可写回，以及基础对齐满足要求。
   - `crates/scoop_runtime/tests/alloc.rs`：写入测试改为写 payload（避免覆盖对象头）。
   - `crates/scoopc/src/llvm/codegen.rs`：同步更新 `Int -> Any` 装箱布局为 `{ ScoopGcObjectHeader, payload }`，并新增对应的 LLVM struct 类型定义（保持与 C runtime 对齐）。
 - 验收：
   - `cargo test --all` 通过。

### T0909 [DONE] GC v0：shadow stack root 扫描（单线程）
- 描述：实现扫描当前线程 `GcFrame` 链并枚举 roots，供 mark 阶段使用。
- 目标：先只支持单线程；不 stop-the-world。
- 验收：新增测试：构造 2 层 frame，每层 2 个 roots，扫描回收集到 4 个 roots。
- 依赖：T0905、T0907
 - 完成：
   - `runtime/c/scoop_gc.h`：新增 roots 扫描 API：`scoop_gc_shadow_stack_visit_roots_current_thread`（visitor 形式枚举 roots slots）。
   - `runtime/c/scoop_runtime.c`：实现遍历 `ScoopGcFrame` 链并对每个非空 slot 调用 visitor；并让 `scoop_gc_debug_count_roots_current_thread` 复用该扫描逻辑。
   - `crates/scoop_runtime/tests/shadow_stack.rs`：新增集成测试：构造两层 frame（每层 2 个 roots）并断言扫描到 4 个 roots。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0910 [DONE] GC v0：最小 mark-sweep（单线程）可用版本
- 描述：在 `scoop_alloc` 中分配对象并记录到 heap 列表；实现一次 mark-sweep（手动触发）。
- 目标：先不做触发策略；先提供 `scoop_gc_collect()` 手动调用。
- 验收：新增 run-pass fixture：分配大量对象并手动触发 collect，不崩溃且能回收未引用对象（可用计数验证）。
- 依赖：T0909、T0106b2
 - 完成：
   - `runtime/c/scoop_gc.h`：新增 `scoop_gc_collect()` 与 heap debug API（object count / bytes allocated / bytes freed），并新增 `scoop_gc_debug_alloc_garbage()`。
   - `runtime/c/scoop_gc.c`：实现单线程 mark-sweep（roots mark + 可选 type descriptor trace + sweep），并提供 heap 统计与 garbage alloc helper。
   - `runtime/c/scoop_runtime.c`：`scoop_alloc` 分配后登记到 heap 链表、累计 `bytes_allocated`，并在未 init 场景下自动 init。
   - `sysroot/core.scoop`：新增 `__scoop_gc_collect/__scoop_gc_debug_heap_object_count/__scoop_gc_debug_alloc_garbage`（测试辅助）。
   - `crates/scoopc/src/llvm/codegen.rs`：把上述 sysroot helper 映射到 runtime C 符号，完成参数/返回值的整数位宽转换。
   - `crates/scoop_runtime/tests/gc_mark_sweep.rs`：新增集成测试覆盖“root 保活 + sweep 回收”。
   - `crates/scoop_runtime/tests/alloc.rs`、`crates/scoop_runtime/tests/object_header.rs`：测试改为使用 `scoop_gc_collect()` 回收对象（不再直接 `free`）。
   - `tests/fixtures/run-pass/gc_mark_sweep_basic.scoop`：新增 run-pass fixture（制造垃圾 + collect + 计数验证）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0911 [DONE] 线程注册 + stop-the-world 扫描所有线程（PLAN §9.1）
- 描述：实现线程注册表，GC 时暂停所有注册线程并扫描其 shadow stack。
- 目标：先只支持 2 线程；暂停策略可先用全局 mutex + 条件变量。
- 验收：新增运行期测试：两线程各自持有对象引用，GC 扫描到两边 roots；程序不崩溃。
- 依赖：T0903、T0910
 - 完成：
   - `runtime/c/scoop_gc.c`：引入线程注册表 + 协作式 stop-the-world（mutex/cond + safepoint），并让 `scoop_gc_collect()` 在 STW 下扫描所有线程的 shadow stack roots。
   - `runtime/c/scoop_runtime.c`：`scoop_thread_register/unregister` 接入 GC 线程表；`scoop_alloc`/`scoop_gc_frame_push/pop` 增加 safepoint；`scoop_runtime_init()` 升级为线程安全幂等。
   - `crates/scoop_runtime/tests/gc_stop_the_world.rs`：新增多线程回归测试，验证 GC 能保活两线程 roots。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0912 [DONE] pin/unpin API（spec §15.10 / PLAN §9.1）
- 描述：在 runtime 中提供 `scoop_pin/scoop_unpin`，并定义 pin 计数或列表，为未来移动 GC 做准备。
- 目标：在非移动 GC 中可先是计数/no-op，但语义与错误检查要固定。
- 验收：新增测试：pin/unpin 计数配对；重复 unpin 报错或断言。
- 依赖：T0910
 - 完成：
   - `runtime/c/scoop_gc.h`：新增 `scoop_pin/scoop_unpin` 声明并固化 v0 返回值语义（成功=1/失败=0）。
   - `runtime/c/scoop_gc.c`：实现 per-object pin 计数；GC 时把 pinned 对象作为额外 roots 标记保活；unpin 下溢返回失败。
   - `crates/scoop_runtime/tests/pin_unpin.rs`：新增集成测试：pin 两次/unpin 两次配对；pin 期间无 roots 仍保活；重复 unpin 返回失败。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0913 [DONE] effect runtime：handler stack push/pop + 最近匹配分发规则（Appendix A）
- 描述：实现 handler stack（TLS），并按“最近匹配 handler”分发；arm body 在 dispatch scope 外执行。
- 目标：先只支持单层 handler；多层嵌套后续。
- 验收：新增 run-pass fixture：嵌套 handle 时最近者优先；在 arm 内再次 perform 不会捕获到同一个 handler（按 Appendix A.4）。
- 依赖：T0906、T0106b2
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 `ScoopEffectHandlerFrame` + TLS handler stack（push/pop/find_nearest/set_active），并在 `scoop_thread_unregister` 时清理。
   - `crates/scoopc/src/llvm/codegen.rs`：`handle`（try/catch）与 `-> resume` lowering 期间维护 handler stack；arm body 执行期间将当前 handler 置为 inactive（Appendix A.4）。
   - `crates/scoopc/src/llvm/codegen.rs`：入口 `main` 也插桩 GC frame push/pop，修复 run-pass `gc_mark_sweep_basic` 在 main 触发 GC 时 roots 丢失的问题。
   - `crates/scoop_runtime/tests/effect_handler_stack.rs`：新增集成测试覆盖 handler stack push/pop、最近匹配查询与 inactive 跳过语义。
   - `tests/fixtures/run-pass/effect_handler_stack_nearest_and_arm_outside_scope.*`：新增 run-pass fixture 回归最近匹配与 arm self-capture 避免。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0914 [DONE] continuation 对象：one-shot 状态位 + resume API（PLAN §6.3.3）
- 描述：定义 continuation 结构：捕获 handler stack + 目标状态；实现原子 one-shot。
- 目标：先只支持单线程；并发 resume 后续。
- 验收：新增运行期测试：同一 continuation resume 两次，第二次失败（返回错误码或 abort，需固定）。
- 依赖：T0913
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 `ScoopContinuation`（捕获 handler stack + state/step_fn）与 one-shot 原子状态位；实现 `scoop_continuation_alloc/scoop_continuation_try_resume`。
   - `crates/scoop_runtime/tests/continuation_one_shot.rs`：新增集成测试覆盖“捕获 handler stack”与“重复 resume 第二次失败”。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0921 [DONE] continuation：重复 `resume` 不再 `exit(3)`，改为 `Raise<RuntimeError.ContinuationAlreadyResumed>`
- 描述：按 `SCOOP_FULL_SPEC.md` §5.5 与 §5.7 的规范，把 “one-shot 违规” 从进程级 abort/exit 改为可被语言层捕获的运行时错误：第二次 `resume` 执行 `Raise.raise(RuntimeError.ContinuationAlreadyResumed)`。
- 目标：
  - sysroot：`enum RuntimeError` 增加 `ContinuationAlreadyResumed`（并更新相关文档与 fixtures）。
  - typecheck：`k.resume(value)` 的 required effects 为 `E + Raise<RuntimeError>`（其中 `E` 为 `Continuation<T, eff E>` 的 row 参数）。
  - runtime/codegen：
    - `scoop_continuation_try_resume`/`scoop_continuation_resume_*` 在 one-shot 检测失败时不再 `exit(3)`；
    - 失败路径写入 perform slot（payload 为 `RuntimeError.ContinuationAlreadyResumed`）并置位 flag，走既有 Raise 的传播/捕获路径（try/catch/handle）。
  - fixtures：
    - 更新 `tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.*`：第二次 `resume` 用 `try/catch (e: RuntimeError)` 捕获并输出；不再依赖退出码断言；
    - 新增 typecheck fixture：`Pure` 函数里调用 `k.resume(...)` 会要求 `Raise<RuntimeError>`（除非被 handle/try 捕获）。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`（若启用 LLVM 后端）
- 依赖：T0617、T0614、T0630、T0445、T0611
 - 完成：
   - `sysroot/core.scoop`：`RuntimeError` 增加 `ContinuationAlreadyResumed`（unit variant）。
   - `crates/scoopc/src/typecheck/expr.rs`：`k.resume(value)` required effects 从 `E` 升级为 `E + Raise<RuntimeError>`。
   - `runtime/c/scoop_runtime.c`：重复 `resume` 不再 `exit(3)`，改为写入 Raise perform slot + set flag（payload 为 `RuntimeError.ContinuationAlreadyResumed`）。
   - `crates/scoopc/src/llvm/codegen.rs`：`k.resume(value)` call-site 增加 effect flag 检查与 unwind（复用 Raise 的最小传播约定）。
   - fixtures：
     - `tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.*`：改为 try/catch 捕获并输出；不再断言退出码。
     - `tests/fixtures/run-pass/try_catch_raise_runtime_error_basic.scoop`、`tests/fixtures/run-pass/kotlin_require_check*.scoop`：补齐 `RuntimeError` 的 `when` 穷尽分支。
     - `tests/fixtures/run-pass/generator_yield_iter_int_basic.scoop`：`k.resume(...)` 包裹 try/catch 以满足 `Raise<RuntimeError>` 的静态要求。
     - `tests/fixtures/typecheck/continuation_type_and_resume_pure_ok.scoop`：显式声明 `/ Raise<RuntimeError>`。
     - `tests/fixtures/typecheck/continuation_resume_requires_raise_runtime_error_missing_is_error.scoop`：新增回归（Pure 中调用 `k.resume` 缺少 Raise 声明应报错）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T0915（拆分为子任务）
- 描述：跨线程 `resume` 时需要把 continuation 捕获的 handler stack 安装到当前线程 TLS，并在返回后恢复原 TLS（spec §5.5）。
- 目标：先不支持并发同时 resume；只支持单次跨线程。
- 备注：端到端 run-pass fixture 依赖编译器侧对堆 continuation（`, k ->`）与跨线程执行链路的接入；为保证“可单独实现 & 单独验证”，先拆成运行时原语与端到端 fixture 两步。

### T0915a [DONE] runtime：continuation resume 时切换/恢复 TLS handler stack（spec §5.5）
- 描述：在 runtime 中提供“临时安装 captured handler stack → 执行 step_fn → 恢复原 TLS handler stack”的原语；允许在另一线程执行。
- 目标：只实现 TLS handler stack 的切换/恢复；不要求编译器立即接入 `, k ->`；不实现并发同时 resume。
- 验收：新增运行期测试（`crates/scoop_runtime/tests`）：跨线程调用 resume 原语时，step_fn 看到的 handler stack top 为 captured 值，且返回后 TLS 恢复为原值；`cargo test -p scoop_runtime` 通过。
- 依赖：T0914、T0911
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 `scoop_effect_handler_stack_swap_top` 与 `scoop_continuation_resume_u64`，在 resume 期间安装 captured handler stack 并在返回后恢复原 TLS。
   - `crates/scoop_runtime/tests/continuation_cross_thread_handler_stack.rs`：新增跨线程回归测试，验证 step_fn 观察到的 handler stack top 与恢复语义。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

### T0617 [DONE] lowering step 3：`, k ->`（堆 continuation + one-shot）（PLAN §6.3.3）
- 描述：实现 continuation 对象捕获 handler stack，支持跨线程 `resume`，并用原子状态位保证 one-shot。
- 目标：先只支持单线程 resume；跨线程作为后续子任务。
- 验收：新增 run-pass fixture：保存 continuation 后稍后 resume；重复 resume 失败（错误/诊断明确）。
- 依赖：T0616、T0914
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：`handle` arm 语法新增 `, k ->`（escape continuation）节点。
   - `crates/scoopc/src/parser/expr.rs`：支持解析 `Effect.op(...), k -> body` 形式的 handler arm。
   - `crates/scoopc/src/resolve/scopes.rs`：在 `, k ->` arm body 作用域内注入局部标识符 `k`。
   - `crates/scoopc/src/hir/mod.rs`、`crates/scoopc/src/hir/lower.rs`：lowering 到 HIR `HandleArmKind::EscapeContinuation`。
   - `crates/scoopc/src/typecheck/expr.rs`：为 `, k ->` arm 注入 `k: Continuation<T>`；并补齐赋值表达式在表达式语境下的 typecheck（用于 arm body）。
   - `crates/scoopc/src/llvm/codegen.rs`：最小可回归的 escape continuation codegen：
     - `handle` arm 支持 `EscapeContinuation`；
     - `k.resume(value)` lowering 到 `scoop_continuation_resume_u64`；
     - arm body 执行前将 handler 从 TLS 栈顶摘除，避免 arm 自捕获（Appendix A.4）。
   - `tests/fixtures/run-pass/effect_escape_continuation_resume_later_exit.scoop`：新增端到端 fixture：
     - 保存 continuation 后稍后 `resume(42)`，并打印恢复后的值；
     - 第二次 `resume` 触发 one-shot 运行期错误（v0 固定 `exit(3)`；规范已要求改为 `Raise<RuntimeError.ContinuationAlreadyResumed>`，见 T0921）。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0618 [DONE] 跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（spec §5.5）
- 描述：实现跨线程 resume 的语义与 runtime 支持（TLS handler stack 切换）。
- 目标：先只支持 2 线程；不实现调度器。
- 验收：新增 run-pass fixture：在新线程 resume continuation，程序输出符合预期且不崩溃。
- 依赖：T0617、T0915
 - 完成：
   - `runtime/c/scoop_runtime.c`：新增 `scoop_thread_spawn_join_resume_u64`（pthread spawn + join），在线程内调用 `scoop_continuation_resume_u64` 并在退出前 `scoop_thread_unregister` 清理注册状态。
   - `sysroot/core.scoop`：新增 sysroot 内部 helper：`__scoop_thread_spawn_join_resume_u64(Continuation<Int>, Int)`。
   - `crates/scoopc/src/llvm/codegen.rs`：将 `scoop.core.__scoop_thread_spawn_join_resume_u64` 映射到 runtime 符号 `scoop_thread_spawn_join_resume_u64`。
   - `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.*`：新增端到端 fixture（跨线程 resume）与 stdout golden。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T0915b [DONE] run-pass：跨线程 resume 的端到端 fixture（spec §5.5）
- 描述：在 Scoop fixtures 中新增 run-pass case：在新线程 `resume` continuation（或等价语义），输出与单线程一致。
- 目标：只做端到端回归；不引入调度器/executor；线程数先固定为 2。
- 验收：新增 run-pass fixture；`PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过。
- 依赖：T0915a、T0618、T0106b2
 - 完成：
   - `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop`：跨线程 `resume` continuation（`__scoop_thread_spawn_join_resume_u64`）端到端 fixture。
   - `tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.stdout`：stdout golden（输出与单线程恢复语义一致）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok，含该用例）。

### T0621 [DONE] generator/yield：库级实现验证（spec §5.7）
- 描述：基于 continuation 或 effect，提供最小 `yield`/迭代器 demo（无需专用语法）。
- 目标：先只作为库/fixtures 验证，不强依赖语法。
- 验收：新增 run-pass fixture：生成器 yield 多次并消费；输出正确。
- 依赖：T0617
 - 完成：
   - `tests/fixtures/run-pass/generator_yield_iter_int_basic.scoop`：用 effect + escape continuation（`, k ->`）提供最小 yield demo；通过“嵌套 handle + arm 内立即 `k.resume(...)`”串联多次 yield，并在 stdout 中可观测。
   - `tests/fixtures/run-pass/generator_yield_iter_int_basic.stdout`：stdout golden（按行精确比对）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok，含该用例）。

### T0916 [DONE] effect runtime：多层 handler stack 嵌套 dispatch（修正 T0913 的单层目标，Appendix A）
- 描述：在已有 handler stack 原语之上补齐多层嵌套 handler：按“最近匹配 handler”分发，并保证 arm body 在自身 handler 的 dispatch scope 外执行。
- 目标：保持与 T0913 兼容，不修改既有任务；本任务专门补齐“多层嵌套”能力。
- 验收：新增 run-pass fixture：三层嵌套 handler 下最近匹配规则成立；arm 内 re-perform 命中外层 handler。
- 依赖：T0913
 - 完成：
   - `tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`：新增三层嵌套 try/catch（Raise.raise）回归用例。
   - `tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.stdout`：stdout golden（按行精确比对）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- dump-hir tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop` 通过；
   - `cargo run -p scoop -- test` 通过；
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 可执行该用例（需要安装 LLVM 并提供 `llvm-config`）。

### T0625 [DONE] Appendix A 一致性：嵌套 handler 的语义契约与 lowering 校验
- 描述：在 lowering/semantics 层明确并验证：嵌套 `handle` 必须遵循”最近匹配 handler”分发，且 handler arm body 在其自身 dispatch scope 外执行。
- 目标：先只覆盖 `Raise` 与最小自定义 effect；实际 runtime 支持由 T0916 补齐。
- 验收：effects + run-pass fixture：嵌套 handler 的最近匹配规则成立；arm 内 re-perform 不会自捕获。
- 依赖：T0615、T0916
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：为最小自定义 non-resuming effect 补齐 `perform/handle` codegen：
     - `perform`：写 slot（1 word payload）+ set flag，并跳转到最近匹配的 handler boundary；
     - `handle`：catch 读取 slot 并清 flag/slot；arm body 在自身 dispatch scope 外执行（避免 self-capture），arm 内 re-perform 先经 `finally_unwind` 再向外传播。
   - `tests/fixtures/run-pass/effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.*`：新增端到端 run-pass fixture（自定义 non-resuming effect 的三层嵌套 + arm re-perform）与 stdout golden。
   - `tests/fixtures/run-pass/effect_handler_stack_nearest_three_levels_and_arm_outside_scope.stdout`：移除尾随空行，避免与实际 stdout 的严格比对产生误报。
 - 验收：
   - `cargo test --all` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过；
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test` 通过（fixtures: ok，包含新用例）。

### T0917 [DONE] runtime：`Task<T>` / executor 最小原语（spec §5.7）
- 描述：提供支撑 `Task<T>` / `async` / `spawn` 的最小 runtime 原语：任务状态、入队/恢复、完成回调、可选显式 start。
- 目标：先只实现 cooperative、最小可观测版本；取消与复杂调度后续。
 - 验收：新增运行期测试：创建 task、入队、完成后恢复 continuation；状态转换与回调顺序稳定。
 - 依赖：T0906、T0914、T0622
 - 完成：
   - `runtime/c/scoop_task_executor.c`：新增最小 `ScoopExecutor` 队列 + `ScoopTaskU64` 状态机：
     - continuation 入队/恢复（u64 payload），并用 `scoop_pin/unpin` 保证 GC 期间不被 sweep；
     - task waiters：完成后按注册顺序把 continuation 入队到指定 executor；
     - 显式 `try_start`：把 task body 入队到 executor 运行并完成。
   - `crates/scoop_runtime/build.rs`：把 `scoop_task_executor.c` 纳入 C runtime 编译列表。
   - `crates/scoop_runtime/tests/task_executor_minimal.rs`：新增集成测试，回归 task start→complete→按序恢复 waiters 的顺序与状态转换。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoop_runtime --test task_executor_minimal` 通过。

### T0918 [DONE] runtime：`object` / `companion object` 的 once 初始化原语（Appendix B.9）
- 描述：若 codegen 采用 runtime 辅助初始化，则提供 once/guard 原语以支持 `object` / `companion object` 的一次初始化。
- 目标：先只支持单进程内初始化一次；跨 DLL / 动态链接细节后续。
- 验收：新增 run-pass fixture：多次并发前的重复访问不会重复初始化；初始化副作用只出现一次。
- 依赖：T0901
 - 完成：
   - `runtime/c/scoop_once.c`：新增 once/guard 原语 `scoop_once_begin/scoop_once_end`（uint64 guard + 原子操作，支持同线程重入与跨线程等待）。
   - `crates/scoop_runtime/build.rs`：把 `scoop_once.c` 纳入 C runtime 编译列表。
   - `crates/scoop_runtime/tests/once_guard.rs`：新增多线程回归测试，保证 init 最多执行一次且重入不死锁。
   - `crates/scoopc/src/llvm/codegen.rs`：object init guard 升级为 `uint64_t` word，并接入 runtime once API（避免并发访问下的 data race）。
   - `tests/fixtures/run-pass/object_once_init_cross_thread.*`：新增跨线程访问 object 的 run-pass fixture，验证 `init` 副作用只出现一次。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（未启用 `scoop` 的 `llvm` feature 时 run-pass pass fixtures 会被跳过执行，但仍会被发现与计数）。

### T0919 [DONE] runtime：`object` / `companion object` 的跨 DLL / 动态链接一次初始化
- 描述：补齐单例 once 初始化在动态链接场景下的语义：同一逻辑单例跨动态库加载边界的身份、初始化竞争、以及导出/导入侧的可见性策略。
- 目标：先覆盖主机平台支持的动态链接模型；静态链接路径保持与 T0918 一致。
- 验收：新增运行期测试或平台 fixture：跨动态库重复访问不会导致重复初始化，且可观测副作用只发生一次。
- 依赖：T0918
 - 完成：
   - `runtime/c/scoop_once.c`：新增 `scoop_once_guard_canonicalize(symbol_name, fallback)`：通过 `dlsym(RTLD_DEFAULT, symbol_name)` 选取进程内 canonical guard 地址，使多个动态库对同一 guard word 做原子状态机操作；并在注释中记录 macOS 的 symbol name 与 `RTLD_GLOBAL` 约束。
   - `crates/scoop_runtime/build.rs`：Linux 下补齐 `cargo:rustc-link-lib=dl`（dlsym/dlerror 依赖）。
   - `crates/scoop_runtime/tests/once_guard_cross_dylib.rs`：新增集成测试：运行时用 `clang` 生成两个 dylib（同名 guard），先访问 A 再加载 B，断言 init 只发生一次且 canonical guard 不随新 dylib 加载漂移。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test -p scoop_runtime --test once_guard_cross_dylib` 通过。

### T0920 [DONE] runtime/codegen：type descriptor 增加 release callback（非通用 finalizer）（spec §15.11）
- 描述：在类型描述（type descriptor）中增加一个可选的 release callback 函数指针，并在对象被 GC 回收（sweep/free）时调用该回调，主要用于 FFI-managed 资源释放（ManagedArena 等）。
- 目标：
  - 明确它不是通用 finalizer：不保证顺序，不允许对象复活，不依赖其他对象存活；
  - 只用于 compiler 自动合成/内部类型（0.1 不对用户开放“自定义 finalizer”语法）；
  - callback 视为 `@NoGC`/`@Unsafe` 语义的受限环境（实现上应尽量避免分配与 re-enter GC）。
- 验收：
  - runtime 集成测试：构造一个带 release callback 的对象类型（可用测试专用 type desc），回收时回调被调用且仅一次；
  - run-pass fixture（可选）：创建对象并触发一次 `GC.collect`（或 debug collect），验证回调 side effect 可观测。
- 依赖：T0907、T0910
 - 完成：
   - `runtime/c/scoop_gc.h`：`ScoopTypeDescriptor` 增加可选 `release_fn` 与 `ScoopTypeReleaseFn`，并补齐“非通用 finalizer”的语义注释。
   - `runtime/c/scoop_gc.c`：GC sweep 回收对象时，在 `free` 前调用 `release_fn`（若存在）。
   - `runtime/c/scoop_runtime.c`：continuation type desc 显式初始化 `release_fn = NULL`。
   - `crates/scoop_runtime/tests/type_descriptor.rs`：同步 `ScoopTypeDescriptor` 的 Rust 对齐结构体布局。
   - `crates/scoop_runtime/tests/type_descriptor_release_callback.rs`：新增集成测试：对象被 roots 保活时不调用；变为 unreachable 后回收时仅调用一次。
 - 验收：
   - `cargo test -p scoop_runtime` 通过；
   - `cargo test --all` 通过。

---

## T10：注解系统与系统编程通道（阶段 9）

### T1001 [DONE] Parser：注解使用 `@Name(...)` 的 AST 与解析（spec §15.3）
- 描述：允许在声明前出现一个或多个注解，并把它们记录到 AST 节点上。
- 目标：先只支持无参/仅字面量参数；不解析复杂表达式参数。
- 验收：新增 parse fixture：`@Unsafe fun f() {}` 可解析；`@Extern("c_name") fun g()` 可解析（若支持字符串参数）。
- 依赖：T0218
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `AnnotationUse`/`AnnotationArg`，并为主要声明节点增加 `annotations` 字段（空时不影响既有 AST snapshot）。
   - `crates/scoopc/src/parser/cursor.rs`：lookahead 支持跳过注解前缀，避免顶层与 type body 分流误判。
   - `crates/scoopc/src/parser/decls.rs`：实现 `@Name(...)` 解析（含可选 use-site target 与字面量参数），并写入各类声明的 `annotations`。
   - `tests/fixtures/parse/annotation_use_fun_basic.*`：新增 fixtures 覆盖 `@Unsafe` 与 `@Extern("c_name")`。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T1002 [DONE] Parser：注解声明 `annotation class X(...)`（spec §15.2）
- 描述：支持声明注解类型，并在 type env 中识别其为“注解类”。
- 目标：先只支持 data-only（无方法体）；target/retention 规则后续。
- 验收：新增 parse+typecheck fixture：定义注解并使用；错误用法给出诊断。
- 依赖：T1001、T0404
 - 完成：
   - `crates/scoopc/src/typecheck/type_env.rs`：`TypeSymbol` 新增 `is_annotation_class`，在 env 构建时识别 `annotation class`。
   - `crates/scoopc/src/typecheck/annotations.rs`：新增 `check_file_annotations`，实现：
     - 注解类 data-only 形态约束（必须是 `class`、不支持 supertypes/type body、参数必须是 `val`）；
     - 注解使用 `@Name(...)` 的最小验证：`Name` 必须解析到注解类类型符号。
   - `crates/scoop/src/commands/build.rs`、`crates/scoop/src/fixtures/mod.rs`、`crates/scoopc/src/monomorph/lower.rs`：在 typecheck pipeline 中接入注解检查（确保 `scoop build`/fixtures/monomorph 一致）。
   - `tests/fixtures/typecheck/annotation_*`：新增 1 个 pass + 2 个 fail fixtures 覆盖“定义并使用 / 非注解类用作注解 / 注解类参数非 val”。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过。

### T1003 [DONE] Typecheck：内建注解 `@Unsafe/@NoGC/@Extern/@Intrinsic` 的合法性检查（PLAN §11）
- 描述：实现基础规则：`@Extern` 隐含 `@NoGC`；调用点需要 unsafe context（按 PLAN 建议）。
- 目标：先只做静态检查，不生成任何 codegen 行为。
- 验收：新增 `tests/fixtures/unsafe_nogc/*`：违规路径 compile-fail；合法路径 pass。
- 依赖：T0101、T1001、T0404
 - 完成：
   - `crates/scoopc/src/typecheck/builtin_annotations.rs`：新增内建注解识别与标记位提取（不依赖 `annotation class`）。
   - `crates/scoopc/src/typecheck/annotations.rs`：
     - 允许 `@Unsafe/@NoGC/@Extern/@Intrinsic` 作为内建注解通过解析；
     - 增加最小合法性检查：target 限制、`@Extern/@Intrinsic` 必须省略函数体、`@Extern` 仅允许 0/1 个字符串字面量参数。
   - `crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/expr.rs`：
     - 引入 unsafe context 深度；
     - 调用点门禁：非 unsafe context 禁止调用 `@Extern/@Unsafe` 函数（当前阶段仅函数级 unsafe）。
   - `crates/scoop/src/fixtures/mod.rs`：将 `tests/fixtures/unsafe_nogc/**` 路由到 typecheck phase。
   - `tests/fixtures/unsafe_nogc/*`：新增 pass/fail fixtures 覆盖 `@Extern` 调用门禁与 extern body 约束。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T1004 [DONE] Typecheck：`@Unsafe { ... }` 块语法与上下文传播（spec §15.9.2）
- 描述：在表达式/语句层支持 unsafe block，并让检查器在该区域放宽限制。
- 目标：先只做上下文标记；不实现指针 API。
- 验收：unsafe_nogc fixture：在 unsafe block 内允许调用 `@Extern`，块外禁止。
- 依赖：T1003、T0207
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `ExprKind::UnsafeBlock`（`@Unsafe { ... }`）。
   - `crates/scoopc/src/parser/expr.rs`：解析 `@Unsafe { ... }` 为 `UnsafeBlock` 表达式。
   - `crates/scoopc/src/resolve/scopes.rs`：resolver 递归进入 unsafe block body。
   - `crates/scoopc/src/typecheck/expr.rs`：进入/退出 unsafe block 时 push/pop unsafe depth，使 block 内允许调用 `@Extern/@Unsafe`。
   - `crates/scoopc/src/hir/lower.rs`、`crates/scoopc/src/typecheck/properties.rs`：补齐 `ExprKind` 分支以保持全 crate 构建通过。
   - `crates/scoopc/src/parser/tests.rs`：新增 parser 单测 `parse_unsafe_block_expr`。
   - `tests/fixtures/unsafe_nogc/*`：新增 1 个 pass + 1 个 fail fixture 覆盖“block 内允许 / block 外仍禁止”。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T1005 [DONE] `@NoGC`：调用限制与“可能分配”静态判定（spec §15.8）
- 描述：实现 `@NoGC`：禁止堆分配、禁止调用非 `@NoGC/@Extern`；当编译器无法证明无分配时必须保守报错。
- 目标：先只做基于“已知分配点”的保守分析；不做全程序逃逸分析。
- 验收：unsafe_nogc fixture：在 `@NoGC` 函数里调用 `scoop_alloc`（或构造 box）报错；调用纯函数通过。
- 依赖：T1003、T0817
 - 完成：
   - `crates/scoopc/src/typecheck/lower.rs`：引入 `@NoGC` 上下文深度（push/pop/suspend），并在 lambda body 中默认抑制该上下文。
   - `crates/scoopc/src/typecheck/expr.rs`：
     - 新增 `@NoGC` 调用门禁：仅允许调用 `@NoGC/@Extern` 函数；
     - 新增“已知分配点”门禁：值类型（或 type param 占位）在赋值到引用类型时视为 boxing（可能堆分配），在 `@NoGC` 中报错；
     - 修复表达式语句位置的调用可绕过问题：`@NoGC` 下的 `call();` 也会强制走调用类型检查路径。
   - `tests/fixtures/unsafe_nogc/*`：新增 `@NoGC` 的 pass/fail fixtures（禁止调用非 NoGC、禁止 boxing）。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T1006 [DONE] `@Extern`：FFI 符号名与 ABI 约定（spec §15.8.3）
- 描述：为 `@Extern` 函数定义名称映射（如 `@Extern("puts")`）与最小 ABI（C ABI）。
- 目标：先只支持简单参数/返回类型（Int/ptr）；结构体传递后续。
- 验收：新增 run-pass fixture：调用 `@Extern("puts")` 打印字符串（或调用自带 runtime 打印 API）；输出正确。
- 依赖：T1001、T0810、T0106b2
 - 完成：
   - `crates/scoopc/src/hir/mod.rs`：新增 `ExternFun/ExternAbi/ExternFunIndex` side table（不影响 dump-hir 输出稳定性）。
   - `crates/scoopc/src/hir/lower.rs`：lowering 阶段提取 `@Extern("symbol")` 的符号名与 ABI，写入 `LoweredHir.extern_funs`。
   - `crates/scoopc/src/llvm/codegen.rs`：
     - `declare_top_level_fun`：对 `@Extern` 以 `symbol` 作为 LLVM function name 声明，并设置 C ABI（callconv 0）；
     - `codegen_top_level_fun_call`：对 extern callee 使用 symbol name 查找/声明；补齐 ptr-return（`String`/`Ref`）的 call-site 返回值处理。
   - `crates/scoopc/src/syntax/string_literal.rs`：抽出最小字符串字面量解码（供 codegen 与 `@Extern` 提取共享）。
   - `tests/fixtures/run-pass/extern_symbol_println_basic.*`：新增 run-pass 用例，通过 `@Extern("scoop_println")` 直接调用 runtime 打印，回归符号名映射与 C ABI。
   - `tests/fixtures/run-pass/object_once_init_cross_thread.stdout`：修复 golden 文件尾部多余空行，保证 `scoop test --features llvm` 下 run-pass 输出一致。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test --all --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T1007 [DONE] `@Intrinsic`：sysroot 声明与编译器 lowering（spec §15.7）
- 描述：在 sysroot 中声明 intrinsic，并在 lowering/codegen 阶段把它们替换为内建操作（例如算术、类型反射）。
- 目标：先只实现 1~2 个 intrinsic（例如 `sizeOf<T>()`/`panic()`）。
- 验收：新增 comptime/typecheck fixture：调用 intrinsic 通过；codegen 侧不产生真正函数调用。
- 依赖：T0418
 - 完成：
   - `sysroot/core.scoop`：新增 `@Intrinsic fun sizeOf(...)` 声明（以 overload 形式暴露 Int/String 的最小可调用表面）。
   - `crates/scoopc/src/llvm/mod.rs`：基于 module data layout 创建 `TargetData` 并注入 `MainCodegen`。
   - `crates/scoopc/src/llvm/codegen.rs`：
     - `codegen_call`：识别 `scoop.core.sizeOf` 并走内建 lowering；
     - `codegen_sysroot_size_of`：把 `sizeOf(x)` 降为编译期常量（LLVM TargetData store size），不生成对 `scoop.core.sizeOf` 的真实函数调用。
   - `tests/fixtures/codegen/intrinsic_size_of_int_word.*`：新增 run-pass fixture，回归 `println(sizeOf(1))` 的输出与链接可用性。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo test --all --features llvm` 通过；
   - `cargo run -p scoop --features llvm -- test` 通过（fixtures: ok）。

### T1008 [DONE] pin/unpin 语言层 API：从 sysroot 暴露到 runtime（spec §15.10）
- 描述：在 sysroot 增加 `pin/unpin` 声明，并在 codegen 中 lower 到 runtime 的 `scoop_pin/scoop_unpin`。
- 目标：先只支持对引用类型/box 对象；value types 不允许 pin。
- 验收：新增 run-pass fixture：pin 后在 GC collect 过程中对象不移动（非移动 GC 下可用“仍可访问”替代验证）；unpin 后仍可访问。
- 依赖：T0912、T0817
 - 完成：
   - `sysroot/core.scoop`：新增 `@Intrinsic object GC`，暴露 `@NoGC @Unsafe fun pin(obj: Any): Pinned` 与 `@NoGC @Unsafe fun unpin(pinned: Pinned): Unit`；新增 `struct Pinned(val value: Any)`。
   - `crates/scoopc/src/typecheck/expr.rs`：对 `scoop.core.GC.pin/unpin` 做门禁与类型检查（unsafe context + pin 仅允许 ref type + unpin 参数必须是 `Pinned`），并补齐错误码 `scoop::typecheck::gc_pin_requires_ref` / `scoop::typecheck::gc_unpin_requires_ref`；同时为 `Pinned.value` 提供 sysroot 字段类型 fallback。
   - `crates/scoopc/src/llvm/codegen.rs`：将 `scoop.core.GC.pin/unpin` lowering 到 runtime `scoop_pin/scoop_unpin`，并生成 `Pinned` 结构体返回值/参数解构；补齐 `scoop.core.Any` 的 codegen 类型映射。
  - 约束：`Pinned` 在 Scoop 0.1 中为非泛型 handle（内部字段 `value: Any`），以保持 runtime ABI 简单可审计。
   - `tests/fixtures/unsafe_nogc/gc_pin_value_type_is_error.scoop`：新增 compile-fail，用于断言对 value type 的 `GC.pin(1)` 报错。
   - `tests/fixtures/run-pass/gc_pin_unpin_basic.*`：新增 run-pass，用于端到端回归 pin/unpin 的保活语义与 stdout。
 - 验收：
   - `cargo test --all`
   - `cargo test --all --features llvm`
   - `cargo run -p scoop --features llvm -- test`（fixtures: ok）

### T1009 [DONE] `@Unsafe`：最小 unsafe 原语（`Ptr<T>`/内存读写/地址转换）的语法与门禁
- 描述：引入最小 unsafe 原语（例如 `Ptr<T>`、`load/store`、`addrOf`、指针↔整数转换），并确保只能在 unsafe context 使用。
- 目标：先只提供极小集合以支撑 runtime/FFI；完整系统编程能力后续逐步补齐。
- 验收：unsafe_nogc fixture：在非 unsafe context 使用 ptr 操作报错；unsafe block 内通过。
- 依赖：T1004
 - 完成：
   - `crates/scoopc/src/typecheck/expr.rs`：为未解析的调用点引入三个位于语言层的 unsafe 指针原语：
     - `addrOf(x)` → `Ptr<T>`（T 为 `x` 的类型）；
     - `load(p)`：要求 `p: Ptr<T>`，返回 `T`；
     - `store(p, v)`：要求 `p: Ptr<T>` 且 `v` 可赋值给 `T`，返回 `Unit`；
     同时增加门禁：非 unsafe context 直接报错（新错误码 `scoop::typecheck::unsafe_ptr_primitive_requires_unsafe`）。
     约定：优先识别未来 sysroot 的 `scoop.unsafe.Ptr`；若尚不存在，则允许 fixtures 在当前包内声明 `struct Ptr<T>` 作为最小落点。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_primitives_require_unsafe_is_error.scoop`：新增 compile-fail，回归“非 unsafe context 使用 ptr 原语报错”。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_primitives_allowed_in_unsafe_block_ok.scoop`：新增 compile-pass，回归“unsafe block 内可用 addrOf/load/store”。
 - 验收：
   - `cargo test --all` 通过；
   - `cargo run -p scoop -- test` 通过（fixtures: ok）。

### T1010 [DONE] sysroot：新增 `scoop.unsafe` 模块声明（`Ptr<T>` + 指针/整数转换 intrinsics）（spec §15.9.4）
- 描述：在 sysroot 增加专门的 unsafe 模块（建议 `package scoop.unsafe`），声明：
  - `@Intrinsic struct Ptr<T>`
  - `@Intrinsic @NoGC @Unsafe fun <T> ptrToUIntPtr(p: Ptr<T>): UIntPtr`
  - `@Intrinsic @NoGC @Unsafe fun <T> uintPtrToPtr(addr: UIntPtr): Ptr<T>`
- 目标：先只做“可见声明”；intrinsic 的具体 lowering 留给后续 codegen；模块命名与路径固定以便审计。
- 验收：新增 resolve fixture：`import scoop.unsafe.*` 后能引用 `Ptr<Int>`、`ptrToUIntPtr`；`scoop test` 通过。
- 依赖：T0418、T1001
 - 完成：
   - `sysroot/unsafe.scoop`：新增 `package scoop.unsafe`，声明 `@Intrinsic struct Ptr<T>` 与 `ptrToUIntPtr/uintPtrToPtr` 两个指针↔整数转换 intrinsics。
   - `tests/fixtures/resolve/sysroot_unsafe_ptr_import_ok.scoop`：新增 resolve fixture，覆盖 `import scoop.unsafe.*` 后引用 `Ptr<Int>` 与 `ptrToUIntPtr`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1011 [DONE] Typecheck：`Ptr<T>` 的 GC-free pointee 限制（spec §15.9.4 / runtime §4.1）
- 描述：实现 `Ptr<T>` 的 well-formedness：`T` 必须是 GC-free value type（不允许直接/间接包含 GC ref），并在违反时给出清晰诊断。
- 目标：先做保守检查（宁可拒绝也不放过）；对 `Option<RefType>` 这类也应拒绝（因为表示里含 GC pointer）。
- 验收：unsafe_nogc/typecheck fixture：`Ptr<Int>` 通过；`Ptr<String>`、`Ptr<Option<String>>` 报错（新错误码）并指向 `Ptr<...>` 的类型参数位置。
- 依赖：T0402、T0403、T1003
 - 完成：
   - `crates/scoopc/src/typecheck/lower.rs`：在 nominal type instantiation 时为 `scoop.unsafe.Ptr<T>` 增加 well-formedness 校验：pointee 必须是 GC-free 值类型；新增错误码 `scoop::typecheck::ptr_pointee_must_be_gc_free`，并尽量把 span 指向 `Ptr<...>` 的类型实参位置。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_pointee_gc_free_int_ok.scoop`：新增 compile-pass：`Ptr<Int>` 合法。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_pointee_not_gc_free_string_is_error.scoop`：新增 compile-fail：`Ptr<String>` 报错并断言错误码/位置。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_pointee_not_gc_free_option_of_string_is_error.scoop`：新增 compile-fail：`Ptr<Option<String>>` 报错并断言错误码/位置（为避免 lexer 将 `>>` 视为 shift token，使用 `Ptr<Option<String> >` 的写法）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1012 [DONE] Typecheck：指针↔整数转换只能通过 sysroot intrinsics，且必须在 unsafe context（spec §15.9.4 / runtime §5）
- 描述：把“pointer/int casts 的限制点”固定为：仅允许调用 sysroot 提供的转换 intrinsics（例如 `ptrToUIntPtr/uintPtrToPtr`），并要求调用点处于 unsafe context；明确 **不** 把 `as/as?` 当作指针转换。
- 目标：先只做静态门禁与错误信息；不做 codegen（lowering 到 LLVM 留给后续任务）。
- 验收：unsafe_nogc fixture：在非 unsafe context 调用 `ptrToUIntPtr` 报错；在 `@Unsafe { ... }` 内通过；`p as UIntPtr` 不被当作合法指针转换（按普通 cast 规则处理并产生对应诊断/required effects）。
- 依赖：T1010、T1004、T0412
 - 完成：
   - `crates/scoopc/src/resolve/mod.rs`：Index 的 `FunSig` 记录函数 type params（名字+span）与内建注解 flags（`@Unsafe/@NoGC/@Extern/@Intrinsic`），供跨文件调用点查询。
   - `crates/scoopc/src/typecheck/expr.rs`：`collect_top_level_fun_signatures_from_index` 支持从 Index lowering “单一 type param”的泛型函数签名，并把 builtin flags 写入 `FunSigOwned`，从而对 sysroot `ptrToUIntPtr` 施加 `@Unsafe` 调用门禁。
   - `crates/scoopc/src/typecheck/lower.rs`：允许 sysroot 声明处的 type param 作为 `Ptr<T>` pointee 出现在 intrinsic 签名中（不放宽用户代码的保守策略）。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_to_uintptr_requires_unsafe_is_error.scoop`：新增 compile-fail，safe context 调用 `ptrToUIntPtr` 报错（错误码 `unsafe_call_requires_unsafe`）。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_to_uintptr_allowed_in_unsafe_block_ok.scoop`：新增 compile-pass，`@Unsafe { ... }` 内调用通过。
   - `tests/fixtures/unsafe_nogc/unsafe_ptr_as_uintptr_is_invalid_cast.scoop`：新增 compile-fail，`p as UIntPtr` 仍按普通 cast 规则报 `invalid_cast`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1013 [DONE] 注解系统：补齐内建注解与 `AnnotationTarget`（spec §15.5）
- 描述：在 sysroot/typecheck 中补齐内建注解集合：`@TailRec`、`@AllowIntrinsic`、`@Suppress`、`@CLayout`、`@Target`、`@Retention`，并引入 `AnnotationTarget` enum。
- 目标：先固定声明面与最小合法性检查；复杂行为（如真正 TCO）后续由各子系统消费。
- 验收：新增 parse/typecheck fixture：这些注解可被声明/使用；非法 target 名报错。
- 依赖：T1002、T0418
 - 完成：
   - `sysroot/core.scoop`：新增 `AnnotationTarget` enum，并补齐内建注解声明面：`TailRec/AllowIntrinsic/Suppress/CLayout/Target/Retention`。
   - `crates/scoopc/src/parser/decls.rs`：注解参数值解析扩展为支持 `Ident(.Ident)*` 形式（用于 `@Target(AnnotationTarget.X, ...)`）。
   - `crates/scoopc/src/typecheck/annotations.rs`：为 `@Target(...)` 增加最小合法性检查：非法 `AnnotationTarget` variant 名报错（错误码 `scoop::typecheck::invalid_annotation_target_name`）。
   - `crates/scoopc/src/parser/tests.rs`：新增 parser 单测覆盖 `@Target(AnnotationTarget.Field)` 解析为 member access。
   - `tests/fixtures/parse/builtin_annotation_target_basic.scoop` + `.ast`：新增 parse fixture 覆盖 enum value 形态的注解参数解析。
   - `tests/fixtures/typecheck/builtin_annotations_annotation_target_ok.scoop`：新增 typecheck fixture 覆盖内建注解声明/使用与合法 `@Target` 参数。
   - `tests/fixtures/typecheck/annotation_target_invalid_name_is_error.scoop`：新增 typecheck fixture 覆盖非法 target 名诊断。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1014 [DONE] 注解 use-site targets：`field:/property:/param:/get:/set:/file:`（spec §15.3）
- 描述：支持 use-site target 前缀语法，并在注解附着时区分实际目标元素。
- 目标：先只覆盖 property / param / field / file；getter/setter 的细化可在同任务内保留占位实现。
- 验收：新增 parse/typecheck fixture：`@property:Rename`、`@param:Validated`、`@file:AllowIntrinsic` 可解析并附着到正确目标。
- 依赖：T1001、T1013
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：为 `File` 增加 `file_annotations`（仅当非空时参与 Debug 输出以保持既有 AST golden 稳定）；为 `Param` 增加 `annotations`（支持构造参数/函数参数等位置的注解挂载）。
   - `crates/scoopc/src/parser/file.rs`：支持在 `package/import` 之前解析文件级注解 `@file:...` 并写入 `File.file_annotations`（只消费显式 `@file:` 前缀，避免误吞普通声明注解）。
   - `crates/scoopc/src/parser/decls.rs`：参数列表解析支持前缀注解（函数参数与主构造参数），并开放 `parse_annotation_use` 供文件解析复用。
   - `crates/scoopc/src/typecheck/annotations.rs`：遍历并检查文件级注解与参数注解（含函数参数、主构造参数、二级构造参数与成员函数参数），并拒绝把内建注解用在 `file/param` 目标上。
   - `tests/fixtures/parse/annotation_use_site_targets_basic.*`：新增 parse fixture + AST golden 覆盖 `@file:` 与参数 use-site targets。
   - `tests/fixtures/typecheck/annotation_use_site_targets_basic_ok.scoop`：新增 typecheck fixture 覆盖 file/param/property/field use-site target 在 typecheck 中可被解析。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1015 [DONE] namespaced annotations：`@Namespace.Annotation(...)`（spec §15.4）
- 描述：支持命名空间注解的解析与绑定：例如 `@Serialization.Rename("x")`。
- 目标：先只支持以 path 形式引用注解类；命名空间对象本身的完整语义可与 object 任务联动。
- 验收：新增 parse+resolve fixture：namespaced annotation 可解析并绑定；未定义路径时报错。
- 依赖：T1001、T0258、T0317
 - 完成：
   - `crates/scoopc/src/resolve/mod.rs`：resolve 阶段对 `@A.B` 形式的注解名路径做最小存在性解析（复用 `resolve_type_path`），覆盖 file/type/object/fun/property/param/enum variant/ctor 等注解载体。
   - `tests/fixtures/parse/namespaced_annotation_basic.*`：新增 parse fixture + AST golden 覆盖 `@Serialization.Rename("...")` 解析。
   - `tests/fixtures/resolve/namespaced_annotation_ok.scoop`：新增 resolve fixture（pass）。
   - `tests/fixtures/resolve/namespaced_annotation_unresolved_is_error.scoop`：新增 resolve fixture（fail），断言错误码与位置。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1016 meta-annotations（拆分为子任务，spec §15.5）
- 描述：实现 meta-annotations 的最小规则：`@Target` 限制注解可应用位置，`@Retention` 决定是否仅编译期可见或保留到 `.cone` 元数据。
- 目标：先只支持 comptime-only 与 cone-preserved 两档；更细粒度 policy 后续再补。
- 备注：为保证 TODO 顺序“首个 `[TODO]` 可直接实现”，把“typecheck 合法性”和“.cone 导出/下游可见性”拆分为两步（后者依赖 Cone 基础设施）；T1016b 已移动到 T1209 之后以满足依赖顺序。

### T1016a [DONE] meta-annotations：typecheck enforce `@Target/@Retention`（不含 `.cone`）
- 描述：在 typecheck 阶段读取注解类上的 meta-annotations，并在所有注解使用点强制执行：
  - `@Target(AnnotationTarget.X, ...)` 限制注解可出现的目标；
  - `@Target/@Retention` 自身只能用于 `annotation class` 声明；
  - `@Retention(policy)` 仅接受 `"comptime"` / `"cone"` 两档（先不做导出行为）。
- 目标：只做“语义检查 + 稳定诊断”；不做 `.cone` 导出；不做注解参数的完整类型检查。
- 验收：新增 typecheck fixtures：
  - 被 `@Target` 禁止的位置报错（含 `@param:` override 的 case）；
  - `@Target`/`@Retention` 用在非注解类上报错；
  - `@Retention` policy 非法时报错；
  - `cargo run -p scoop -- test`（fixtures: ok）
- 依赖：T1013
 - 完成：
   - `crates/scoopc/src/typecheck/type_env.rs`：在 `TypeSymbol` 中缓存注解类的 meta 信息（`annotation_targets` / `annotation_retention`），并在构建 type env 时从注解类声明头提取 `@Target/@Retention`。
   - `crates/scoopc/src/typecheck/annotations.rs`：在所有注解使用点强制执行 `@Target` 目标限制；限制 `@Target/@Retention` 只能用于 `annotation class`；对 `@Retention("comptime"|"cone")` 做最小 policy 合法性检查，并提供稳定错误码。
   - `tests/fixtures/typecheck/*`：新增 4 个 fixtures 覆盖：
     - `@Target` 禁止目标（fun/`@param:` override）；
     - `@Target` 用在非注解类上报错；
     - `@Retention` policy 非法时报错。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1019 [DONE] 注解参数：支持常量表达式 / 数组 / enum / class-literal 参数
- 描述：把注解参数从“仅无参或字面量”扩展到更完整的常量参数模型：常量表达式、数组字面量、enum 值、类字面量等，并在 typecheck 中验证参数类型与可求值性。
- 目标：先只接受编译期可确定的参数；不允许运行期依赖值混入注解参数。
- 验收：新增 parse/typecheck fixtures：`@Anno(1 + 2)`、`@Anno([A, B])`、`@Anno(Color.Red)`、`@Anno(String::class)` 可通过；非常量参数报错。
- 依赖：T1001、T1002
 - 完成：
   - `crates/scoopc/src/parser/decls.rs`：注解参数改为复用表达式 parser，并避免把 `String::class` 误判为 named arg（`String: ...`）。
   - `crates/scoopc/src/parser/expr.rs`：新增 `[...]` 数组字面量与 `TypeName::class`（class literal）解析。
   - `crates/scoopc/src/ast/mod.rs`：新增 `ExprKind::ArrayLit` / `ExprKind::ClassLit`。
   - `crates/scoopc/src/typecheck/type_env.rs`：为注解类缓存主构造参数签名（name/type/default），供注解使用点检查。
   - `crates/scoopc/src/typecheck/annotations.rs`：在注解使用点执行“参数绑定 + 常量表达式判定 + 类型匹配”，并给出稳定错误码（`annotation_arg_not_const` 等）。
   - `sysroot/core.scoop`：补齐 `Array<T>` 声明面，支持注解参数类型 `Array<...>`。
   - `tests/fixtures/parse/*`、`tests/fixtures/typecheck/*`：新增 pass + fail fixtures 覆盖四类参数与非常量报错。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1020 [DONE] `@Extern` 扩展：支持 `lib`/`name` 参数 + extern 变量声明（spec §15.5.1）
- 描述：扩展 `@Extern` 注解的语义：
  - 支持 `@Extern(lib = "...", name = "...")` 显式指定库名与符号名；
  - 允许 `@Extern` 标注全局变量（GC-free）作为外部符号声明；
  - 当 `lib` 指定时，driver 链接阶段自动加入 `-l<lib>`（先假设标准库路径，无需新增 library path 配置）。
- 目标：
  - 不改变现有 extern codegen（仍走 C ABI 调用/符号解析）；
  - 先把“link 参数收集/透传”与“extern var 的语义门禁”做出来；
  - 先不做跨平台复杂链接选项（`-L`/rpath 等后续再立项）。
- 验收：
  - 新增 typecheck fixture：`@Extern(name="x") var v: Int`（无 initializer）允许；带 initializer 报错（稳定错误码）；
  - 新增 driver 单测：收集到 `@Extern(lib="m")` 后，clang 参数包含 `-lm`（或等价形式）；
  - `cargo test --all` 通过。
- 依赖：T1006、T1019、T0806
 - 完成：
   - `crates/scoopc/src/typecheck/annotations.rs`：扩展内建 `@Extern` 参数解析（支持 `name/lib`，兼容位置参数），允许 extern 顶层变量声明，并对 initializer 给出稳定错误码。
   - `crates/scoopc/src/hir/lower.rs`：收集 `@Extern(lib = "...")` 的外部库依赖（稳定排序去重），并让 extern 函数符号名支持 `name = "..."`。
   - `crates/scoop/src/toolchain.rs`：clang 链接参数支持透传 `-l<lib>`；新增单测覆盖。
   - `tests/fixtures/typecheck/*`：新增 extern 顶层变量 pass/fail fixtures。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1021 [DONE] `@Safe`：在 unsafe context 内显式“收窄”为 safe 区域（spec §15.9.5）
- 描述：支持 `@Safe { ... }`（以及函数级 `@Safe`）语义：即使外层处于 unsafe context，也要在 `@Safe` 区域内禁止 unsafe primitives / 调用 `@Extern` / 调用 `@Unsafe` 函数。
- 目标：
  - 只做 typecheck 层面的上下文传播与门禁；
  - `@Safe` 内仍允许嵌套 `@Unsafe { ... }` 重新开启 unsafe（局部化）。
- 验收：
  - 新增 unsafe_nogc fixtures：
    - `@Unsafe fun f(){ @Safe { addrOf(x) } }` 报错；
    - `@Unsafe fun f(){ @Safe { @Unsafe { addrOf(x) } } }` 通过；
  - `cargo run -p scoop -- test` 通过。
- 依赖：T1004、T1009
 - 完成：
   - `crates/scoopc/src/parser/expr.rs`：解析 `@Safe { ... }` 为专用表达式节点（safe block）。
   - `crates/scoopc/src/ast/mod.rs`：新增 `ExprKind::SafeBlock`。
   - `crates/scoopc/src/typecheck/lower.rs`：新增 `with_unsafe_context_suspended`，用于 `@Safe` 临时抑制 unsafe context。
   - `crates/scoopc/src/typecheck/builtin_annotations.rs`：把 `@Safe` 纳入内建注解识别与 flags。
   - `crates/scoopc/src/typecheck/expr.rs`：在 safe block / `@Safe` 函数体内禁止 unsafe primitives 与 `@Extern/@Unsafe` 调用，并允许嵌套 `@Unsafe` 重新开启。
   - `tests/fixtures/unsafe_nogc/*`：新增 2 个 fixtures 覆盖 `@Safe` 收窄规则。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1022 [DONE] 全局可变 GC-free 变量门禁：`@ThreadLocal` / `@Global`（spec §15.5.3）
- 描述：新增全局 `var` 的显式标注规则：
  - 顶层 `var`（module/file scope）必须带 `@ThreadLocal` 或 `@Global`；
  - 仅允许 GC-free 类型（标量/GC-free struct/GC-free enum 等；实现可先保守）。
- 目标：先只做 typecheck 门禁与稳定诊断；不要求立刻支持 codegen/运行期访问。
- 验收：
  - 新增 typecheck fixtures：
    - 顶层 `var x: Int = 0`（无注解）报错；
    - `@ThreadLocal var x: Int = 0` 通过；
    - `@Global var y: String = "x"` 报错（非 GC-free）；
  - `cargo run -p scoop -- test` 通过。
- 依赖：T0404、T1019
 - 完成：
   - `sysroot/core.scoop`：补齐 `@ThreadLocal/@Global` 的注解声明面（供解析/绑定）。
   - `crates/scoopc/src/typecheck/annotations.rs`：对顶层 `var` 强制 `@ThreadLocal/@Global`，并在非 GC-free 类型时报稳定错误码。
   - `tests/fixtures/typecheck/*`：新增 3 个 fixtures 覆盖 missing/ok/non-gc-free 三种场景。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`（fixtures: ok）

### T1023 [DONE] `@ThreadLocal` / `@Global`：codegen + runtime 存储与初始化（spec §15.5.3）
- 描述：为 GC-free 全局可变变量生成：
  - TLS（`@ThreadLocal`）或进程全局（`@Global`）的静态存储；
  - 最小可用的初始化策略（早期可限制为编译期常量 initializer）。
- 目标：
  - 先只支持 GC-free 标量与 GC-free `@CLayout` struct；
  - 不引入 GC root（全局变量本身是 GC-free，不需要扫描）；
  - 并发语义（数据竞争）不在编译期自动解决，仅保证 ABI/访问可用。
- 验收：新增 run-pass fixture：在两个函数中读写 `@ThreadLocal` 计数器并断言输出/exit code；`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：T1022
 - 完成：
   - `crates/scoopc/src/hir/lower.rs`：收集顶层 `@ThreadLocal/@Global var` 并写入 `LoweredHir::top_level_vars` side table。
   - `crates/scoopc/src/llvm/codegen.rs`：为顶层 `var` 生成 TLS/全局静态存储，并支持在函数体内对其 load/store（含常量初始化与默认全零初始化）。
   - `tests/fixtures/run-pass/top_level_var_threadlocal_global_counter_basic.scoop`：新增 run-pass fixture 回归跨函数读写。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1024 [DONE] `@CLayout(aligned, packed)`：GC-free struct 的 C ABI 布局控制（spec §15.5.2）
- 描述：扩展 `@CLayout`：
  - 支持 `aligned = N` 与 `packed = M` 参数；
  - struct 必须是 GC-free；
  - codegen 侧在 LLVM struct type 上设置对齐/pack（或等价 data layout 行为），确保与目标 C ABI 一致。
- 目标：先覆盖 `packed=1` 与显式 `aligned` 的常见组合；其余复杂 ABI 细节可后置，但必须给出稳定错误/限制说明。
- 验收：新增 run-pass fixture（或 `--emit-llvm` golden）：对 `@CLayout(packed=1)` struct 的字段 offset/align 与预期一致；并新增 compile-fail fixture：`@CLayout` 用在非 GC-free struct 上报错。
- 依赖：T1022、T0811
 - 完成：
   - `sysroot/core.scoop`：为 `annotation class CLayout` 补齐 `aligned/packed` 参数（带默认值），与 spec 对齐。
   - `crates/scoopc/src/typecheck/annotations.rs`：实现 `@CLayout` 的最小语义门禁：
     - struct 必须 GC-free；
     - `packed` 仅支持 `1`；
     - `aligned` 必须是正的 2 的幂；
     - 并给出稳定错误码。
   - `crates/scoopc/src/hir/mod.rs` + `crates/scoopc/src/hir/lower.rs`：在 `StructLayout` side table 中收集 `@CLayout` 参数，供后端消费（不影响 dump-hir 输出稳定性）。
   - `crates/scoopc/src/llvm/codegen.rs`：对 `@CLayout(packed=1)` 生成 packed LLVM struct；对 `aligned` 透传 alloca/global alignment；并修正 packed struct 字段 load 的 alignment（避免 UB）。
   - `crates/scoopc/src/llvm/mod.rs`：新增 LLVM 单测回归 packed struct 的字段 offset，以及 alloca/load 对齐。
   - `tests/fixtures/typecheck/clayout_struct_must_be_gc_free_is_error.scoop`：新增 compile-fail fixture 回归 GC-free 约束。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm`

## T11：Cone（包/稳定 IR/分发）（阶段 10）

### T1101 [DONE] Cone.toml：解析 manifest（spec §13.7、PLAN §12）
- 描述：实现 `Cone.toml` 的解析（可用 toml crate），并暴露结构体给 driver。
- 目标：先只解析 package name/version/deps；不实现构建图。
- 验收：新增单测：解析最小 Cone.toml；新增 fixture：带 `Cone.toml` 的 package 目录可被发现。
- 依赖：T0002

### T1102 [DONE] 包加载：按 Cone 目录结构发现源文件（spec §13.2）
- 描述：实现 “package root → sources 列表” 的加载规则。
- 目标：先不做增量编译；不做 sysroot 之外的标准库。
- 验收：新增集成测试：构造临时目录 package，`scoop build` 能找到 main 并 parse/resolve。
- 依赖：T1101、T0805
 - 完成：
   - `crates/scoopc/src/cone/package.rs`：实现 cone root → `src/**/*.scoop` sources 发现与 `src/main.scoop` 入口定位。
   - `crates/scoop/src/commands/build.rs`：`scoop build` 支持输入包目录（包含 `Cone.toml`），并以单一编译单元执行 parse/resolve/typecheck（sysroot cone=0，当前 cone=1）。
   - `crates/scoop/src/cli.rs`：更新 `build/run` 参数说明支持“源文件或包目录”。
   - `crates/scoop/src/commands/build.rs`：新增单测覆盖“包目录 build”。
 - 验收：
   - `cargo test --all`

### T1110 [DONE] Cone.toml：平台选择器 `[[select]]` 的 manifest 解析（spec §13.9）
- 描述：在 `Cone.toml` 中新增 `[[select]]` 条目解析（v0 仅支持 platform 条件），用于按 target platform include/exclude sources。
- 目标：
  - 解析 `when = { platform = "linux-x64" }`（后续可扩展到 feature/toolchain 等条件）；
  - 解析 `include = ["..."]` / `exclude = ["..."]` 的 glob 列表；
  - 不在本任务实现 glob 语义与真正筛选，仅把结构化数据暴露给 package loader。
- 验收：新增单测：解析包含 2 个 `[[select]]` 的 Cone.toml，结构体字段正确；`cargo test --all` 通过。
- 依赖：T1101
 - 完成：
   - `crates/scoopc/src/cone/manifest.rs`：新增 `ConeSelectEntry/ConeSelectWhen` 与 `selectors` 字段，解析 `[[select]]`（when/include/exclude）。
 - 验收：
   - `cargo test --all`

### T1111 [DONE] 包加载：按 `[[select]]`（platform selector）对 sources 做 include/exclude（spec §13.9）
- 描述：在 cone source 发现阶段应用 platform selector：
  - `when` 匹配当前 target platform 时，按顺序应用 include/exclude；
  - 保证结果 sources 列表稳定排序与跨平台一致（路径分隔符/大小写策略需明确）。
- 目标：
  - 先只支持 `src/**/*.scoop` 下的路径 glob；
  - 先只支持 host target（交叉编译 target 参数后续再补），但接口要可扩展。
- 验收：
  - 新增 cone fixtures：同一 package 在不同 platform selector 下挑选不同文件集合（可用新增 `EXPECT-SOURCES:` golden 或 `scoop dump-sources` 专用子命令）；
  - `cargo run -p scoop -- test` 通过。
- 依赖：T1110、T1102、T0803
 - 完成：
   - `crates/scoopc/src/cone/package.rs`：在 cone source 发现阶段应用 `[[select]]`：
     - 初始 source set = `src/**/*.scoop`；
     - 对匹配 platform 的 selector 按文件顺序应用 include/exclude；
     - glob 与 path 统一为 UTF-8 + forward slashes，排序按相对路径稳定输出；
     - 入口 `src/main.scoop` 若被排除则返回清晰诊断。
   - `tests/testdata/cone/selectors_basic/*`：新增 cone fixture，覆盖 linux/windows/unknown 三种平台下的 source set 差异。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1103 [DONE] scoopir v0：定义稳定 IR schema（仅 public API）
- 描述：定义一个最小可序列化 schema（JSON/CBOR/自定义）表达 public API（类型/函数签名）。
- 目标：先只覆盖 type + fun header；不包含函数体。
- 验收：新增单测：从 HIR/type env 导出 scoopir；快照测试保证 schema 稳定（带版本号）。
- 依赖：T0702
 - 完成：
   - `crates/scoopc/src/cone/scoopir/*`：新增 ScoopIR v0 schema（JSON/serde）与导出器（从 HIR + TypeEnv 导出 public API）。
   - `tests/fixtures/scoopir/public_api_filter.scoop`：新增 fixture 覆盖可见性过滤（只导出 `public` type/fun）。
   - `tests/fixtures/scoopir/public_api_filter.scoopir.json`：新增 JSON golden，单测回归 schema 稳定性与版本号。
   - `crates/scoop/src/fixtures/mod.rs`：`scoop test` 新增 `scoopir` phase（对 `.scoopir.json` 做 golden 比对）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1104 [DONE] `.cone` 归档 v0：打包 scoopir 与元数据（PLAN §12.2）
- 描述：用 zip/tar 实现最小归档：包含 `Cone.toml`、`api.scoopir`、sources hash。
- 目标：先只实现写包；读包后续任务。
- 验收：`scoop package`（新命令）能生成 `.cone` 文件并列出内容；新增测试验证归档包含必需文件。
- 依赖：T1101、T1103
 - 完成：
   - `crates/scoopc/src/cone/archive.rs`：实现 `.cone`（tar）写入与读取条目（仅用于列出/单测）；归档包含 `Cone.toml`、`api.scoopir`、`SOURCES_SHA256`。
   - `crates/scoopc/src/hir/lower.rs`：新增 `lower_for_compilation_unit` 以支持多文件 cone 的 HIR lowering（供 ScoopIR 导出使用）。
   - `crates/scoopc/src/cone/scoopir/export.rs`：新增 `export_public_api_for_cone_sources` 聚合导出 cone 的 public API。
   - `crates/scoop/src/commands/package.rs` + `crates/scoop/src/cli.rs`：新增 `scoop package` 子命令并输出归档条目列表。
 - 验收：
   - `cargo test --all`

### T1105 [DONE] `.cone` 读取：加载 `api.scoopir` 并参与下游类型检查（spec §13.3）
- 描述：实现从 `.cone` 读取 IR 与元数据，把依赖包的 public API 注入 type env。
- 目标：先只支持同平台/同版本；版本兼容后续任务。
- 验收：新增 cone fixture：A 包导出一个类型/函数，B 包依赖 A 并能通过 typecheck。
- 依赖：T1104、T0402
 - 完成：
   - `crates/scoopc/src/cone/consume.rs`：实现 `.cone` 读取（Cone.toml + api.scoopir）与 public API 注入（Index + TypeEnv）。
   - `crates/scoopc/src/typecheck/type_env.rs`：新增外部 source/type symbol 注入接口，供 `.cone` 依赖使用。
   - `crates/scoop/src/fixtures/mod.rs`：新增 `typecheck_cone_archive` fixtures runner（先打包依赖，再注入 api.scoopir）。
   - `tests/fixtures/typecheck_cone_archive/deps_api_injection/*`：新增 fixture：consumer 通过 `.cone` 依赖成功 typecheck。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

> 注：以下 resolver 任务原位于 T03（包与名字解析）章节；因依赖 `.cone` 读取（T1105），已移动至此处以保持依赖顺序。

### T0321b [DONE] Resolver：接入 `.cone` 依赖的可见性过滤（真实下游）
- 描述：当依赖来自 `.cone` 时，下游只能看到依赖 cone 的 `public` API；`internal/private` 必须在 resolver/typecheck 阶段一致地被拒绝。
- 目标：只打通“加载依赖 API → resolve 可见性过滤 → 稳定诊断”主路径；friend module 等高级规则不做。
- 验收：新增 cone fixture：B 依赖 A（`.cone`），可引用 A 的 `public` 类型/函数；引用 A 的 `internal/private` 报 `not_visible` 且定位稳定。
- 依赖：T0321a、T1105
 - 完成：
   - `crates/scoopc/src/cone/visibility.rs`：新增 `SYMBOL_VISIBILITY.json`（仅记录非 public 符号的存在性 + 可见性层级）生成/解析，用于下游稳定产出 `not_visible`。
   - `crates/scoopc/src/cone/archive.rs`：打包时写入 `SYMBOL_VISIBILITY.json`；新增 `try_read_cone_archive_entry` 以支持可选元数据向前兼容。
   - `crates/scoopc/src/cone/consume.rs`：读取 `SYMBOL_VISIBILITY.json` 并把 non-public 符号以“不可见占位符”注入 resolver `Index`（不注入 TypeEnv），从而在使用点统一报 `scoop::resolve::not_visible`。
   - `tests/fixtures/typecheck_cone_archive/deps_visibility_filter/*`：新增真实 `.cone` 依赖 fixtures：public API 可用；internal type/private fun 报 `not_visible`（带稳定 code+位置断言）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T0322 [DONE] Resolver：跨包 extension 导入与候选收集
- 描述：补齐 extension 在跨包场景下的导入与发现规则：显式 import、star import、可见性过滤、shadowing，以及把可见候选写入调用点候选集。
- 目标：先固定“能否发现某个 imported extension”的规则；最终 overload 决议与 receiver specificity 仍交给 typecheck/infer。
- 验收：新增多包 fixtures：显式导入的 extension 可被发现，未导入时不可见；star import 与本地成员同名时仍遵守 member 优先。
- 依赖：T0312、T0319、T0321
 - 完成：
   - `crates/scoopc/src/resolve/scopes.rs`：新增 `extension_fun_candidates`，把 extension 发现规则扩展到“同包（同 cone）+ 显式 import（含 alias）+ star import”，并接入 `resolve_member_access_on_value_receiver`；同时在 member call 写回“可见 extension 候选集合”到 `MemberIdent.call`（供后续 typecheck/infer 决议）。
   - `crates/scoopc/src/cone/consume.rs`：`.cone` API 注入时同步把 extension fun 元信息写入 `Index::extension_funs`，使下游可通过 import 发现依赖 cone 的 extension。
   - `crates/scoopc/src/resolve/mod.rs`：移除旧的“仅同包” extension 查找辅助函数（避免死代码与规则分叉）。
   - `tests/fixtures/resolve_cone/extension_imports/**`：新增 fixtures 覆盖显式导入可见/未导入不可见/star import 规则与 member 优先（含 member 不可见时不回退）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1106 [DONE] IR 稳定性与版本协商（spec §13.4）
- 描述：为 scoopir 增加显式版本号，并实现“旧版本可读/不兼容报错”的策略。
- 目标：先只做版本号检查；不实现自动升级。
- 验收：新增单测：构造一个旧版本 header 读取成功或按规则失败；错误码稳定。
- 依赖：T1103、T1105
 - 完成：
   - `crates/scoopc/src/cone/consume.rs`：实现 `api.scoopir` schema version 协商（允许读取 <= 当前版本；更高版本报错），并为 schema name/version mismatch 增加稳定错误码；新增单测覆盖“当前版本 OK / 更高版本失败且 code 稳定”。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1107 [DONE] consumer 编译与链接流程（多包）（spec §13.3）
- 描述：实现 `scoop build` 能处理依赖图：先加载依赖 cone，再编译当前包，最后链接。
- 目标：先不做增量；先只支持 DAG，无循环依赖。
- 验收：cone fixture：两包依赖编译并链接成可执行，运行输出正确。
- 依赖：T1105、T0806
 - 完成：
   - `crates/scoop/src/commands/build/deps.rs`：实现 `.cone` 依赖图解析（DAG 拓扑序 + 循环检测 + 版本字符串精确匹配），支持通过 `SCOOP_CONE_PATH` 或 consumer 包目录下 `cone/`、`deps/` 搜索依赖归档。
   - `crates/scoop/src/commands/build.rs`：`scoop build` 在 cone 包模式下注入依赖 `.cone` 的 public API（resolver/typecheck），并在启用 `llvm` 时复用同一编译单元 lowering 结果生成 `.ll`/`.o`/`.s`/可执行文件；新增单测覆盖“带 `.cone` 依赖的 build 前端通过 +（llvm 下）可执行输出与 stdout”。
   - `crates/scoopc/src/llvm/mod.rs`：新增基于 `hir::LoweredHir` 的 emit 入口（IR/obj/asm），避免后端二次 parse/resolve 导致多包依赖 import 失效。
 - 验收：
   - `cargo test --all`
   - `cargo test -p scoop --features llvm`
   - `cargo run -p scoop -- test`

### T0629b [DONE] program boundary：库导出入口 + host/embedded entry points（需要 build 链路）
- 描述：定义并实现“库导出入口”（例如作为 `.cone` 动态/嵌入式入口）与 host callback 的 entry point 集合，并固定哪些入口必须显式 `Pure!`。
- 目标：入口集合来源可配置（例如 Cone.toml 或 `scoop build --entry ...`）；不做运行时动态扫描。
- 验收：新增 cone fixtures：多个 entry point 共存时规则稳定；导出入口若声明非 `Pure` 或漏处理 effect 会被拒绝。
- 依赖：T1107
 - 完成：
   - `crates/scoopc/src/cone/manifest.rs`：解析 `Cone.toml` 可选段 `[entry-points].exports`，并在 `ConeManifest` 中保存 `export_entry_points`。
   - `crates/scoopc/src/resolve/mod.rs`：`Index` 增加导出入口集合，并提供 `set_export_entry_points/is_export_entry_point` 供 typecheck 判定。
   - `crates/scoopc/src/typecheck/expr.rs`：当函数 FQN 命中导出入口集合时，按 entry point 规则强制其显式声明 `/ Pure!`；新增稳定诊断：
     - `scoop::typecheck::export_entry_point_must_declare_closed_pure`
     - `scoop::typecheck::export_entry_point_must_be_pure`
     - `scoop::typecheck::export_entry_point_must_be_closed_pure`
   - `crates/scoop/src/commands/build.rs`：cone 包模式下将 `Cone.toml` 的导出入口配置注入 `Index`。
   - `crates/scoop/src/fixtures/mod.rs`：`typecheck_cone_archive` runner 注入导出入口配置，便于用真实 cone fixtures 回归。
   - `tests/fixtures/typecheck_cone_archive/program_boundary_export_entry_points/**`：新增用例覆盖“多导出入口共存”与“non-Pure/漏处理 effect 被拒绝”。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1108 [DONE] pre-specialize：从 Cone.toml 指定常用单态化实例（spec §13.7）
- 描述：支持在 Cone.toml 中列出需要预编译的泛型实例，并在打包时写入 `.cone`。
- 目标：先只支持函数实例；类型实例后续。
- 验收：新增 cone fixture：指定 `id<Int>` 预编译；下游消费时无需再次单态化（可用 dump 日志/计数验证）。
- 依赖：T0712、T1104
 - 完成：
   - `crates/scoopc/src/cone/manifest.rs`：解析 `[pre-specialize].functions`（兼容 `funs`）与 `types` 并写入 `ConeManifest`（types 暂只保存，留给 T1109）。
   - `crates/scoopc/src/cone/pre_specialize.rs`：新增 `PRE_SPECIALIZE.json`（schema v0）以及基于 sources 的预编译实例生成（当前以 MIR Debug 文本占位产物）。
   - `crates/scoopc/src/cone/archive.rs`：打包 `.cone` 时按需写入 `PRE_SPECIALIZE.json`（未声明则保持不写入）。
   - `crates/scoopc/src/cone/consume.rs`：下游读取 `.cone` 时可选加载 `PRE_SPECIALIZE.json` 到 `ConeArchiveApi`。
   - `crates/scoop/src/fixtures/expectations.rs`：新增 `EXPECT-MONOMORPH-HIT/MISS` 指令解析。
   - `crates/scoop/src/fixtures/mod.rs`：在 `typecheck_cone_archive` runner 中注入依赖 `.cone` 的 pre-specialize 索引，并在表达式 typecheck 时收集 `MonomorphKey` 做命中/缺失计数断言。
   - `tests/fixtures/typecheck_cone_archive/pre_specialize_id_int/**`：新增真实 `.cone` 依赖 fixture：lib 在 `Cone.toml` 预编译 `id<Int>`；consumer 断言命中 1、缺失 0。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1109 [DONE] pre-specialize：类型实例的导出与消费
- 描述：把 pre-specialize 从“仅函数实例”扩展到类型实例：泛型 struct/class/enum 的常用实例可被预编译、打包进 `.cone`，并被下游直接复用。
- 目标：先覆盖无递归或有限递归的常见类型实例；不要求一次处理所有互递归图。
- 验收：新增 cone fixtures：预编译 `Vec<Int>` 或等价类型实例后，下游消费不再重新生成相同实例；dump 日志或计数可证明命中缓存。
- 依赖：T1108、T0712
 - 完成：
   - `crates/scoopc/src/cone/pre_specialize.rs`：扩展 `PRE_SPECIALIZE.json` v0 schema，新增 `types` 条目；支持从 `Cone.toml` 的 `[pre-specialize].types` 生成并写入类型实例索引，并在消费侧提供 `type_key_set()` 供下游做命中计数。
   - `crates/scoopc/src/typecheck/lower.rs`：在 `TypeLowering` 中引入可选的“泛型类型实例化 key”收集（去重 + 稳定顺序），并新增 `check_file_type_refs_with_type_instantiation_keys` 入口用于 fixtures。
   - `crates/scoopc/src/typecheck/expr.rs`：表达式 typecheck 新增可选的“类型实例化 key”收集入口，并提供同时收集 monomorph+type inst 的组合函数，避免重复执行 typecheck。
   - `crates/scoop/src/fixtures/expectations.rs`：新增 `EXPECT-TYPE-MONOMORPH-HIT/MISS` 指令解析。
   - `crates/scoop/src/fixtures/mod.rs`：typecheck_cone_archive runner 读取依赖 `.cone` 的 `PRE_SPECIALIZE.json` 类型索引，并对 consumer 文件收集到的类型实例做 hit/miss 断言。
   - `tests/fixtures/typecheck_cone_archive/pre_specialize_type_box_int/**`：新增真实 `.cone` 依赖 fixture：lib 在 `Cone.toml` 预生成 `Box<Int>`；consumer 断言 type hit 1、miss 0。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

---

## T12：编译期执行与反射（阶段 11）

### T1201 [DONE] const fun：在 AST/HIR 中标记 `const fun`（spec §6.2）
- 描述：解析并在 HIR 中标记 const 函数，便于后续解释器选择入口。
- 目标：先只做标记与语法限制检查（如禁止 effect）。
- 验收：新增 parse/typecheck fixture：`const fun add(a:Int,b:Int):Int { ... }`；非法操作在 typecheck 报错（规则可先简化）。
- 依赖：T0246、T0404
 - 完成：
   - `crates/scoopc/src/hir/mod.rs`：HIR `FunDecl` 新增 `is_const` 字段，并在 Debug 输出中仅当 `true` 时打印（保持既有 `.hir` golden 稳定）。
   - `crates/scoopc/src/hir/lower.rs`：从 AST `Modifier::Const` 传播到 HIR `is_const`。
   - `crates/scoopc/src/typecheck/headers.rs`：新增 const fun 的最小门禁：
     - 禁止声明 effect row 参数（稳定错误码 `scoop::typecheck::const_fun_eff_param_not_allowed`）。
     - 禁止声明非 Pure 的 effect row（稳定错误码 `scoop::typecheck::const_fun_effects_not_allowed`）。
   - `tests/fixtures/hir/const_fun_basic.*`：新增 HIR golden 回归覆盖 `is_const: true`。
   - `tests/fixtures/typecheck/const_fun_*`：新增 typecheck pass/fail fixtures 覆盖 const fun 允许 Pure、禁止 non-Pure effects 与 eff param。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1202 const interpreter v0（拆分为子任务，PLAN §13）
- 描述：实现解释器能执行：整数运算、tuple/struct/enum 构造、`String` 操作、函数调用（仅 const）。`String` 虽为引用类型，但具备值语义，在 comptime 中特殊处理。
- 目标：不支持堆分配（`String` 除外）、不支持 effects、不支持循环（可先限制）。
- 备注：该任务涉及“值模型 + 求值器 + 调用/环境 + fixtures 接入”，为保证每步都可单独验证，拆分为以下子任务。

### T1202a [DONE] const interpreter：值模型 + 纯表达式求值 v0（字面量/一元/二元）
- 描述：新增 `crates/scoopc::comptime`，提供最小的 `ConstValue` 与表达式求值器，覆盖：
  - 字面量：Int/Bool/Unit/String（从 `SourceFile + Span` 解析源码）
  - 一元：`!`/`-`/`~`
  - 二元：算术/位运算/比较/逻辑（按整数位宽做 wrap/mask；逻辑运算保持短路）
- 目标：暂不支持函数调用、tuple/struct/enum aggregate、控制流（`if/when`）、effects 与循环。
- 验收：新增单元测试覆盖上述求值；`cargo test -p scoopc` 通过。
- 依赖：T1201
 - 完成：
   - `crates/scoopc/src/lib.rs`：新增 `pub mod comptime;`，暴露编译期求值模块。
   - `crates/scoopc/src/comptime/mod.rs`：新增模块骨架与对外 API（`ConstValue`/`eval_const_expr`）。
   - `crates/scoopc/src/comptime/value.rs`：实现 `ConstValue`/`ConstInt`/`ConstIntTy`（支持位宽 wrap/mask）。
   - `crates/scoopc/src/comptime/eval.rs`：实现纯表达式常量求值（字面量 + 一元/二元运算，含短路）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单元测试覆盖算术/位运算/短路与有符号/无符号右移。
 - 验收：
   - `cargo test -p scoopc`

### T1202b [DONE] const interpreter：支持 tuple/struct/enum 的值构造与访问（最小）
- 描述：在 `ConstValue` 中补齐 tuple/struct/enum 表示，并在求值器中支持最小构造（如 tuple/struct literal、enum ctor）。
- 目标：先只做“构造 + 读取”所需最小语义；不做模式匹配与复杂布局。
- 验收：新增单元测试覆盖 tuple/struct/enum 的构造与字段读取；`cargo test -p scoopc` 通过。
- 依赖：T1202a
 - 完成：
   - `crates/scoopc/src/comptime/value.rs`：`ConstValue` 扩展 tuple/struct/enum 变体与最小数据结构。
   - `crates/scoopc/src/comptime/eval.rs`：支持 tuple/struct literal、enum ctor（含 unit variant）与成员访问（`_0/_1/...`、struct 字段）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 tuple/struct/enum 的构造与字段读取。
 - 验收：
   - `cargo test -p scoopc`
   - `cargo test --all`

### T1202c [DONE] const interpreter：支持 `const fun` 调用（仅 Pure）并接入 `tests/fixtures/comptime`
- 描述：实现 const fun 的 body 求值（局部 `val`、表达式返回、最小 block），并在 fixtures runner 中新增 `comptime` phase 以回归 const 求值结果。
- 目标：先保守：不支持闭包/lambda、不支持 `perform/handle`、不支持循环；对递归设定上限避免无限求值。
- 验收：新增 `tests/fixtures/comptime/*`：const 计算结果可用于类型/代码生成位置（可先以编译期常量折叠为目标）；`cargo run -p scoop -- test` 通过。
- 依赖：T1202b、T0702
 - 完成：
   - `crates/scoopc/src/comptime/eval.rs`：引入 `ConstEvalHost` + `eval_const_expr_with_host`，为解释器注入“名字解析/函数调用”能力；call 表达式在 enum ctor 与 `const fun` 之间做最小分流。
   - `crates/scoopc/src/comptime/interpreter.rs`：实现 `const fun` 调用（按 name+arity 最小选择）、局部 `val`、`return`、block 末尾表达式返回；并对递归深度设上限（稳定错误码）。
   - `crates/scoopc/src/comptime/mod.rs`：导出 `eval_const_bindings_in_file`/`ConstBinding` 作为 fixtures/后续阶段接入点。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 const fun 调用、调用非 const fun 的稳定错误码、递归深度超限的稳定错误码。
   - `crates/scoop/src/fixtures/mod.rs`：新增 fixtures `comptime` phase，执行 `const val` 常量折叠并与 `.comptime` golden 比对。
   - `tests/fixtures/comptime/*`：新增 pass/fail fixtures 回归覆盖 const fun 调用与诊断。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1203 [DONE] `comptime { ... }` block 语法与执行入口（spec §6.3）
- 描述：解析 comptime block 并在编译时执行（Pure 限制）。
- 目标：先只支持 block 内 `val` 与 `comptime if`（spec §6.3.2）；`comptime for` 后续。
- 验收：comptime fixture：生成若干声明/常量（即使仅影响常量值）；失败时诊断可读。
- 依赖：T1202
 - 完成：
   - `crates/scoopc/src/comptime/interpreter.rs`：const 解释器支持执行 `comptime { ... }` 与 `comptime if`（含 else-if 链），条件必须为 Bool，未选中分支不求值。
   - `crates/scoopc/src/comptime/eval.rs`：导出 `value_kind` 供解释器复用（保持诊断一致性）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖：
     - comptime if 分支裁剪（未选中分支含除以 0 也不报错）；
     - else-if 链求值；
     - 条件非 Bool 的稳定错误码。
   - `tests/fixtures/comptime/*`：新增 pass fixture `comptime_block_and_if_basic` 与 fail fixture `comptime_if_condition_not_bool_is_error` 回归覆盖。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1204 [DONE] 反射 intrinsics v0：`nameOf/sizeOf/fieldsOf` 的声明与解释器支持（spec §6.4）
- 描述：在 sysroot 中声明内建反射函数，并在 comptime 执行时实现其行为。
- 目标：先只支持 struct 的字段列表与基本类型 size；RTTI 后续。
- 验收：comptime fixture：基于 `fieldsOf<T>()` 生成序列化代码片段（可先只打印/导出元数据）。
- 依赖：T1203、T0010
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `ExprKind::TypeApply`，用于承载 `callee<T>` 的显式类型实参（避免改动 `Call` 形态导致 AST golden 漂移）。
   - `crates/scoopc/src/parser/expr.rs`：支持解析 `callee<T>()`（仅在 `>` 后紧跟 `(` 时生效，避免与 `<` 比较运算歧义）；生成 `TypeApply + Call` 结构。
   - `crates/scoopc/src/comptime/eval.rs`：`ConstEvalHost::call_fun` 增加 `type_args`；支持 `TypeApply` 作为 call callee，并把类型实参传给宿主。
   - `crates/scoopc/src/comptime/interpreter.rs`：在 const 解释器中内建实现 `nameOf/sizeOf/fieldsOf`：
     - `nameOf<T>()` 返回类型名字符串；
     - `sizeOf<T>()` 支持基础类型（Bool/Unit/Int/Int8/Int64...）与 `String`（指针大小）；
     - `fieldsOf<T>()` 支持 struct：收集主构造 `val/var` 字段 + 无 getter/setter/delegate 的 body 属性字段（v0 返回字段名 tuple）。
   - `sysroot/core.scoop`：补齐 `ComptimeList<T>` 声明，并新增 `const fun <T> nameOf/sizeOf/fieldsOf` 的 sysroot 表面（v0：`fieldsOf<T>() -> ComptimeList<String>`）。
   - `tests/fixtures/comptime/reflection_intrinsics_v0_basic.*`：新增 comptime fixture 回归覆盖三个 intrinsics。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 `nameOf/sizeOf/fieldsOf` 的 v0 语义与字段过滤规则。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1205 [DONE] Splice operator（`value.[field]`）最小实现（spec §6.4）
- 描述：支持 `.[field]` 语法，在 comptime for 中通过 FieldMeta 访问值的特定字段（先限用于声明位置）。
- 目标：先只做“生成声明列表”；不做表达式 splice。
- 验收：comptime fixture：通过 splice 生成一个函数/struct，后续解析/类型检查通过。
- 依赖：T1203、T0201
 - 完成：
   - `crates/scoopc/src/comptime/eval.rs`：const eval 支持 `ExprKind::SpliceField`：
     - `field` 为 `String`（字段名）时读取 struct 字段；
     - `field` 为包含 `name: String` 的 struct 值时读取该字段（为后续 FieldMeta 兼容预留）。
   - `crates/scoopc/src/typecheck/expr.rs`：typecheck 支持 `ExprKind::SpliceField`：
     - `field` 为字符串字面量时，按 `receiverTy.<name>` 推导字段类型；
     - `field` 非字面量时，当前阶段保守退化为 `Any`（留给后续 comptime 展开/元数据补齐）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 splice 字段读取（字符串字面量 + `name` 结构体形态）。
   - `tests/fixtures/comptime/splice_field_access_v0_basic.*`：新增 comptime fixture 回归覆盖 splice 字段读取。
   - `tests/fixtures/typecheck/splice_field_access_string_lit_ok.scoop`：新增 typecheck fixture 回归覆盖字符串字面量字段 splice。
   - `tests/fixtures/typecheck/splice_field_unknown_field_is_error.scoop`：新增 typecheck fail fixture：未知字段返回稳定错误码。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1206 [DONE] RTTI v0：为类型生成运行期描述符（spec §6.6）
- 描述：定义运行期类型信息结构（type id/field layout），先能在调试/反射中使用。
- 目标：先只生成静态表；GC trace bitmap 后续。
- 验收：新增 `--dump-rtti`（或测试）可打印某个类型的 RTTI；输出稳定。
- 依赖：T0802、T0409
 - 完成：
   - `crates/scoopc/src/rtti/mod.rs`：新增 RTTI v0（type id + size/align + struct 字段 offset）与单测。
   - `crates/scoopc/src/lib.rs`：导出 `scoopc::rtti` 模块。
   - `crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/mod.rs`：开放 `TypeLowering` 供 crate 内工具复用。
   - `crates/scoop/src/cli.rs`：新增 `scoop dump-rtti` 子命令（支持 `--type` 过滤单个类型）。
   - `crates/scoop/src/commands/dump_rtti.rs`、`crates/scoop/src/commands/mod.rs`：接入子命令实现与 dispatch。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- dump-rtti tests/fixtures/typecheck/struct_lit_ok.scoop --type Point`

### T1207 [DONE] `comptime for`：编译期循环（spec §6.3.1）
- 描述：实现 `comptime for` 的语法与执行（对编译期常量范围/列表迭代）。
- 目标：先只支持整数范围与固定数组；break/continue 后置。
- 验收：comptime fixture：用 comptime for 生成重复的字段/函数；生成代码能通过 parse/typecheck。
- 依赖：T1203
 - 完成：
   - `crates/scoopc/src/syntax/token.rs`：新增 `Symbol::DotDot`（`..`）。
   - `crates/scoopc/src/syntax/lexer.rs`：lexer 支持 `..` token。
   - `crates/scoopc/src/ast/mod.rs`：新增 `BinaryOp::RangeInclusive`（`a..b`）。
   - `crates/scoopc/src/parser/expr.rs`：二元运算解析支持 `..`（range）。
   - `crates/scoopc/src/parser/pattern.rs`、`crates/scoopc/src/parser/expr.rs`：更新 `..` rest pattern 解析以兼容新 token（tuple/struct/when patterns）。
   - `crates/scoopc/src/comptime/eval.rs`：const eval 支持 `ArrayLit`（v0 以 `ConstValue::Tuple` 承载）。
   - `crates/scoopc/src/comptime/interpreter.rs`：const 解释器支持执行 `comptime for`（整数范围 `a..b` + tuple/array 迭代）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 range/array/tuple 迭代。
   - `tests/fixtures/comptime/comptime_for_range_and_array_basic.*`：新增 comptime fixture 回归覆盖。
   - `crates/scoopc/src/typecheck/expr.rs`、`crates/scoopc/src/typecheck/annotations.rs`、`crates/scoopc/src/hir/lower.rs`：补齐 `RangeInclusive` 的最小处理，保持 workspace 可编译。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1208 [DONE] 编译期元数据类型：`TypeMeta/FieldMeta/PropertyMeta`（spec §6.4、§10.4）
- 描述：实现编译期可用的元数据结构（字段名/类型/属性名/owner 等），供反射与委托属性生成使用。
- 目标：先只支持 struct 字段与 class 属性；注解信息后续任务补。
- 验收：comptime fixture：`fieldsOf<T>()` 返回 FieldMeta 列表；并能读取 name/type。
- 依赖：T1204、T0409
 - 完成：
   - `sysroot/core.scoop`：新增 `TypeMeta/FieldMeta/PropertyMeta` 声明；`fieldsOf<T>()` 签名升级为 `ComptimeList<FieldMeta>`。
   - `crates/scoopc/src/comptime/interpreter.rs`：`fieldsOf<T>()` 在 comptime 执行时返回 `FieldMeta { name, type: TypeMeta, index }` 列表，并允许 target 为 `struct/class`。
   - `crates/scoopc/src/comptime/eval.rs`：更新 `ReflectionUnsupportedTarget` 的错误消息为 `struct/class`。
   - `crates/scoopc/src/comptime/tests.rs`：更新反射 intrinsics 单测，并新增 class 覆盖用例。
   - `tests/fixtures/comptime/reflection_intrinsics_v0_basic.*`：更新 `.comptime` golden，并新增读取 `field.type.name` 的常量断言。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1209 [DONE] 编译期注解访问（spec §15.6）
- 描述：在 comptime/reflection API 中暴露读取注解的能力（限定在编译期使用）。
- 目标：先只支持读取注解名与字面量参数；复杂表达式参数后置。
- 验收：comptime fixture：读取 `@Deprecated("x")` 的参数并生成代码/诊断；输出稳定。
- 依赖：T1001、T1208
 - 完成：
   - `sysroot/core.scoop`：新增 `AnnotationMeta/AnnotationArgMeta` 与 `annotationsOf<T>()` 的声明面。
   - `crates/scoopc/src/comptime/interpreter.rs`：const 解释器内建实现 `annotationsOf<T>()`（v0：type-level），支持读取注解名与字面量/常量表达式参数；positional 参数名优先从 `annotation class` 主构造参数名推导。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 `annotationsOf<T>()` 的注解读取（positional + named）。
   - `tests/fixtures/comptime/annotation_access_v0_basic.*`：新增 comptime fixture + `.comptime` golden 回归覆盖。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1016b [DONE] meta-annotations：按 `@Retention` 导出到 `.cone` 并在下游可见
- 描述：把标记为 cone-preserved 的注解写入 `.cone` 元数据（或 scoopir），并在下游编译/反射中可读；comptime-only 不导出。
- 目标：先只保证“下游可见/不可见”的边界；注解参数复杂表达式留给 T1019/T1209 之后再补。
- 验收：新增 cone fixture：cone-preserved 注解在下游可见；comptime-only 注解在下游不可见（或为空）。
- 依赖：T1103、T1209、T1016a
 - 完成：
   - `crates/scoopc/src/cone/annotations.rs`：新增 `ANNOTATION_CLASSES.json`（schema v0）与导出/解析逻辑；仅导出 `@Retention("cone")` 的 public annotation classes。
   - `crates/scoopc/src/cone/scoopir/export.rs`：ScoopIR 导出时过滤掉 comptime-only annotation classes（默认 comptime-only，只有显式 `cone` 才导出）。
   - `crates/scoopc/src/cone/archive.rs`：`.cone` 打包写入 `ANNOTATION_CLASSES.json`。
   - `crates/scoopc/src/cone/consume.rs`：`.cone` 读取并注入注解类元信息：下游可识别 `annotation class`，并按导出集实现“可见/不可见”边界。
   - `tests/fixtures/typecheck_cone_archive/annotation_retention_export/*`：新增真实 `.cone` 依赖 fixtures：`cone` 注解类可用；`comptime` 注解类不可见（报 `unresolved_annotation_type`）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1210 [DONE] 委托属性 lowering 所需：生成 `PropertyMeta` 常量并传参（spec §10.4）
- 描述：为 delegated property 生成静态 `PropertyMeta` 常量，并在 getter/setter 转发时传入。
- 目标：先只生成元数据与调用形状；delegate 的具体库实现不要求存在。
- 验收：typecheck/IR fixture：delegated property lowering 后 getter 调用包含 `PropertyMeta` 参数（可用 dump-hir 验证）。
- 依赖：T0434、T1208
 - 完成：
   - `crates/scoopc/src/hir/lower.rs`：HIR lowering 预扫描 delegated properties（`Owner.prop`），并把
     - `receiver.prop` 降糖为 `receiver.prop$delegate.getValue(receiver, Owner.$PropertyMeta$prop)`；
     - `receiver.prop = v` 降糖为 `receiver.prop$delegate.setValue(receiver, Owner.$PropertyMeta$prop, v)`。
   - `tests/fixtures/hir/delegated_property_lowering.*`：新增 HIR golden fixture，回归验证 `getValue/setValue` 的第二参为合成的 `PropertyMeta` 常量引用。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1211 [DONE] `const fun` 规则：allowed/prohibited 清单的静态 enforcement（spec §6.2）
- 描述：实现 const fun 的限制：禁止 perform/分配/IO/闭包等；允许纯计算、value types 操作与 `String`（值语义特例）。
- 目标：先保守（宁可多报错）；后续逐步放宽。
- 验收：comptime fixture：在 const fun 中调用非 const fun 报错；在 const fun 中进行纯算术通过；`String` 操作通过；使用闭包/lambda 报错。
- 依赖：T1201、T1005
 - 完成：
   - `crates/scoopc/src/resolve/mod.rs`：`Index` 的 `FunSig` 增加 `is_const`，用于跨文件调用点判断 const 性质。
   - `crates/scoopc/src/typecheck/lower.rs`：`TypeLowering` 增加 `const` 上下文深度（push/pop/in），供表达式 typecheck 使用。
   - `crates/scoopc/src/typecheck/expr.rs`：
     - 在 `const fun` 函数体内 push/pop const 上下文；
     - 新增调用门禁：`const fun` 只能调用 `const fun` 或 `@Intrinsic`（稳定错误码 `scoop::typecheck::const_fun_call_forbidden`）；
     - 禁止 lambda（稳定错误码 `scoop::typecheck::const_fun_lambda_not_allowed`）；
     - 禁止 class ctor（稳定错误码 `scoop::typecheck::const_fun_ref_type_construction_not_allowed`）；
     - 禁止“可能分配”的 boxing（稳定错误码 `scoop::typecheck::const_fun_boxing_forbidden`）。
   - `crates/scoopc/src/cone/consume.rs`：在从 `.cone` 构建 `FunSig` 时补齐 `is_const`（v0 先置为 `false`；后续可随 ScoopIR schema 扩展导出该信息）。
   - `tests/fixtures/typecheck/const_fun_*`：新增 pass/fail fixtures 覆盖 non-const call、lambda、String 特例与 class ctor。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1212 [DONE] 运行期值上的反射回退路径（spec §6.4 末尾说明）
- 描述：当反射 API 的 receiver 是运行期值时，遵循 const fun 规则回退为普通运行期调用（不做编译器特殊处理）。
- 目标：先在文档与测试中固定该行为；实现上不需要特殊 case。
- 验收：新增文档/fixture：同一个反射调用在 comptime 与 runtime 下分别走不同路径但语义一致。
- 依赖：T1204
 - 完成：
   - `crates/scoopc/src/typecheck/expr.rs`：typecheck 支持 `Call(TypeApply(...))` 的显式类型实参调用，并在函数调用实例化处接入 explicit type args（不依赖“反射特判”）。
   - `SCOOP_FULL_SPEC.md`：在 §6.4 增加 “Runtime Fallback (No Special-Case)” 小节，明确遵循 §6.2 的 `const fun` 规则。
   - `tests/fixtures/comptime/reflection_runtime_fallback_v0.*`：新增 comptime fixture，回归 `const val` 中的 `nameOf<Point>()` 常量折叠结果。
   - `tests/fixtures/typecheck/reflection_runtime_fallback_v0.scoop`：新增 typecheck fixture，回归运行期语境下 `nameOf<Point>()` 可通过 typecheck。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1213 [DONE] sysroot：补齐 scope functions 的 effect-polymorphic 签名（spec §11）
- 描述：在 sysroot 中声明 `let/run/also/apply` 的推荐签名（含 `<eff E = Pure>` 与 receiver function type）。
- 目标：先只做声明；标准库实现可后置。
- 验收：typecheck fixture：调用 `x.run { ... }` 的 effect row 会传播（在推断/required effects 上可观测）。
- 依赖：T0509、T0219
 - 完成：
   - `sysroot/core.scoop`：新增 `let/run/also/apply` 的 scope functions 声明，包含 `<eff E = Pure>` 与 receiver function type；
     - v0 限制：由于当前泛型调用推断仅支持单一 type param，`let/run` 的返回类型先使用 `Any` 作为擦除视图（保持 effect row 传播）。
   - `crates/scoopc/src/resolve/mod.rs`、`crates/scoopc/src/resolve/scopes.rs`：resolver 支持“receiver 为声明处 type param（例如 `<T> T.ext()`）”的扩展函数参与候选匹配（通配）。
   - `crates/scoopc/src/typecheck/lower.rs`：新增 `lower_type_ref_in_decl_file_with_scopes()`，用于跨文件 `<eff E>` 签名 lowering。
   - `crates/scoopc/src/typecheck/expr.rs`：
     - 扩展函数调用在缺失当前文件签名时回退到 `Index`（并支持跨文件 `<eff E>`）；
     - lambda expected-context 推断支持 receiver function type；
     - `FunSigOwned` 增加 `decl_file`，required effects 处使用 decl_file 上下文 lower effects，避免跨文件 span slice panic。
   - `tests/fixtures/typecheck/scope_functions_run_effect_propagates_missing_is_error.scoop`：新增 fail fixture，断言 `x.run { Raise.raise(1) }` 会触发 `scoop::typecheck::required_effect_not_declared`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1214 [DONE] 反射 intrinsics 完整化：`variantsOf/alignOf/superTypesOf/annotationsOf/paramsOf`（spec §6.4 / §15.6）
- 描述：在 sysroot + comptime evaluator 中补齐缺失的反射 intrinsic：`variantsOf<T>()`、`alignOf<T>()`、`superTypesOf<T>()`、`annotationsOf<T>()`、`paramsOf(fn)`。
- 目标：先只覆盖 language spec 已列出的最小集合；复杂跨包元数据后续与 Cone 联动。
- 验收：新增 comptime fixture：读取 enum variants、alignment、super types、函数参数与注解列表；输出稳定。
- 依赖：T1204、T1209、T0418
 - 完成：
   - `sysroot/core.scoop`：新增 `FunctionMeta`（v0：仅 `name` 字段）并补齐 `alignOf/variantsOf/superTypesOf/paramsOf` 的 sysroot 声明面。
   - `crates/scoopc/src/comptime/interpreter.rs`：const 解释器内建实现：
     - `alignOf<T>()`（v0：builtin/指针对齐）；
     - `variantsOf<T>()`（v0：返回 enum variant 名列表）；
     - `superTypesOf<T>()`（返回 `TypeMeta` 列表，基于声明处 supertypes）；
     - `paramsOf(fn)`（v0：以 `FunctionMeta { name }` 或字符串提供句柄，返回 `FieldMeta` 列表视图）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖上述 intrinsics 的最小可回归语义。
   - `tests/fixtures/comptime/reflection_intrinsics_v0_more.*`：新增 comptime fixture + `.comptime` golden 回归覆盖 enum variants / align / supertypes / params / annotations 组合用例。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1215 [DONE] 编译期元数据补齐：`VariantMeta/ParamMeta/FunctionMeta/AnnotationMeta/AnnotationArgMeta`（spec §6.4 / §15.6）
- 描述：补齐反射所需的元数据结构，并让它们可在 comptime 中被访问和迭代。
- 目标：先只支持只读 metadata；不支持运行期动态修改。
- 验收：comptime fixture：`variantsOf<T>()` 返回 `VariantMeta`；`paramsOf(fn)` 返回 `ParamMeta`；注解参数可通过 `AnnotationArgMeta` 读取。
- 依赖：T1208、T1214
 - 完成：
   - `sysroot/core.scoop`：
     - 新增 `TypeKind` 与 `VariantMeta/ParamMeta`；
     - `TypeMeta/FieldMeta` 补齐 `kind/annotations` 与 `annotations` 字段；
     - `variantsOf<T>()` 签名升级为 `ComptimeList<VariantMeta>`；
     - `paramsOf(fn)` 签名升级为 `ComptimeList<ParamMeta>`。
   - `crates/scoopc/src/comptime/interpreter.rs`：
     - `variantsOf<T>()` 在 comptime 执行时返回 `VariantMeta { name, fields, index, annotations }`；
     - `paramsOf(fn)` 返回 `ParamMeta { name, type, index, annotations }`；
     - `TypeMeta` v0 补齐 `kind/annotations`（仅当当前文件内存在唯一声明时可识别非 Primitive kind）；
     - `AnnotationMeta.args` 按 ctor 参数名解析并补齐默认值（无法定位 annotation class 时回退为“仅读取提供的 args”）。
   - `crates/scoopc/src/comptime/tests.rs`：更新/扩展单测覆盖 `VariantMeta/ParamMeta/TypeKind` 与 enum variant fields。
   - `tests/fixtures/comptime/reflection_intrinsics_v0_{basic,more}.*`：更新 fixtures 与 `.comptime` golden 回归覆盖 `VariantMeta/ParamMeta`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1216 [DONE] `trimIndent()`：编译期求值 + 普通运行期回退约定（spec §8.4 / §6.2）
- 描述：把 `trimIndent()` 纳入 `String` 的 `const fun` 语义：接收者是编译期常量时在编译期求值，否则保留为普通运行期调用。
- 目标：先只覆盖 raw string 与普通 string 的常见路径；不做额外字符串 API 扩展。
- 验收：新增 comptime fixture：raw string `.trimIndent()` 在编译期折叠；运行期 fixture 由 T0827 验证 fallback 路径。
- 依赖：T1202、T1211
 - 完成：
   - `crates/scoopc/src/comptime/eval.rs`：在 const eval 中支持 `String.trimIndent()` 折叠，并实现与 runtime 一致的 trim 算法（含 CRLF 兼容与空白行规范化）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测覆盖 raw string `.trimIndent()` 编译期折叠。
   - `tests/fixtures/comptime/string_trim_indent_const_fold_basic.*`：新增 comptime fixture + `.comptime` golden 回归。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1217 [DONE] sysroot/stdlib：标准 delegated properties API surface（spec §10.4）
- 描述：在 sysroot 或标准库层补齐 delegated properties 的 API surface：`scoop.delegates.lazy`、`observable`、`vetoable`，以及 map-backed delegate 所需接口。
- 目标：先只固定声明面与最小文档/fixture；行为实现与线程安全语义后续补齐。
- 验收：新增 resolve/typecheck fixture：引用 `scoop.delegates.lazy` 等 API 可通过；缺失导入时报错。
- 依赖：T0451、T1210
 - 完成：
   - `sysroot/delegates.scoop`：为 `lazy` 补齐 `lazy(mode, initializer)` 重载（匹配 spec §10.4 示例）。
   - `crates/scoopc/src/typecheck/properties.rs`：允许同一 overload set 在“返回名义类型一致”时仍可推导 delegate nominal type，避免标准 delegates 因重载而跳过 `getValue/setValue` 签名检查。
   - `tests/fixtures/typecheck/delegated_property_lazy_with_mode_ok.scoop`：新增 typecheck fixture 覆盖 `lazy(LazyThreadSafetyMode.None) { ... }`。
   - `tests/fixtures/resolve/delegates_lazy_missing_import_is_error.scoop`：新增 resolve fixture：未导入 `scoop.delegates.*` 时引用 `lazy` 报 `unresolved_value`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1218 [DONE] 编译期注解访问：复杂参数表达式 / 数组 / enum / class-literal
- 描述：在 comptime/reflection API 中补齐对复杂注解参数的读取：不仅能拿到字面量，还能读取常量表达式求值结果、数组参数、enum 值与类字面量。
- 目标：读取结果应与 T1019 的注解参数语义保持一致；不暴露未归一化的 parser 细节。
- 验收：新增 comptime fixtures：读取 `@Anno(1 + 2, [Color.Red], String::class)` 得到稳定元数据；输出与下游 `.cone` 元数据一致。
- 依赖：T1019、T1209、T1215
 - 完成：
   - `crates/scoopc/src/comptime/eval.rs`：
     - const eval 支持 `TypeName::class`（v0：返回类型名字符串常量）；
     - enum unit variant 常量识别升级为支持 `pkg.Enum.Variant` 形式，并避免在路径首段可解析为运行期值时误判。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测 `const_eval_reflection_annotations_of_v0_complex_args` 覆盖 `1 + 2` / `[Color.Red]` / `Color.Red` / `String::class` 四类参数读取。
   - `tests/fixtures/comptime/annotation_access_v0_complex_args.*`：新增 comptime fixture + `.comptime` golden 回归覆盖复杂注解参数读取。
   - `sysroot/core.scoop`、`crates/scoopc/src/comptime/interpreter.rs`：更新注释，明确 v0 可读的 compile-time constant 注解参数形态。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1219 [DONE] 平台反射：`Platform` struct + `getPlatform(): Platform`（spec §6.4）
- 描述：在 sysroot 中新增 `Platform` 值类型，并提供 `const fun getPlatform(): Platform`：
  - comptime 下返回编译 target 的 LLVM triple 与拆分字段；
  - runtime 下返回当前执行环境平台（早期阶段可先等同于编译 target）。
- 目标：
  - 先只覆盖 host target（与 T0803 一致）；交叉编译后续再扩展；
  - 字段拆分规则以 LLVM triple 为准，无法解析时给出稳定错误或空字段策略（需规范化）。
- 验收：
  - 新增 comptime fixture：`const val p = getPlatform(); p.arch/p.os` 可被读取并输出稳定（可用 `.comptime` golden）；
  - 新增 typecheck fixture：运行期语境调用 `getPlatform()` 可通过（遵循 const fun fallback 规则）。
- 依赖：T0803、T1202、T1204
 - 完成：
   - `sysroot/core.scoop`：新增 `Platform` 值类型与 `@Intrinsic const fun getPlatform(): Platform` 声明。
   - `crates/scoopc/src/comptime/interpreter.rs`：为 const 解释器内建实现 `getPlatform()`（v0：基于 Cargo cfg 构造 host target triple，并按 `arch-vendor-os-env` 拆分）。
   - `crates/scoopc/src/typecheck/expr.rs`：补齐 sysroot 跨文件特判：`Platform.*` 字段成员访问返回 `String`（避免 `struct_field_types` 仅收集当前文件导致 runtime 使用被拒）。
   - `crates/scoopc/src/comptime/tests.rs`：新增单测 `const_eval_get_platform_intrinsic_v0` 覆盖 triple 与拆分字段一致性。
   - `tests/fixtures/comptime/get_platform_v0_basic.*`：新增 comptime fixture + `.comptime` golden（跨平台稳定：只断言字段可访问与非空性）。
   - `tests/fixtures/typecheck/get_platform_runtime_ok.scoop`：新增 typecheck fixture：运行期语境调用 `getPlatform()` 并读取 `p.os` 可通过。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

---

## T13：Kotlin 语义兼容项（阶段 11+，按需补齐）

### T1301 [DONE] 操作符重载：解析 `a + b` 到约定方法名（Appendix B.8 / PLAN §14）
- 描述：在 typecheck/bind 阶段把二元运算映射到 `plus/minus/...`。
- 目标：先只实现 `+` 与 `-`；只对 struct/class 方法生效。
- 验收：language fixture：自定义 `plus` 后 `a + b` 通过；缺少方法时报错。
- 依赖：T0211、T0302、T0407
 - 完成：
   - `crates/scoopc/src/typecheck/expr.rs`：对 `BinaryOp::Add/Sub` 增加 operator overloading：当 lhs 为 struct/class 时，尝试绑定到 `{Type}.plus/minus` 成员方法；同时保留整数 `+/-` 的内建规则；新增稳定诊断 `scoop::typecheck::operator_overload_not_found`。
   - `tests/fixtures/typecheck/operator_overload_plus_minus_ok.scoop`：新增通过用例覆盖 `a + b`/`a - b`。
   - `tests/fixtures/typecheck/operator_overload_plus_missing_is_error.scoop`：新增失败用例覆盖缺少 `plus` 方法时报错。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1302 [DONE] typealias：完善语义（泛型 alias / 跨包导出 / Cone 交互）（Appendix B.10）
- 描述：在已有“最小 typealias 展开”（见 T0446）的基础上，补齐 Kotlin 风格的 typealias 能力：支持泛型 typealias、跨包引用与可见性/导出规则（与 Cone 的 public API 对齐）。
- 目标：不改变既有别名展开语义；重点解决：泛型参数作用域、跨包循环检测、以及在 `.cone` 导出时的表现形式（展开还是保留 alias，需固定策略）。
- 验收：typecheck + cone fixtures：A 包导出 `typealias`，B 包依赖并使用；泛型 alias 可被正确实例化；非法循环在跨包时仍能报错。
- 依赖：T0446、T0218、T1105
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：`TypeAliasDecl` 增加 `type_params`，允许 `typealias Name<T> = ...`。
   - `crates/scoopc/src/parser/decls.rs`：typealias 支持解析泛型参数列表；并显式拒绝 `eff` 参数（保持阶段边界）。
   - `crates/scoopc/src/resolve/mod.rs`：resolver headers phase 为 typealias 建立声明级 type param scope，使 RHS 中 `T` 可解析。
   - `crates/scoopc/src/typecheck/type_env.rs`：TypeEnv 收集 typealias 的 type params（arity + variances）。
   - `crates/scoopc/src/typecheck/lower.rs`：
     - typealias 展开支持泛型实例化：在 decl 文件上下文中用 `type_args` 绑定 alias 的 type params；
     - cache key 升级为 `(alias fqn, type args)`，避免不同实例互相污染；
     - 展开时不捕获 use-site 的 type/effect param scopes；
     - 对 `.cone` 注入的合成 source：缺省使用空 package/import 上下文，保证 RHS 可展开。
   - `crates/scoopc/src/cone/scoopir/schema.rs`：ScoopIR `IrTypeDecl` 为 `typealias` 增加可选 `alias_of`（RHS）。
   - `crates/scoopc/src/cone/scoopir/export.rs`：导出 public typealias 的 RHS（按 typecheck lowering 规范化），并校验 RHS 不得暴露非 public 类型。
   - `crates/scoopc/src/cone/consume.rs`：注入依赖 `.cone` 的 typealias RHS 到 TypeEnv，供下游展开。
   - fixtures：
     - `tests/fixtures/typecheck/typealias_generic_handler_ok.scoop`：泛型 typealias 在同文件通过。
     - `tests/fixtures/typecheck_multi/typealias_cross_package_cycle/*`：跨包循环别名报 `cyclic_type_alias`。
     - `tests/fixtures/typecheck_cone_archive/typealias_export_generic/*`：真实 `.cone` 依赖下导出/消费 typealias（含泛型）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1303 [DONE] object / companion object（Appendix B.9）
- 描述：按需支持 singleton 对象声明与伴生对象。
- 目标：先只实现语法与 name resolution；codegen 可后置。
- 验收：parse+resolve fixture：object 声明可解析；引用解析正确。
- 依赖：T0201、T0301
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `ObjectDecl/ObjectKind`，并纳入顶层 `Item` 与 `TypeMember`。
   - `crates/scoopc/src/parser/file.rs`：支持顶层 `object Name { ... }` 解析。
   - `crates/scoopc/src/parser/decls.rs`：支持 `object Name { ... }` 与 `companion object [Name] { ... }`（并限制 `companion object` 只能出现在 class body）。
   - `crates/scoopc/src/resolve/mod.rs`：object 同时进入 type/value 命名空间；未命名 companion object 使用隐式名 `Companion` 建立索引；并记录 class→companion object 关系用于后续解析。
   - fixtures：
     - `tests/fixtures/parse/object_and_companion_object_basic.scoop`（含 AST golden）
     - `tests/fixtures/resolve/object_member_access_ok.scoop`
     - `tests/fixtures/resolve/companion_member_access_via_class_name_ok.scoop`
     - `tests/fixtures/resolve/missing_companion_object_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1304 [DONE] 范围与 for 协议（Appendix B.12）
- 描述：实现 `for (x in range)` 语法与最小 typecheck，并把循环语义降糖到“迭代协议”。
- 目标：先只做语法 + typecheck 规则；运行时库后续。
- 验收：language fixture：for 语法可解析并类型检查；缺少迭代协议方法时报错。
- 依赖：T0207、T0407
 - 完成：
   - `crates/scoopc/src/ast/mod.rs`：新增 `ForStmt` 与 `StmtKind::For`。
   - `crates/scoopc/src/parser/stmt.rs`：支持解析 `for (x in xs) { ... }`。
   - `crates/scoopc/src/parser/cursor.rs`：把 `for` 纳入 `is_stmt_start`（用于错误恢复与 `return` 边界）。
   - `crates/scoopc/src/resolve/scopes.rs`：for binder 仅在 loop body 作用域内可见，并写回 `ResolvedValueRef::Local`。
   - `crates/scoopc/src/typecheck/expr.rs`：实现迭代协议类型检查（v0：`iterator/hasNext/next`）+ 稳定诊断码，并将隐式调用的 effects 计入 performed effects。
   - `crates/scoopc/src/typecheck/properties.rs`：把 `for` 纳入 backing-field 逃逸检查的语句遍历。
   - `crates/scoopc/src/comptime/interpreter.rs`：const eval 中将 `for` 视为不支持语句（与 `while` 对齐）。
   - `crates/scoopc/src/hir/lower.rs`：HIR lowering 暂以 `Todo("for")` 占位（语义 lowering 留给后续 codegen 任务）。
   - fixtures：
     - `tests/fixtures/typecheck/for_loop_iter_protocol_ok.scoop`
     - `tests/fixtures/typecheck/for_loop_missing_iterator_is_error.scoop`
     - `tests/fixtures/typecheck/for_loop_missing_has_next_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

> 注：规范已升级迭代协议为 `Iterator.next(): Option<T>`（见 `SCOOP_FULL_SPEC.md` §16.2），不再使用 `hasNext()/next()` 二元协议；迁移任务见 T1328。

### T1305 [DONE] 默认参数语义：调用点补齐默认值（Appendix B.5.2）
- 描述：在类型检查/调用解析阶段实现默认参数：调用时省略参数自动补齐默认值表达式。
- 目标：先只支持尾部参数默认值；中间省略与命名参数结合后续。
- 验收：typecheck+run-pass fixture：`fun f(x:Int=1,y:Int=2)` 调用 `f()` 输出 3（或等价行为）。
- 依赖：T0230、T0810
 - 完成：
   - `crates/scoopc/src/hir/lower.rs`：在 HIR lowering 阶段对“少传尾部默认参数”的顶层函数调用做语义补齐；把调用点改写为 block（按参数名绑定实参/默认值为局部 `val`，再调用完整参数形态），保证默认值可引用更早参数且实参不会被重复求值。
   - `crates/scoopc/src/typecheck/expr.rs`：把函数形参默认值表达式纳入 typecheck（按形参类型做 expected-context 推断 + 可赋值检查），并新增稳定诊断 `default_param_value_type_mismatch`。
   - fixtures：
     - `tests/fixtures/run-pass/default_param_call_site_fill_basic.scoop`（stdout golden：`3`）
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1306 [DONE] 命名参数语义：重排与混用规则（Appendix B.5.3）
- 描述：实现命名参数：按参数名匹配并重排；禁止与位置参数混用的非法形式（按 Kotlin 规则）。
- 目标：先不支持 varargs 与命名参数组合。
- 验收：typecheck fixture：`f(y=2,x=1)` 通过；重复命名/不存在参数名报错并指向 name span。
- 依赖：T0231、T1305
 - 完成：
   - `crates/scoopc/src/typecheck/expr.rs`：在函数/构造/扩展调用的 typecheck 路径中加入 Kotlin-like 命名参数规则检查：
     - 命名参数之后禁止再出现位置参数；
     - 同名命名参数禁止重复；
     - 对重载候选集合预检查“命名参数名是否存在于任一候选签名”，并在 name span 上报稳定诊断。
   - `crates/scoopc/src/typecheck/expr.rs`：更新形参映射逻辑为“位置参数按序绑定 + 命名参数按 name 重排”，不再允许在命名参数存在时通过位置参数“跳槽”。
   - fixtures：
     - `tests/fixtures/typecheck/call_named_args_reorder_ok.scoop`
     - `tests/fixtures/typecheck/call_named_args_duplicate_is_error.scoop`
     - `tests/fixtures/typecheck/call_named_args_unknown_is_error.scoop`
     - `tests/fixtures/typecheck/call_named_args_positional_after_named_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1307（拆分为子任务）
- 描述：当 call 最后一个参数是 lambda 时，支持 `f { ... }`/`f(1) { ... }` 并正确进行 lambda 参数下推推断。
- 目标：保持“可单独实现 & 单独验证”的小步：先补齐前端推断与 `it` 语义，再补齐可执行的 run-pass 回归。
- 备注：当前 LLVM `main` codegen 仍未支持 `ExprKind::Closure` 与“调用局部函数值”，因此 run-pass 需要后置到后端接入（见 `crates/scoopc/src/llvm/codegen.rs`）。
- 依赖：T0232、T0504

### T1307a [DONE] trailing lambda：隐式 `it` + 期望类型下推（Appendix B.5.4）
- 描述：在 `{ body }` 形式 lambda 中引入 Kotlin-like 隐式单参数 `it`：当期望函数类型为 `(T) -> R` 时，`takes { it }` 可推断通过。
- 目标：只改 resolver/typecheck；不要求后端可执行。
- 验收：infer fixture：`takes { it }` 推断成功（`it` 类型由期望类型下推）。
- 依赖：T0232、T0504
 - 完成：
   - `crates/scoopc/src/resolve/scopes.rs`：对 `{ body }` lambda 预注入隐式 `it` 局部绑定，并让 scope 声明使用 `Ident::text(..)` 以支持合成标识符。
   - `crates/scoopc/src/typecheck/expr.rs`：当期望函数类型为单参数时，把 `{ body }` 视作“隐式 `it` 单参 lambda”，并在 lambda locals 中注入 `it: T` 完成 body typecheck。
   - fixtures：新增 `tests/fixtures/infer/lambda_implicit_it_param_type_is_propagated_from_expected.scoop` 回归用例。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1308 [DONE] varargs：`vararg` 参数与 spread（Appendix B.5.5）
- 描述：解析/类型检查 `vararg` 参数，并支持调用点 `*arr` spread（若语言采用同语法）。
- 目标：先只支持数组/tuple 的最小 spread；集合转换后置。
- 验收：typecheck fixture：vararg 调用通过；不支持 spread 的类型时报错。
- 依赖：T0218、T0407
 - 完成：
   - lexer：新增 `vararg` 关键字（`Keyword::Vararg`）。
   - AST：`Param` 增加 `is_vararg`；调用实参增加 `ExprKind::SpreadArg`（`*expr`）。
   - parser：函数/构造参数列表支持 `vararg`；调用实参支持 `*expr` spread（含命名实参 value）。
   - typecheck：`vararg` 形参映射支持 0..N 实参；spread 支持 `Array<T>`/tuple（元素逐一做可赋值检查）。
   - headers：`vararg` 只能出现一次且必须为最后一个形参（最小门禁）。
   - fixtures：
     - `tests/fixtures/typecheck/vararg_call_ok.scoop`
     - `tests/fixtures/typecheck/vararg_spread_non_array_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1309 [DONE] 操作符重载：补齐位运算/移位映射（`& | ^ ~ << >>` → `and/or/xor/inv/shl/shr`）（Appendix B.8）
- 描述：在已实现的操作符重载绑定机制上，加入位运算与移位的 operator→方法名映射，并对 unary `~` 映射到 `inv()`。
- 目标：只做“绑定规则”补齐；不引入新的优先级（优先级在 parser 已固定）；不处理复合赋值（如 `shlAssign`）除非 spec 明确要求。
- 验收：language fixture：自定义 `and`/`shl`/`inv` 后 `a & b`/`a << 1`/`~a` 可通过；缺少方法时报错并指向操作符。
- 依赖：T1301、T0211、T0252
 - 完成：
   - `crates/scoopc/src/typecheck/expr.rs`：
     - 扩展二元操作符重载映射：`& | ^ << >>` → `and/or/xor/shl/shr`；
     - 补齐 unary `~` 的重载：对 struct/class 绑定到 `inv()`；并新增稳定诊断 `scoop::typecheck::unary_operator_overload_not_found`。
   - fixtures：
     - `tests/fixtures/typecheck/operator_overload_bitwise_shift_inv_ok.scoop`：覆盖 `& | ^ << >> ~` 的通过用例；
     - `tests/fixtures/typecheck/operator_overload_bitwise_and_missing_is_error.scoop`：缺少 `and` 方法时报错；
     - `tests/fixtures/typecheck/operator_overload_bitwise_inv_missing_is_error.scoop`：缺少 `inv` 方法时报错。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1310 [DONE] import alias：`import foo.bar.Baz as Qux`（Appendix B.7）
- 描述：在 Kotlin 兼容层补齐 alias import 的完整语义：可见性、shadowing、与普通 import / wildcard import 的交互。
- 目标：在 parser/resolve 子任务基础上补齐语言级规则；不改变既有 import 语义。
- 验收：language fixture：alias import 与普通 import 混用时解析正确；冲突时报清晰诊断。
- 依赖：T0254、T0315
 - 完成：
   - `crates/scoopc/src/resolve/imports.rs`：允许 alias 与同文件顶层声明同名（由后续 shadowing 规则处理），并更新单测覆盖。
   - `crates/scoopc/src/resolve/scopes.rs` / `crates/scoopc/src/resolve/mod.rs`：修正单段名字解析优先级：同包（含 root）> 显式 import（含 alias）> star import，避免解析结果依赖 fqn 字典序。
   - fixtures：
     - `tests/fixtures/resolve/import_alias_can_be_shadowed_by_local_decl_ok.scoop`
     - `tests/fixtures/resolve_multi/import_alias_precedence_star/**`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1311 [DONE] `object` / `companion object`：补齐类型检查、静态访问与初始化语义（Appendix B.9）
- 描述：在已有 parse/resolve 任务基础上，补齐 `object` / `companion object` 的语言级行为：单例语义、通过类名访问 companion 成员、初始化时机与可见性。
- 目标：不修改既有 T1303；本任务专门把它从“语法/解析”推进到“完整语言语义”。
- 验收：language fixture：top-level object、nested object、named/unnamed companion object 的成员访问和初始化行为符合预期。
- 依赖：T1303、T0452、T0828
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：
     - 支持 object/companion object 的“单例值”在表达式位置被引用（`val a = Foo` / `val b = C.Named`）；
     - 读取单例值会触发 once 初始化（复用既有 `__scoop_object_init__*`）；
     - 为单例值引入 module-local 的稳定“实例指针”全局（`__scoop_object_instance__*`），用于提供唯一身份。
   - `crates/scoopc/src/resolve/scopes.rs`：`TypeName.member` 经 companion object 分发时，把 companion 自身可见性作为访问上界，避免 “private companion 但成员 public” 绕过。
   - fixtures：
     - `tests/fixtures/run-pass/object_companion_value_named_nested_init_basic.*`：覆盖 object 单例值引用、nested object、named companion 多路径访问共享 once 初始化；
     - `tests/fixtures/typecheck_multi/companion_visibility_private_companion/*`：跨文件回归 companion 可见性上界（`C.x` 应报 `not_visible`）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1312 [DONE] 类初始化语义：property initializer / `init` / secondary constructor 顺序（Appendix B.2.2）
- 描述：补齐 Kotlin-like 类初始化顺序与规则：属性初始化、多个 `init` block、secondary constructor body 的执行顺序和可见性边界。
- 目标：先覆盖单类/单继承常见情况；复杂继承链细节后续扩展。
- 验收：language fixture：初始化顺序输出稳定；非法在初始化早期访问未就绪成员时报错。
- 依赖：T0256、T0257、T0448
 - 完成：
   - `crates/scoopc/src/hir/mod.rs`：引入 class 初始化 side table（`ClassInitIndex` / `ClassInitStep` / `ClassCtor` 等）与 ctor call-site candidates 索引（`CtorCallSiteIndex`），不影响 `dump-hir` 输出稳定性。
   - `crates/scoopc/src/hir/lower.rs`：收集 class 字段/初始化步骤/构造器，并在 lowering 时记录 ctor 调用点候选集合供 LLVM codegen 使用。
   - `crates/scoopc/src/resolve/scopes.rs`：初始化阶段的 forward-reference 可见集合预置 primary ctor `val/var` 参数属性，允许在 class init 期通过 `this.x` 访问它们。
   - `crates/scoopc/src/llvm/codegen.rs`：
     - 支持 class ctor call：分配对象、按 Kotlin-like 顺序执行 property initializer + `init {}` blocks，并在选择 secondary ctor 时最后执行其 body；
     - 支持 class 实例字段读取与写入（`this.x` / `obj.x` 以及 `this.x = ...`）。
   - `crates/scoopc/src/llvm/mod.rs`：把 `class_inits` / `ctor_call_sites` 接线传入 `MainCodegen`。
   - fixtures：
     - `tests/fixtures/run-pass/class_init_order_primary_secondary_basic.*`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1313 [DONE] 标准 delegated properties：`lazy` / `observable` / `vetoable` / map-backed（spec §10.4）
- 描述：在语言兼容层补齐标准 delegated properties 的行为与示例，确保 `by` 不只停留在语法和最小接口层。
- 目标：先覆盖最常见的行为：lazy 首次访问缓存、observable/vetoable 回调、map-backed 属性读取；更复杂线程安全语义后续。
- 验收：language/run-pass fixture：`lazy` 只初始化一次；`observable` / `vetoable` 回调按预期触发；map-backed delegate 可读取字段。
- 依赖：T1217、T0434
 - 完成：
   - `crates/scoopc/src/hir/lower.rs`：
     - 在 lowering 阶段对标准 delegates 做特判（early strategy：避免依赖 `$delegate` 字段、`PropertyMeta` 运行期构造与闭包调用等尚未落地链路）；
     - `lazy`：为属性生成隐藏 `inited/value` 字段，并将 getter lower 成“首次访问初始化一次 + 缓存”；
     - `observable` / `vetoable`：将回调体 inline 到赋值 lowering；vetoable 先判定是否允许赋值，再决定是否写入与触发回调；
     - map-backed（early）：在 class init 阶段把 `by this.data` 读取改写成从 `this.data.p` 拷贝到真实字段 `p`（读取走字段，后续可升级为真实 map lookup）。
   - fixtures：
     - `tests/fixtures/run-pass/delegated_property_lazy_init_once_basic.*`
     - `tests/fixtures/run-pass/delegated_property_observable_vetoable_basic.*`
     - `tests/fixtures/run-pass/delegated_property_map_backed_basic.*`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1314 [DONE] Kotlin runtime / Scoop core runtime gap 审计（when applicable）
- 描述：盘点 Scoop core runtime / stdlib 与 Kotlin runtime 之间“语义上值得补齐、且与 JVM 绑定无关”的缺口，并按“纯 Scoop 可实现 / 需要 runtime libs / 需要新 intrinsic”三类归档。
- 目标：不盲目追求 1:1 复制 Kotlin/JVM runtime；只补对 Scoop 语言模型成立、且对用户价值高的部分。
- 验收：产出一份 capability matrix，列出候选模块、优先级、是否纯 Scoop 可实现，以及是否需要走 T1017/T1018 通道。
- 依赖：T1311、T1312、T1217
 - 完成：
   - `KOTLIN_RUNTIME_GAP_AUDIT.md`：新增 capability matrix（pure_scoop_ok / needs_runtime_lib / needs_new_intrinsic 候选）与对后续 T1315/T1316/T1317/T1017 的落点建议。
 - 验收：
   - `cargo test --all`

### T1307b [DONE] trailing lambda：run-pass（lambda 被调用）回归（Appendix B.5.4）
- 描述：补齐可执行链路：让 `takes { it }` 在运行期能真正调用 lambda 并产生可观测结果（stdout 或 exit code 断言）。
- 目标：先只做最小可执行落点（可先限制为不捕获 lambda）；捕获闭包可复用既有 HIR/MIR 结构（T0710/T0711/T0714）。
- 验收：新增 run-pass fixture：lambda 被正确调用（stdout 或 exit code 断言）。
- 依赖：T1307a、T0710、T0810
 - 完成：
   - `crates/scoopc/src/llvm/codegen.rs`：实现最小函数值运行期表示与调用：
     - lambda 表达式 lowering 为 heap `closure object`（`{ header, env_ptr, fn_ptr }`，当前 env 固定为 NULL）；
     - 支持调用局部函数值（function value / 闭包参数），生成 indirect call；
     - 支持 `{ body }` 形式的隐式 `it`：将 resolver 注入的 `it` 捕获视作单参 lambda 的参数绑定（仍限制为不捕获闭包）。
   - fixtures：新增 `tests/fixtures/run-pass/trailing_lambda_implicit_it_called_basic.*`（stdout：`42`）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1017 [DONE] 后期 runtime/std 的 intrinsic 需求审计（gate task）
- 描述：针对“纯 Scoop 补齐 Kotlin runtime gap 与全量 std”做一次底层 primitive 审计，明确哪些能力可以完全用现有 runtime/API 实现，哪些能力确实缺少 primitive。
- 目标：默认结论应是“无新增 intrinsic”；只有审计证明无法表达时，才允许进入 T1018。
- 验收：输出一份分层清单：`pure_scoop_ok` / `needs_runtime_lib` / `needs_new_intrinsic`；每个 `needs_new_intrinsic` 都必须附带无法用现有机制实现的理由。
- 依赖：T1217、T1314
 - 完成：
   - `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`：新增审计文档，输出分层清单并给出对 T1018/T1315/T1316/T1317 的落点建议（结论：std 主线不需要新增 intrinsic）。
   - `KOTLIN_RUNTIME_GAP_AUDIT.md`：回写 3.3 小节，记录 T1017 结论：数组 buffer/原子/identity hash 等候选项优先转为 runtime lib/ABI，不作为 intrinsic gate 理由。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1315 纯 Scoop 补齐 Kotlin runtime 适用缺口（不新增 intrinsic）（拆分为子任务）
- 描述：根据 T1314 的审计结果，用纯 Scoop 实现可补齐的核心 runtime 库能力，例如文本/集合辅助、ranges/progressions helpers、sequence-like utilities、常见 runtime support APIs 等。
- 目标：默认不得新增 intrinsic；若遇到底层 blocker，必须回流到 T1017/T1018，而不是在本任务里偷偷扩 intrinsic。
- 验收：新增 language/run-pass fixtures：至少一组来自 Kotlin 运行库常见能力的用法可直接在 Scoop 上工作，且实现主体为 Scoop 代码。
- 依赖：T1314、T1017

### T1315a [DONE] build/run：自动注入可编译的纯 Scoop prelude（`stdlib` v0）
- 描述：引入 `stdlib/*.scoop`，并让 `scoop build/run` 以稳定顺序把它们加入当前编译单元，使其顶层函数可被 resolver/typecheck/lowering/codegen 消费。
- 目标：
  - 为后续纯 Scoop stdlib/运行库（库层）helpers/collections/ranges 提供落点；
  - 不改变 sysroot 的“声明源”定位（sysroot 仍只作为 cone 0 的内建声明面）；
  - 在后端尚不支持“跨文件 span → 源文本切片”前，`stdlib v0` 的实现不依赖数值/字符串字面量（避免 codegen 读取错误 SourceFile）。
- 验收：
  - 新增 run-pass fixture：调用 `stdlib` 中实现的 Kotlin-like helper（例如 `require`）并通过 `try/catch` 捕获 `RuntimeError`；
  - `cargo test --all` 通过；
  - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test` 本机通过。
- 依赖：T1314、T1017
 - 完成：
   - `stdlib/prelude.scoop`：新增 Kotlin-like `require/check`（纯 Scoop）。
   - `crates/scoop/src/commands/build.rs`：`scoop build/run` 自动注入 `stdlib/*.scoop` 到编译单元。
   - `crates/scoopc/src/hir/lower.rs`：新增 multi-file lowering 入口 `lower_for_compilation_unit_multi_files`（并限制非入口文件不得含 source-backed literals）。
   - `crates/scoop/src/commands/build.rs`：后端 lowering 改为 multi-file，以便 stdlib 顶层函数进入 `fun_index` 并可被 codegen。
   - fixtures：新增 `tests/fixtures/run-pass/kotlin_require_check_basic.*`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1315b [DONE] pure-scoop：补齐 Kotlin-like core runtime helpers（v1）
- 描述：在 `stdlib` 中补齐更多“高频但不依赖平台”的 runtime support APIs，例如 preconditions（`check/require` 族）、`also/run/let/apply` 的早期可用形态、以及常用小工具函数。
- 目标：优先产出 fixtures 价值高、能显著提升示例可写性的能力；默认不新增 intrinsic。
- 验收：新增 run-pass fixtures：至少覆盖 2 组常用 helper 的可执行语义（stdout/exit code 断言）。
- 依赖：T1315a
 - 完成：
   - `stdlib/prelude.scoop`：
     - 新增 `requireLazy/checkLazy`（lazy message，失败分支求值；当前阶段 message 先用 `() -> Unit` 避免 String/aggregate return 限制）。
     - 新增 Kotlin-like scope functions（spec §11）v1：`Int.{also,let,run,apply}`（当前先做 `Int` 专用落点，避免泛型单态化/跨文件实例生成缺口影响可执行回归）。
   - `sysroot/core.scoop`：补齐 scope functions 的声明表面（与 stdlib v1 对齐：Int-only + effect-polymorphic）。
   - `crates/scoopc/src/typecheck/expr.rs`：修复“sysroot 仅声明 + stdlib 提供实现”导致的重复候选/重载歧义（优先选择 has_body=true 的实现版本）。
   - `crates/scoopc/src/hir/lower.rs`：
     - extension call 降糖：`receiver.ext(args...)` → `ext(receiver, args...)`（便于后端识别为顶层调用）。
     - extension receiver 显式参数化：把 `this` 作为第 0 个参数降入 HIR，避免 codegen 找不到 `this` 局部绑定。
   - fixtures：
     - `tests/fixtures/run-pass/kotlin_require_check_lazy_message_basic.*`
     - `tests/fixtures/run-pass/kotlin_scope_functions_basic.*`
     - `tests/fixtures/run-pass/kotlin_require_check_basic.stdout`：修复 stdout golden 末尾空行，避免 run-pass mismatch。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1315c [DONE] pure-scoop：ranges/progressions 与 sequence-like utilities（v0）
- 描述：基于现有语法/语义，为 `Int` 等标量类型补齐 Kotlin-like ranges/progressions helpers 与最小惰性序列/迭代 utilities（不引入新的集合底座）。
- 目标：先以“可表达 + 可回归”为主，不追求性能最优；不引入新的 intrinsic。
- 验收：新增 run-pass fixtures：覆盖 `range/progression` 的构造与最小迭代行为（stdout/exit code 断言）。
 - 依赖：T1315a
 - 完成：
   - `sysroot/core.scoop`：新增 `IntProgression` 与 `Int.rangeTo/downTo(step)`、`IntProgression.forEach` 的声明表面，供 resolver/typecheck 可见。
   - `stdlib/prelude.scoop`：新增 `Int.rangeTo/downTo(step)` 与 `IntProgression.forEach` 的纯 Scoop 可执行实现（无新增 intrinsic、且不在 stdlib 中引入 Int/String 字面量）。
   - `tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop`：新增 run-pass fixture 覆盖 up/down progression 的构造与最小迭代行为。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`

### T1316 [DONE] 全量 `std` 设计：分层、稳定性、能力矩阵（目标对标 Rust std）
- 描述：设计 Scoop 的标准库分层与包边界，目标是能力与 Rust `std` 同量级、可比较，但不要求 API 相同。建议至少区分 `core` / `alloc` / `std` / 平台适配层。
- 目标：固定模块边界、稳定性策略、目标平台 capability matrix（desktop / server / embedded / wasm）。
- 验收：产出 `std` 模块树与 capability matrix；说明各模块依赖于哪些 runtime / platform backends。
- 依赖：T1314、T1017
 - 完成：
   - `STDLIB_DESIGN.md`：新增 `std` 分层（core/alloc/std/platform）、推荐模块树与 capability matrix，并明确各模块对 runtime/platform backends 的依赖边界。
   - `README.md`：补充链接，便于从项目入口快速定位 std 设计文档。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1317 [DONE] `std` v1：collections / iterators / text / algorithms（已拆分为子任务：T1317a~T1317f）
- 描述：实现全量 `std` 的第一层基础模块，重点先把 **collections 与 iterators** 的核心形态固定下来：以 `Array<T>` / `MutableArray<T>` 为唯一集合底座，并在其上用纯 Scoop 构建 `List/Set/Map` 等。
- 目标：
  - **最小 intrinsics**：除 `Array/MutableArray` 的必要底层 primitive 外，`List/Set/Map`（含 mutable）不新增 intrinsic；除“绝对必要”的底层 primitive 外，其它方法与操作都用 Scoop 实现（保持语义一致、便于维护）。
  - `Array<T>`（不可变）：
    - 只读集合：支持 `get`、`length/size`、迭代与常用数组操作；
    - 支持从 iterable 构造（优先 `Array.from(iterable)`）。
  - `MutableArray<T>`（可变）：
    - 支持 `get/set/push/pop/insert/remove/splice` 等基础操作；
    - 以容量策略（capacity growth）保证 `push/pop/insert/remove` 的摊还 O(1)（允许扩容/搬移导致的偶发 O(n)）；
    - 如确需新增 intrinsic，应仅限于 buffer 分配/搬移/容量查询等底层 primitive；`push/pop/insert/remove/splice` 的策略与边界条件尽量由纯 Scoop 库代码实现。
  - 数组字面量 `[...]`：
    - 按 expected type 推断为 `Array<T>` 或 `MutableArray<T>`；
    - 支持显式类型注解：`val xs: Array<Int> = [1, 2, 3]`（以及 `val ys: MutableArray<Int> = [1, 2, 3]`）。
    - 元素类型 `T` 为所有元素表达式类型的 LUB（最小公共父类型）；空数组 `[]` 必须依赖 expected type 才能确定 `T`（否则报错）。
    - **禁止隐式逐元素装箱**：array literal 不会因为 expected element type 是 `Any` 就自动把 `Int` 等值类型装箱；需要显式写 `as Any`（例如 `[1 as Any, 2 as Any]`）。形如 `[1, 2 as Any]` 必须报错。
  - iterable 构造与 `MutableArray -> Array`（是否暴露转换 API）：
    - 允许实现上用内部 builder（例如内部 `MutableArray`）做增量构造；
    - 若需要“零拷贝把 builder 变成 `Array`”，必须先定义**显式且安全**的语义（例如 `freeze`：冻结后任何别名都不可再变更）；
    - 在缺少上述语义前，不对外暴露 `MutableArray -> Array` 的零拷贝转换 API（必要时仅 `internal` 使用）。
  - `List<T>`：定义为 `Array<T>` 的别名：`typealias List<T> = Array<T>`。
  - `Hashable`：新增 `Hashable` 接口，并为 primitive types 提供实现（用于 `Set/Map` 的键约束）。
  - `Set<T: Hashable>` / `Map<K: Hashable, V>`：基于 `Array` 的纯 Scoop 实现（不新增 intrinsics）。
  - `MutableSet<T: Hashable>` / `MutableMap<K: Hashable, V>`：基于 `MutableArray` 的纯 Scoop 实现（不新增 intrinsics）。
  - `MutableList<T>`：用 `MutableArray` 做 backing pool，以纯 Scoop 实现全部方法，并保证 `push/pop/insert/remove` 摊还 O(1)。
- 验收：新增 std fixtures（compile + run-pass）覆盖：
  - `Array`/`MutableArray` 的 immutability/mutability 与 `get/set/push/pop/insert/remove/splice` 行为；
  - `[...]` 字面量的 expected-type 推断与类型注解；
  - 迭代（`for` 协议）与从 iterable 构造；
  - `Hashable` 在 primitive 上可用，且 `Set/Map`（含 mutable）在“无新增 intrinsics”的前提下可运行。
- 依赖：T1316、T1315

> 备注：该任务范围过大。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，已拆分为以下子任务；本条仅保留原始目标汇总。

### T1317a [DONE] typecheck：数组字面量按 expected type 推断为 `Array<T>` / `MutableArray<T>`
- 描述：补齐 sysroot `MutableArray<T>` 声明，并在表达式 typecheck 中支持 `[...]` 在**期望类型语境**下推断为 `Array<T>` 或 `MutableArray<T>`。
- 目标：仅覆盖 sysroot 声明面 + typecheck；不引入 HIR lowering/codegen/runtime；不要求 `val xs = [...]`（无类型注解）可推断通过。
- 验收：新增 typecheck fixture 覆盖：
  - `val xs: Array<Int> = [1, 2, 3]` / `val ys: MutableArray<Int> = [1, 2, 3]`
  - `takesArray([1, 2])` 这类“实参位置的期望类型推断”
  - `[]` 空数组字面量在期望类型语境可用
  - 回归：`cargo test --all`、`cargo run -p scoop -- test`
- 依赖：T1316、T1315
 - 完成：
   - `sysroot/core.scoop`：新增 `class MutableArray<T>` 声明面。
   - `crates/scoopc/src/typecheck/expr.rs`：在 expected type 语境下为 `[...]` 增加 `Array<T>`/`MutableArray<T>` 推断与元素可赋值检查。
   - fixtures：新增 `tests/fixtures/typecheck/array_lit_expected_type_array_and_mutable_array_ok.scoop` 覆盖 initializer 与 call args 两类位置。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1317b [DONE] sysroot/stdlib：补齐 `Array`/`MutableArray` API surface（只声明）
- 描述：在 sysroot/stdlib 中补齐 `Array<T>`/`MutableArray<T>` 的最小公开 API 声明（`get/size` 与 `set` 等），为后续 lowering/codegen 统一接口落点。
- 目标：只做“类型与签名表面”；不实现运行期语义。
- 验收：新增 typecheck fixtures：对最小 API 的引用可解析、可类型检查（不要求 run-pass）。
- 依赖：T1317a
 - 完成：
   - `sysroot/core.scoop`：为 `Array<T>`/`MutableArray<T>` 增加 `size/get`（以及 `set`）的最小声明面（以 extension fun 形式声明）。
   - fixtures：新增 `tests/fixtures/typecheck/array_mutable_array_min_api_surface_ok.scoop` 覆盖 `size/get/set` 的解析与类型检查。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1317c [DONE] HIR：数组字面量 lowering（`[...]` → builder/intrinsics 调用）
- 描述：为 `[...]` 增加 HIR lowering，把语法糖降到统一的 builder/intrinsic 调用形态，便于后端接入。
- 目标：只落地 lowering 与 `.hir` golden 回归；不要求可执行。
- 验收：新增 `tests/fixtures/hir/*` golden 覆盖 `Array`/`MutableArray` 的 `[...]` lowering 输出稳定。
- 依赖：T1317b
 - 完成：
   - `crates/scoopc/src/hir/lower.rs`：为 `ExprKind::ArrayLit` 增加 expected-type hint（Array vs MutableArray），并 lowering 为统一的 builder/intrinsics block 调用形态。
   - fixtures：新增 `tests/fixtures/hir/array_lit_lowering.scoop` + `tests/fixtures/hir/array_lit_lowering.hir`，覆盖：
     - `val xs: Array<Int> = [...]`
     - `val ys: MutableArray<Int> = [...]`
     - `takesArray([...])` / `takesMutableArray([...])`
     - `[]` 空数组字面量在两类期望类型下的 lowering
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1317d [DONE] runtime/codegen：`Array`/`MutableArray` 最小运行期 primitive（alloc/len/get/set）
- 描述：实现 `Array/MutableArray` 的最小运行期表示与必要 intrinsic（分配、长度、读写），并让 LLVM codegen 能生成可执行代码。
- 目标：intrinsic 只做 buffer 分配/搬移/长度查询等底层 primitive；策略与边界条件尽量留给 stdlib 纯 Scoop 实现。
- 验收：新增 run-pass fixtures：覆盖 `get/set/size` 的最小可观测语义；`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：T1317c、T1017
 - 完成：
   - `runtime/c/scoop_array.c`：新增 Array/MutableArray 的最小运行期实现（word buffer）与 array literal builder（`scoop_array_builder_*`），并提供 `scoop_array_len/get_u64/set_u64` primitive。
   - `crates/scoop_runtime/build.rs`：编译并跟踪 `runtime/c/scoop_array.c`。
   - `crates/scoopc/src/llvm/codegen.rs`：为 sysroot `size/get/set` 与 `__scoop_array_builder_*` 增加 codegen 映射，调用 runtime C ABI 并在 `get` 时按 receiver 的 `Array<T>`/`MutableArray<T>` type args 解码 u64 word。
   - fixtures：新增 `tests/fixtures/run-pass/array_mutable_array_min_primitive_basic.*` 回归覆盖空数组、`size/get/set` 行为。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`

### T1317e [DONE] `std`：`MutableArray` 容量策略 + `push/pop/insert/remove/splice`
- 描述：在 `stdlib` 中用纯 Scoop 实现 `MutableArray<T>` 的扩容/搬移策略与常用操作，并保证摊还复杂度。
- 目标：不新增 intrinsic；仅调用 T1317d 提供的底层 primitive。
 - 验收：新增 run-pass fixtures：覆盖 `push/pop/insert/remove/splice` 的边界条件与 stdout 断言。
 - 依赖：T1317d
 - 完成：
   - `stdlib/mutable_array.scoop`：新增 `MutableArray<Int>` 扩展 `push/pop/insert/removeAt/splice`（early stage 先对 `Int` 提供落点；不新增 intrinsic；通过 `__scoop_array_builder_*` 做分配与搬移；并在 stdlib 内避免 source-backed 字面量）。
   - `crates/scoopc/src/llvm/codegen.rs`：LLVM codegen 允许顶层函数 body 使用 `while` 语句（仍禁止 `break/continue`，保持早期子集可回归）。
   - `tests/fixtures/run-pass/mutable_array_ops_basic.*`：新增 run-pass fixture 覆盖空数组 `pop`、插入/删除 index clamp、`splice` 删除裁剪与 stdout golden。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop --features llvm -- test`

### T1317f `std`：`Hashable` + `List/Set/Map`（含 mutable）与迭代/算法 v0（拆分为子任务）
- 描述：引入 `Hashable` 接口并为 primitive types 提供实现；在 `Array/MutableArray` 之上用纯 Scoop 实现 `List/MutableList`、`Set/Map`（含 mutable）与最小迭代/算法（`forEach`/`map`/`filter`/`fold` 等）。
- 目标：不新增 intrinsic；以语义正确与可回归为主，性能优化后置。
- 验收：新增 compile+run-pass fixtures：`Hashable` 可用、`Set/Map` 可运行、迭代与最小算法在 host 平台可回归。
- 依赖：T1317e

> 备注：该任务包含多个彼此相对独立、且“可单独实现 & 单独验证”的能力点。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，拆分为以下子任务；本条仅保留目标汇总。

### T1317f1 [DONE] sysroot/typecheck：引入 `Hashable` 并让 primitive types 满足 `where` 约束
- 描述：在 sysroot 增加 `Hashable` 接口，并让 `Int/Bool/...` 等内建类型声明实现该接口；新增 typecheck fixtures 回归 `where T: Hashable` 的满足性与失败诊断。
- 目标：只保证“约束可被 typecheck 验证”，不要求本阶段可调用 interface member method（当前成员调用仍受限）；不引入新的 intrinsic。
- 验收：
  - 新增 typecheck fixtures：`Int` 能满足 `where T: Hashable`；自定义 `struct` 未实现时触发 `where_constraint_not_satisfied`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T1317e
 - 完成：
   - `sysroot/core.scoop`：新增 `Hashable` 接口（提供默认实现以避免 early-stage 强制补齐 member），并让 `Bool/Int/UInt/Int8.../UInt64/String` 等内建类型声明实现 `Hashable`。
   - `crates/scoopc/src/typecheck/assignable.rs`：允许 builtin 标量（`Int/Bool/...`）在目标为 interface 时按 sysroot FQN 走最小上转（boxing）规则，使 `where T: Hashable` 对 primitive 生效。
   - fixtures：新增 `tests/fixtures/typecheck/hashable_where_constraint_ok.scoop` 与 `tests/fixtures/typecheck/hashable_where_constraint_not_satisfied_is_error.scoop` 覆盖 pass/fail。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1317f2 [DONE] sysroot/stdlib：`List<T>`/`MutableList<T>` 最小表面（v0）
- 描述：在 sysroot 声明 `typealias List<T> = Array<T>` 与 `class MutableList<T>`（或 typealias）等最小表面，并在 stdlib 基于 `MutableArray` 给出 early-stage 实现落点。
- 目标：先以“可表达 + 可类型检查”为主；实现可先仅覆盖 `Int` 专用版本；不新增 intrinsic。
- 验收：新增 typecheck +（可选）run-pass fixtures 覆盖 `List/MutableList` 的构造与最小操作。
- 依赖：T1317f1、T1317e
 - 完成：
   - `sysroot/core.scoop`：新增 `typealias List<T> = Array<T>` 与 `typealias MutableList<T> = MutableArray<T>`。
   - `stdlib/mutable_list.scoop`：新增 `MutableList<Int>.add`（v0：Int 专用；内部复用 `MutableArray<Int>.push`）。
   - `crates/scoopc/src/resolve/scopes.rs`：extension receiver 匹配对 `List/MutableList` 做最小归一化（映射到 `Array/MutableArray`），避免 `xs: List<Int>` 无法调用 `size/get`、`ys: MutableList<Int>` 无法调用 `push/pop/...`。
   - `crates/scoopc/src/hir/lower.rs`：数组字面量 expected-type hint 支持 `List/MutableList`（按别名映射到 `Array/MutableArray`）。
   - `crates/scoopc/src/llvm/codegen.rs`：array element 类型推断把 `List/MutableList` 视为 array-like，避免 `print/println` 的 `String` expected context 导致 `get` 误解码。
   - fixtures：
     - `tests/fixtures/typecheck/list_and_mutable_list_alias_ok.scoop`
     - `tests/fixtures/run-pass/list_and_mutable_list_basic.*`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1317f3 [DONE] stdlib：迭代/算法 v0（`forEach`/`map`/`filter`/`fold`）
- 描述：在 `Array/MutableArray/List/MutableList` 上提供最小迭代/算法 API，优先覆盖 `Int` 版本，并保持 effect-polymorphic 形态。
- 目标：不追求性能；不新增 intrinsic；保证 stdout/exit 码可回归。
- 验收：新增 run-pass fixtures：`forEach/map/filter/fold` 的行为正确；`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：T1317f2
 - 完成：
   - `stdlib/array_iter.scoop`：为 `Array<Int>`/`List<Int>` 提供 `forEach/map/filter/fold`（`<eff E = Pure>`）。
   - `stdlib/mutable_array_iter.scoop`：为 `MutableArray<Int>`/`MutableList<Int>` 提供 `forEach/map/filter/fold`（`<eff E = Pure>`）。
   - `crates/scoopc/src/typecheck/expr.rs`：lambda 期望类型下推推断扩展到 2 参数（支持 `(A, B) -> R`），以解锁 `fold(0) { acc, x -> ... }`。
   - `crates/scoopc/src/resolve/scopes.rs`：lambda 形参带类型注解时写入 local binding，便于后续成员访问解析（例如 `step.field`）。
   - fixtures：新增 `tests/fixtures/run-pass/stdlib_iter_algorithms_basic.scoop/.stdout`，覆盖 `Array/List/MutableArray/MutableList` 的 `forEach/map/filter/fold`。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1317f4 [DONE] stdlib：`Set/Map`（含 mutable）v0（基于 `MutableArray`，先做 `Int` 落点）
- 描述：基于 `MutableArray` 在 pure Scoop 中实现 `MutableSet<Int>`/`MutableMap<Int, V>`（必要时先固定 `Int->Int`）与只读 `Set/Map` 视图；与 delegated property 所需的 `scoop.collections.Map` 表面保持兼容。
- 目标：先保证语义正确与可回归，允许使用线性查找；后续再用 `Hashable.hash`/开放寻址优化；不新增 intrinsic。
- 验收：新增 run-pass fixtures 覆盖 set/map 的 `put/get/contains/remove` 等最小行为。
- 依赖：T1317f1、T1317e
 - 完成：
   - `stdlib/collections_set.scoop`：以 `typealias Set = Array<Int>` / `typealias MutableSet = MutableArray<Int>` 落地（避免 struct 持有引用字段导致 LLVM codegen 阻塞），提供 `len/contains/add/remove/asSet` 与 `mutableIntSet()` 构造。
   - `stdlib/collections_map.scoop`：以 `typealias MapView = Array<Int>` / `typealias MutableMap = MutableArray<Int>` 落地（interleaved pairs：`[k0,v0,...]`），提供 `entryCount/put/getByKey/getOrDefault/containsKey/removeKey/asMapView` 与 `mutableIntIntMap()` 构造；保持与 sysroot 的 `scoop.collections.Map`（delegated property 表面）不冲突。
   - fixtures：
     - `tests/fixtures/run-pass/stdlib_set_map_basic.scoop`
     - `tests/fixtures/run-pass/stdlib_set_map_basic.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1318（拆分为子任务）
- 描述：实现标准库的系统接口层：文件系统、路径、进程、环境变量、时钟/时间、基础 I/O 抽象。
- 目标：保持跨平台抽象；对不支持的平台通过 capability gating 或 no_std-like 降级。
- 验收：新增 std/run-pass fixtures：文件读写、路径操作、环境变量、时间 API 等在 host 平台可通过；不支持的平台有明确 gating/诊断。
- 依赖：T1316
- 备注：该任务包含多个相对独立的能力点。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，拆分为以下子任务并逐步推进：
  - 早期阶段先以 **C runtime（host platform backend）** 提供底层实现（符合 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md` 的结论：平台能力应走 runtime lib，而非新增 intrinsic）。
  - 平台差异隔离/ABI 最小化/更严格 capability matrix 的收敛由 T1401～T1404 承接，本组子任务先以 host POSIX/desktop 为主线落地最小可执行回归。

### T1318a [DONE] `std` v2：`env/time` 最小平台 API（host）
- 描述：提供最小可执行的 `scoop.env` 与 `scoop.time` API：读取环境变量、查询当前时间戳（或 monotonic clock）。
- 目标：先只覆盖 host 平台 happy path；不做完整错误类型体系；不做 WASM/embedded 的 adapter。
- 验收：新增 typecheck + run-pass fixtures：`env.getOrNull` 可读取 `// ENV:` 注入的变量；`time.now*` 可被调用并参与表达式求值；`cargo test --all` 与 `cargo run -p scoop -- test` 通过（启用 LLVM 时 run-pass 可回归）。
- 依赖：T1316
 - 完成：
   - sysroot：
     - `sysroot/env.scoop`：新增 `scoop.env.getOrNull(key: String): String?`
     - `sysroot/time.scoop`：新增 `scoop.time.nowUnixMillis(): Int`
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_env_get`（NULL → None）与 `scoop_time_now_unix_millis`（gettimeofday）
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.env.getOrNull`/`scoop.time.nowUnixMillis` 增加 sysroot → runtime 符号映射
   - fixtures：
     - `tests/fixtures/typecheck/std_env_time_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_env_time_basic.scoop`
     - `tests/fixtures/run-pass/std_env_time_basic.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `cargo run -p scoop -- test`
   - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1318b [DONE] `std` v2：`fs` 读写文本 v0（UTF-8）
- 描述：提供最小可执行的 `scoop.fs` 文件读写 API（例如 `readAllText`/`writeAllText`）。
- 目标：先只支持 UTF-8 文本与 host 平台；先不引入流式接口与权限/元数据 API。
- 验收：新增 run-pass fixtures：写入临时文件、再读回并断言 stdout；平台不支持时有稳定诊断。
- 依赖：T1318a
 - 完成：
   - sysroot：
     - `sysroot/fs.scoop`：新增 `scoop.fs.readAllText(path: String): String?` 与 `writeAllText(path: String, content: String): Int` 声明面（失败语义：`None` / 非 0）。
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_fs_read_all_text_utf8` / `scoop_fs_write_all_text_utf8`（`fopen/fread/fwrite`，失败返回 NULL / 非 0）。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.fs.readAllText/writeAllText` 增加 sysroot → runtime 符号映射；并让 `Option<String>` 返回值走 pointer niche（NULL → None）。
   - fixtures：
     - `tests/fixtures/run-pass/std_fs_text_basic.scoop`
     - `tests/fixtures/run-pass/std_fs_text_basic.stdout`
   - 修正：
     - `tests/fixtures/run-pass/std_env_time_basic.scoop`：避免把关键字 `out` 当作变量名（改为 `result`），保证 run-pass fixtures 可在启用 LLVM 后端时执行。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1318c [DONE] `std` v2：`process` 最小接口 v0（exit/args）
- 描述：提供最小可执行的 `scoop.process`：退出码、参数读取（必要时先只支持 `args.size/get`）。
- 目标：先不做子进程 `spawn`；只做“当前进程可观察接口”。
- 验收：新增 run-pass fixtures：能读取 argv 并输出；能按约定退出码退出（fixtures runner 断言 `EXPECT-EXIT`）。
- 依赖：T1318a
 - 完成：
   - sysroot：
     - `sysroot/process.scoop`：新增 `scoop.process.exit(code: Int): Unit` 与 `scoop.process.args(): Array<String>`（不含 argv[0]）声明面。
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_process_init`（保存 argv 指针）、`scoop_process_args_array`（首次构造并缓存 argv 数组）、`scoop_process_exit`（映射到 `exit(3)`）。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/mod.rs`：入口 `main` 改为 `main(argc, argv)` 并调用 `scoop_process_init(argc, argv)`。
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.process.exit/args` 增加 sysroot → runtime 符号映射。
   - driver：
     - `scoop run <input> [-- <args...>]`：支持向被运行程序透传 argv（用于 run-pass fixtures 回归）。
     - `crates/scoop/src/fixtures/run_pass.rs`：run-pass phase 将 fixture 的 `// ARGS:` 透传给 `scoop run`（最终成为程序 argv）。
   - fixtures：
     - `tests/fixtures/run-pass/std_process_args_exit_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1318d [DONE] `std` v2：`path` 最小接口 v0（join/basename/dirname）
- 描述：提供最小可执行的 `scoop.path` 路径操作（join/dirname/basename/normalize 的子集）。
- 目标：先只做 host 平台的分隔符规则；不要求与 Kotlin/Java/NIO 完全一致。
- 验收：新增 run-pass fixtures：路径 join 与 basename/dirname 行为可回归；不支持平台有明确 gating/诊断。
- 依赖：T1318a
 - 完成：
   - sysroot：
     - `sysroot/path.scoop`：新增 `scoop.path.normalize/join/basename/dirname` 声明面（最小归一化与分隔符规则）。
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_path_normalize/join/basename/dirname`（字符串层面的最小实现：折叠重复分隔符、去除末尾分隔符、root/`.` 约定）。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.path.*` 增加 sysroot → runtime 符号映射与函数签名声明。
   - fixtures：
     - `tests/fixtures/typecheck/std_path_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_path_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1318e [DONE] `std` v2：`io` 最小接口 v0（stdin/stdout/stderr 抽象）
- 描述：在已有 `print/println` 基础上，引入最小 `scoop.io` 抽象（`stdoutWriteString/stderrWriteString/stdinReadLine` 的占位或最小可执行子集）。
- 目标：先让库 API 形状固定并可回归；更完整的 buffering/编码/错误模型后续渐进补齐。
- 验收：新增 run-pass fixtures：stdout/stderr 可被区分断言（配合 `RUN-STDERR`）；stdin 在支持平台可读取（fixtures runner 支持注入 stdin）。
- 依赖：T1318a
 - 完成：
   - sysroot：
     - `sysroot/io.scoop`：新增 `scoop.io.stdoutWriteString/stderrWriteString/stdinReadLine` 声明面（top-level 形式，避免早期阶段 object member call 的门禁）。
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_io_stdout_write_string/scoop_io_stderr_write_string/scoop_io_stdin_read_line_utf8`（stdout/stderr 写入 + stdin readLine，UTF-8）。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.io.*` 增加 sysroot → runtime 符号映射与 `stdinReadLine` 的返回类型（`Option<String>` pointer niche）。
   - fixtures runner：
     - `crates/scoop/src/fixtures/expectations.rs`：新增 `RUN-STDIN` 指令解析（stdin 输入文件）。
     - `crates/scoop/src/fixtures/run_pass.rs`：run-pass phase 支持将 `RUN-STDIN` 文件内容写入子进程 stdin（随后关闭）。
   - fixtures：
     - `tests/fixtures/typecheck/std_io_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_io_stdout_stderr_basic.scoop/.stdout/.stderr`
     - `tests/fixtures/run-pass/std_io_stdin_read_line_basic.scoop/.stdin/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1319 `std` v3：sync / thread / channels / task support（拆分为子任务）
- 描述：实现标准库中的并发与同步层：线程 API、锁、条件变量、channel、thread-local、任务/调度辅助接口。
- 目标：桌面/服务端优先；embedded / wasm 通过 capability matrix 进行裁剪或适配。
- 备注：该任务范围过大。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，拆分为以下子任务；本条仅保留原始目标汇总。

### T1319a [DONE] `std` v3：`sync` 最小声明面 v0（Mutex/CondVar/Once）
- 描述：在 sysroot 中新增 `scoop.sync` 的最小 API 声明面（先只做 names/signatures），并新增 typecheck fixtures 回归。
- 目标：只做 sysroot 声明 + typecheck 覆盖；不实现 runtime C，不接入 LLVM codegen，不新增 run-pass fixtures。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`（typecheck fixtures 通过）
- 依赖：T1316
 - 完成：
   - sysroot：
     - `sysroot/sync.scoop`：新增 `scoop.sync` 的 `Mutex/CondVar/Once` 声明面与最小操作函数签名。
   - fixtures：
     - `tests/fixtures/typecheck/std_sync_api_surface_ok.scoop`

### T1319b [DONE] `std` v3：`sync` runtime v0（pthread mutex/cond/once）+ LLVM 映射 + run-pass
- 描述：为 `scoop.sync` 的 Mutex/CondVar/Once 提供最小可执行实现（runtime C + sysroot→runtime 映射），并新增 run-pass fixtures 回归锁/条件变量的基本行为。
- 目标：先只覆盖 host 平台（pthread）；不引入原子/无锁结构；不做公平性/性能优化。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`
- 依赖：T1319a、T0807、T0911
 - 完成：
   - runtime：
     - `runtime/c/scoop_sync.c`：新增 `scoop_sync_*`（Mutex/CondVar/Once）pthread 实现（对象以 `scoop_alloc` 分配，显式 destroy）。
   - runtime build：
     - `crates/scoop_runtime/build.rs`：把 `scoop_sync.c` 纳入 clang 构建与 rerun-if-changed。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：将 `scoop.sync.*` 映射到 runtime C 符号；并处理 `destroy` overload（按 receiver nominal 分派）。
   - fixtures：
     - `tests/fixtures/run-pass/std_sync_basic.scoop/.stdout`：新增 `Mutex/CondVar/Once` 最小可执行回归用例。
   - 回归修正（为保证 `--features llvm` 下 run-pass 可回归）：
     - `tests/fixtures/run-pass/std_io_stdin_read_line_basic.scoop`：避免使用关键字 `out` 作为变量名。
     - `tests/fixtures/run-pass/std_path_basic.stdout`：移除末尾多余空行（与实际 println 输出一致）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1319c [DONE] `std` v3：`thread` 最小接口 v0（spawn/join/sleep/yield）+ run-pass
- 描述：新增 `scoop.thread` 的最小可执行接口：线程创建与 join、sleep/yield、线程 id（若平台支持），并新增 run-pass fixtures 回归。
- 目标：先只做“spawn + join”等待完成；不做取消；不做 thread-local 初始化顺序保证。
- 验收：新增 run-pass fixtures：线程启动与 join 的可观察行为正确；不支持平台有稳定诊断。
- 依赖：T1319b
 - 完成：
   - sysroot：
     - `sysroot/thread.scoop`：新增 `scoop.thread` 最小声明面（`Thread` + `threadSpawn/join/sleepMillis/yield/currentId`）。
     - 说明：避免与语言内建 `spawn { ... }`（Task 语法糖）冲突，线程创建 API 采用 `threadSpawn` 命名。
   - runtime：
     - `runtime/c/scoop_thread.c`：新增 `scoop_thread_*`（pthread backend）：spawn/join/sleep/yield/current_id。
   - runtime build：
     - `crates/scoop_runtime/build.rs`：把 `scoop_thread.c` 纳入 clang 构建与 rerun-if-changed。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：将 `scoop.thread.*` 映射到 runtime C 符号，并支持传递 `() -> Unit` closure（env + fn_ptr）。
   - fixtures：
     - `tests/fixtures/typecheck/std_thread_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_thread_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1319d [DONE] `std` v3：`channels` 最小接口 v0（unbounded mpsc）+ run-pass
- 描述：新增 `scoop.channels`：最小的 unbounded mpsc channel（send/recv/close），并用 run-pass fixtures 固化基本语义。
- 目标：先只做阻塞 recv（配合 CondVar）；不做 select；不做 async adapter。
- 验收：新增 run-pass fixtures：多线程 send/recv 顺序与 close 语义可回归；不支持平台有稳定诊断。
- 依赖：T1319b、T1319c
 - 完成：
   - sysroot：
     - `sysroot/channels.scoop`：新增 `scoop.channels`（`Channel<T>` + `channelCreate/send/recv/close`）声明面。
   - runtime：
     - `runtime/c/scoop_channels.c`：新增 `scoop_channels_*`（pthread backend）：unbounded 队列 + `send/recv/close`。
   - runtime build：
     - `crates/scoop_runtime/build.rs`：把 `scoop_channels.c` 纳入 clang 构建与 rerun-if-changed。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：将 `scoop.channels.*` 映射到 runtime C 符号；`recv` 返回 `Option<T>`（Some/None）。
     - 回归修正：补齐 closure capture v0（immutable captures）：closure env 使用 `malloc` 分配并经 `env_ptr` 传入（用于 `threadSpawn` 传递 channel 到新线程）。
   - fixtures：
     - `tests/fixtures/typecheck/std_channels_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_channels_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1319e [DONE] `std` v3：task/executor 支持接口 v0（对接 runtime task executor）
- 描述：为 `Task<T>` / executor 提供 std 层适配接口（对接 `runtime/c/scoop_task_executor.c`），并新增最小 run-pass fixtures 回归。
- 目标：先只做 “create/start/complete/onComplete” 的最小路径；不做 work-stealing/多 executor；不做 cancellation。
- 验收：新增 run-pass fixtures：task state 与完成回调行为可回归。
- 依赖：T0917、T1319b
 - 完成：
   - sysroot：
     - `sysroot/task.scoop`：新增 `scoop.task`（`Executor` + `executorCreate/debugPendingCount/runNext/runUntilIdle/destroy`；`Task<Int>` 的 `taskCreate/taskCreateManual/state/result/tryStart/complete/onComplete`）。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：把 `scoop.task.*` sysroot 表面映射到 `scoop_task_executor.c` 的 runtime C 符号：
       - `Executor` 在 early stage 落到 word-sized handle（与 `Task<T>` 一致）；
       - `taskCreate { ... }` 传递 `() -> Int` closure 的 env/fn 指针给 runtime（u64 body_fn + ctx）。
   - fixtures：
     - `tests/fixtures/typecheck/std_task_executor_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_task_executor_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- test`

### T1320 `std` v4：net / async adapters / testing & support utilities（拆分为子任务）
- 描述：补齐标准库的高阶能力：网络抽象、与 `Task`/executor 的 async adapters、测试支持工具、日志/诊断/配置等公共 utilities。
- 目标：保持与 runtime backend 解耦；WASM/embedded 环境通过 adapter 或 capability gating 处理。
- 验收：新增 std/run-pass fixtures：基础 TCP/HTTP-like adapter、async task utility、test helper 能在受支持平台工作；不支持平台有 capability matrix 覆盖。
- 依赖：T1316、T0917、T1403
- 备注：该任务范围过大。为保持“可单独实现 & 单独验证”的粒度，拆分为以下子任务；本条仅保留原始目标汇总。

### T1320a [DONE] `std` v4：`scoop.test` 最小断言工具（assertions）+ fixtures
- 描述：在 `stdlib` 注入层新增 `scoop.test` 包，提供最小断言（assert）函数，方便 run-pass fixtures 书写与复用。
- 目标：
  - 只提供最小、可执行、无新增 runtime 依赖的断言：`assertTrue/assertFalse/assertEqInt/assertSomeInt/assertNoneInt`；
  - 不实现字符串格式化/拼接（避免引入新的 `String` utilities 依赖）；失败时直接通过 `Raise<RuntimeError>` 终止。
- 验收：
  - 新增 typecheck fixture：`tests/fixtures/typecheck/std_test_assertions_api_surface_ok.scoop`；
  - 新增 run-pass fixture：`tests/fixtures/run-pass/std_test_assertions_basic.scoop/.stdout`；
  - `cargo test --all`；
  - `cargo run -p scoop -- test`；
  - `cargo run -p scoop --features llvm -- test`（若本机 LLVM 可用）。
- 依赖：T1315a
 - 完成：
   - sysroot：
     - `sysroot/test.scoop`：新增 `scoop.test` 断言声明面（typecheck 可见）。
   - stdlib：
     - `stdlib/test.scoop`：新增断言实现（避免在 stdlib 文件中引入字面量）。
   - fixtures：
     - `tests/fixtures/typecheck/std_test_assertions_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_test_assertions_basic.scoop/.stdout`

### T1320b [DONE] `std` v4：`Task`/executor async adapters v0（map/await/bridge）
- 描述：在 stdlib 层为 `Task<T>`/`Executor` 提供最小的组合子与桥接工具（例如 `map/andThen/await` 的 v0 形态），提升异步样例可写性。
- 目标：
  - 先以 `Int` 专用落点为主（与现有 stdlib 策略一致），避免一次引入泛型/单态化边角；
  - 不引入新的调度策略：仍复用 runtime executor（T0917/T1319e）。
- 验收：新增至少 1 个 run-pass fixture 覆盖 adapter 的基本行为；`cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 依赖：T1319e
 - 完成：
   - sysroot：
     - `sysroot/task.scoop`：补齐 `Executor.spawn/await` 与 `Task<Int>.map/andThen` 的声明表面（typecheck 可见）。
   - stdlib：
     - `stdlib/task.scoop`：落地 `spawn/await/map/andThen` 的 pure-scoop 实现（内部用 `__TaskAwaitInt` 承载 escape continuation 适配）。
   - fixtures：
     - `tests/fixtures/typecheck/std_task_async_adapters_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_task_async_adapters_basic.scoop/.stdout`

### T1320c [DONE] `std` v4：测试/诊断支持 utilities v0（platform info / io helpers）
- 描述：补齐一组“为 fixtures/示例服务”的通用工具（例如 platform/query、简单的 stdout/stderr helper），并用 fixtures 固化。
- 目标：先提供最小可观测性；不引入复杂 logging 框架；避免把 OS 概念直透到 Scoop 侧（配合 capability matrix）。
- 验收：新增 typecheck/run-pass fixtures 覆盖至少 2 个工具函数；不支持平台有稳定诊断或返回 `None/false`。
- 依赖：T1318e、T1219
 - 完成：
   - sysroot：
     - `sysroot/io.scoop`：新增 `stdoutWriteLine/stderrWriteLine` 声明面（写入并追加换行）。
   - runtime：
     - `runtime/c/scoop_runtime.c`：新增 `scoop_io_stdout_write_line/scoop_io_stderr_write_line`。
   - LLVM codegen：
     - `crates/scoopc/src/llvm/codegen.rs`：为 `scoop.io.stdoutWriteLine/stderrWriteLine` 增加 sysroot → runtime 符号映射。
   - fixtures：
     - `tests/fixtures/typecheck/std_io_write_line_api_surface_ok.scoop`
     - `tests/fixtures/run-pass/std_io_write_line_basic.scoop/.stdout/.stderr`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo build -p scoop --features llvm`
   - （可选）`PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" target/debug/scoop run tests/fixtures/run-pass/std_io_write_line_basic.scoop`

### T1320d [DONE] `std` v4：net 抽象 v0（API surface + capability gating）
- 描述：定义 `scoop.net` 的最小 API surface（TCP 连接/监听或更高层 adapter），并通过 capability gating 表达“不支持”。
- 目标：
  - 先固定 API 形状与错误模型（可先用 `Option/Result` 的简化版本），避免把 libc socket 细节暴露给 Scoop；
  - runtime 侧实现以 platform/backends 为边界（与 T1402/T1403 对齐）。
- 验收：新增 typecheck fixtures 固化 API surface；后续在支持平台增加 run-pass fixtures 覆盖最小 TCP roundtrip。
- 依赖：T1403、T1404
 - 完成：
   - sysroot：
     - `sysroot/net.scoop`：新增 `scoop.net` 最小声明表面（TCP handle + capability gating）。
   - fixtures：
     - `tests/fixtures/typecheck/std_net_tcp_api_surface_ok.scoop`

### T1018 [DONE] 若审计证明必要：新增最小 intrinsic/backends 以解锁纯 Scoop stdlib/运行库（库层）
- 描述：仅针对 T1017 证明无法绕过的阻塞项，增加最小的新 intrinsic 或 backend hook；并把这部分与上层 Scoop stdlib/运行库（库层）任务解耦。
- 目标：不直接在此任务实现高层库功能；只提供最小 primitive，并保持数量与语义面尽可能小。
- 验收：每个新增 intrinsic 都有对应的 blocker 说明、fixture、以及至少一个上层库调用方从“卡住”变为“可实现”的证明。
- 依赖：T1017
 - 完成：
   - 结论：根据 `RUNTIME_STDLIB_INTRINSIC_AUDIT.md`（T1017）的 §2.3 `needs_new_intrinsic`，在 `std v1` 与 Kotlin runtime gap 的“纯 Scoop 补齐”目标边界内，未发现必须新增 intrinsic/backends 的 blocker，因此本任务在 0.1 阶段以 **no-op** 形式完成（无新增 intrinsic）。
   - `TODO.md`：将本任务标记为 `[DONE]`，并记录“未来出现真实 case 时再 reopen”的维护策略（见上文结论）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1027 [DONE] internal atomics：`__AtomicInt/__AtomicLong/...`（FFI-oriented）（spec §15.9.4）
- 描述：新增一组内部原子值类型（GC-free，同底层布局），并用 intrinsics 直接生成 LLVM IR 原子指令（load/store/CAS 等），用于 runtime/FFI 的全局状态。
- 目标：与对外暴露的 `AtomicInt`（若存在）区分：这些是**值类型**，且 API 面向 runtime/interop，不作为通用高层并发库承诺。
- 验收：新增 run-pass fixture：对 `__AtomicInt` 做原子 load/store/CAS（多线程可先用单线程语义/模型测试；并发正确性可后续补）。
- 依赖：T1017、T1018
 - 完成：
   - sysroot：
     - `sysroot/unsafe.scoop`：新增 `__AtomicInt` 与 `__atomicIntLoad/__atomicIntStore/__atomicIntCompareExchange` 声明面（SeqCst）。
   - compiler（HIR/codegen）：
     - `crates/scoopc/src/hir/lower.rs`：将 `scoop.unsafe.__AtomicInt` 视为 `Int`（value type）以保持 ABI 一致（避免被默认降级为 ref nominal）。
     - `crates/scoopc/src/llvm/codegen.rs`：识别并为上述 intrinsics 直接生成 LLVM atomic load/store/cmpxchg。
   - fixtures：
     - `tests/fixtures/run-pass/unsafe_atomic_int_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/unsafe_atomic_int_basic.scoop`

### T1025 [DONE] unsafe 指针 API 升级：`addressOf` + `Ptr<T>` methods + `stackAlloc`（spec §15.9.4）
- 描述：把当前最小原语从“自由函数形态”演进为规范中的 API：
  - `addressOf(var: T): Ptr<T>`
  - `Ptr<T>.cast/load/store`
  - `Ptr<T>` 的元素级 `plus/minus`
  - `stackAlloc<T>(): Ptr<T>`（GC-free）
- 目标：
  - 兼容迁移期：允许 sysroot 同时提供旧名（`addrOf/load/store`）与新名一段时间；
  - typecheck 必须保持 unsafe context 门禁与 `Ptr<T>` 的 GC-free 限制不被绕过。
- 验收：新增 unsafe_nogc fixtures 覆盖新 API 的 pass/fail；并保持旧 fixtures 仍可回归（或提供自动迁移脚本后一次性更新 fixtures）。
- 依赖：T1017、T1018
 - 完成：
   - sysroot：
     - `sysroot/unsafe.scoop`：补齐 `addressOf/stackAlloc` 与 `Ptr<T>.cast/load/store/plus/minus` 的声明表面，并保留迁移期旧名 `addrOf/load/store`。
   - parser：
     - `crates/scoopc/src/parser/decls.rs`：允许在形参位置把关键字 `var` 解析为参数名，以支持 sysroot 声明 `addressOf(var: T)` 的约定语法。
   - typecheck：
     - `crates/scoopc/src/typecheck/expr.rs`：
       - 新增 `var` 形参的 lvalue 门禁（错误码 `scoop::typecheck::var_param_requires_lvalue`）；
       - 扩展成员调用支持显式类型实参（`p.cast<T>()`）；
       - 泛型实例化支持多 type params + 迁移期“部分显式类型实参 + 其余推断”的路径；
       - 在泛型 substitution 中补齐 `Ptr<T>` 的 GC-free pointee 门禁，避免通过 `cast/stackAlloc/uintPtrToPtr` 绕过。
   - fixtures：
     - 更新 `tests/fixtures/unsafe_nogc/unsafe_ptr_primitives_*` 以覆盖新 API；
     - 新增 compile-fail fixtures：`var` 形参 lvalue 门禁、`Ptr<T>` GC-free 门禁（cast/stackAlloc）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1026 [DONE] `FunPtr<F>` + `@CallingConvention`：FFI 函数指针（spec §15.9.4 / §15.5.4）
- 描述：新增 `FunPtr<F>`（值类型、GC-free）与调用约定：
  - 支持 `@CallingConvention("stdcall" | "cdecl" | ...)` 影响 codegen；
  - 允许与 `Ptr<T>` 互相 unsafe cast；
  - 提供 `invoke`/`()` 调用形态（sysroot 定义，编译器提供 lowering/codegen）。
- 目标：先只覆盖 host 平台常见 calling convention；不做跨平台全量覆盖。
- 验收：新增 run-pass fixture：从 `@Extern` 获取函数指针并通过 `FunPtr` 调用（可用 runtime/c 的测试导出函数）；并新增 compile-fail fixture：在 safe context 调用 `FunPtr` 报错。
- 依赖：T1017、T1018、T1025
 - 完成：
   - sysroot：
     - `sysroot/unsafe.scoop`：新增 `@Intrinsic struct FunPtr<F>` + `funPtrToUIntPtr/uintPtrToFunPtr` + `FunPtr<...>.invoke(...)` 声明表面。
   - typecheck：
     - 内建注解：新增 `@CallingConvention("c"|"cdecl")`（仅允许用于 `fun`/`typealias`；其它值给出稳定错误码）。
     - `FunPtr<F>` 良构性：`F` 必须是 non-receiver function type；并在表达式层面支持 `fp(args...)`（要求 unsafe context；const fun 内禁止）。
   - HIR / LLVM：
     - `@Extern` 函数携带可选 calling convention 元信息；LLVM 侧目前仅落地 C ABI（callconv 0）。
     - `FunPtr<F>` 在 LLVM 中表示为 word-sized unsigned address，并支持 indirect call（`inttoptr` + `call`）。
     - sysroot `scoop.unsafe.invoke(...)` 作为 intrinsic 内建 lowering（用于 `fp.invoke(...)`）。
   - runtime：
     - `runtime/c/scoop_test.c`：新增 `scoop_test_add_int` + `scoop_test_get_add_int_funptr`，供 fixtures 回归链接/调用。
   - fixtures：
     - compile-fail：`tests/fixtures/unsafe_nogc/funptr_call_requires_unsafe_is_error.scoop`
     - run-pass：`tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop/.stdout`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo test -p scoopc --features llvm`
   - `PATH="/opt/homebrew/opt/llvm@18/bin:$PATH" cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/unsafe_funptr_extern_call_basic.scoop`

### T1321 [DONE] Kotlin 风格重载决议：most specific candidate 规则收口
- 描述：把通用 overload resolution 收口为 Kotlin 风格的用户可感知规则：最具体候选优先、member/extension/constructor 的优先级固定、歧义行为稳定。
- 目标：不要求与 Kotlin 每个边角完全一致，但要把 Scoop 采用的差异点写清楚并通过 fixtures 固化。
- 验收：language fixture：`Int` vs `Any`、member vs extension、constructor overload 的最具体候选行为稳定；文档中列出与 Kotlin 不同处。
- 依赖：T0513、T0454、T0455
 - 完成：
   - typecheck：
     - `crates/scoopc/src/typecheck/expr.rs`：class ctor overload 在多匹配时引入 most-specific candidate 选择（参数更具体优先；tie-break：默认参数使用更少者优先）。
   - fixtures：
     - `tests/fixtures/typecheck/class_ctor_overload_most_specific_int_vs_any_ok.scoop`：覆盖 ctor overload `Int` vs `Any` 的 most-specific 选择。
   - 文档：
     - `PLAN.md`：补充 Kotlin overload resolution 的差异说明（member/extension 优先级当前仅按“同名存在性”决定）。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1322 [DONE] 默认参数 / 命名参数 / trailing lambda 与重载集合的交互
- 描述：把 T1305/T1306/T1307 与 overload sets 联动：调用时允许这些特性参与候选筛选与 tie-break，而不是只在“唯一目标”前提下工作。
- 目标：先覆盖最常见组合；varargs 与重载的复杂交互后续再细化。
- 验收：language fixture：多个 overload 中，命名参数和 trailing lambda 能帮助选中正确候选；真正无法区分时仍报歧义。
- 依赖：T1305、T1306、T1307、T0512
 - 完成：
   - typecheck/expr：
     - 命名参数规则放开 Kotlin-like 例外：允许 `f(x = 1) { ... }`（trailing lambda 作为最后一个位置实参）；
     - 形参映射补齐 trailing-lambda + 默认参数交互：当严格映射失败且最后一个实参为 lambda 时，尝试把该 lambda 绑定到最后一个形参槽位，并允许中间默认参数缺失（用于 overload sets 的候选可用性判定）。
   - fixtures：
     - pass：`tests/fixtures/infer/overload_resolution_named_args_trailing_lambda_disambiguate_ok.scoop`
     - fail：`tests/fixtures/infer/overload_resolution_named_args_trailing_lambda_ambiguous_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1323 [DONE] 默认参数：中间参数省略与命名参数联合规则
- 描述：把默认参数从“只省略尾部参数”推进到 Kotlin 风格的中间参数省略，只要调用点使用命名参数并满足规则，就允许跳过中间带默认值的形参。
- 目标：不改变 T1305 的基础行为；本任务专门补齐“中间省略 + 命名参数”的语言规则与诊断。
- 验收：language fixtures：`f(a = 1, c = 3)` 会用默认值补上 `b`；非法跳过非默认参数或与位置参数混用时报错。
- 依赖：T1305、T1306、T1322
 - 完成：
   - typecheck/expr：
     - 默认参数映射允许在命名实参存在时“跳过中间 default 形参”（Kotlin-like；既有规则固化为 fixtures）；
     - 新增稳定诊断 `call_missing_required_args`：命名实参跳过非默认形参时报错并列出缺失形参名。
   - hir/lower：
     - call-site 默认参数补齐扩展为支持命名实参与中间默认参数；
     - 默认参数索引从“仅尾部 defaults”扩展为“任意位置 defaults”（仍限制：当前文件内、非泛型、无 receiver、无 overload、无 vararg）。
   - fixtures：
     - pass：`tests/fixtures/typecheck/default_param_named_args_mid_omit_ok.scoop`
     - fail：`tests/fixtures/typecheck/default_param_named_args_skip_non_default_is_error.scoop`
     - hir：`tests/fixtures/hir/default_param_named_args_mid_omit_lowering.scoop/.hir`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1324 [DONE] 多 trailing lambda：语法、expected type 与重载联动
- 描述：在已有单 trailing lambda 基础上，支持多个 trailing lambda（若采用 Kotlin-like 语法），并让 expected type、命名参数与 overload resolution 一起参与决议。
- 目标：先固定语法与候选选择规则；不要求所有 DSL 风格写法一次覆盖。
- 验收：language fixtures：多个 lambda 参数的调用可在 trailing 形式下通过；歧义或位置非法时报清晰错误。
- 依赖：T1307、T1322
 - 完成：
   - parser/expr：放开连续 `{ ... }` postfix，使 `f { ... } { ... }`/`f(args) { ... } { ... }` 解析为同一次 call 的多个 lambda 实参。
   - typecheck/expr：
     - 命名参数规则扩展：允许“命名实参之后”出现多个末尾 lambda（trailing lambdas）。
     - 形参映射 fallback 扩展：支持末尾连续 N 个 lambda，绑定到最后 N 个形参槽位；若最后形参为 vararg，则视为追加 N 个 vararg 元素。
   - fixtures：
     - pass：`tests/fixtures/infer/trailing_lambda_multiple_overload_and_expected_type_ok.scoop`
     - fail：`tests/fixtures/typecheck/trailing_lambda_multiple_arity_mismatch_is_error.scoop`
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1325（拆分为子任务）
- 描述：把 `vararg` spread 从“数组/tuple 的最小形式”扩展到更完整的桥接规则：允许标准集合/可迭代视图在明确转换或约定 API 下参与 spread。
- 目标：先固定语言层/stdlib 层边界；避免隐式、不可控的大规模分配。
- 验收：language + std fixtures：集合经约定桥接后可用于 `vararg` 调用；不满足桥接条件时报错并指出所需转换。
- 依赖：T1308、T1317

> 备注：该任务包含多个相对独立的能力点。为保持 TODO 顺序“首个 `[TODO]` 可直接实现”，拆分为以下子任务；本条仅保留原始目标汇总。

### T1325a [DONE] varargs spread：`MutableArray/MutableList` → `Array` 的显式桥接（`toArray()`）
- 描述：在 stdlib 为 `MutableArray<Int>` 提供 `toArray(): Array<Int>`（复制语义），使其可在调用点通过 `*xs.toArray()` 参与 vararg spread。
- 目标：只提供 `Int` 专用落点；不改变现有 spread 规则（仍只接受 `Array/tuple`）；不新增 intrinsic。
- 验收：
  - 新增 typecheck fixture：`MutableArray<Int>` 经 `toArray()` 可用于 spread；
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T1308、T1317
 - 完成：
   - `sysroot/core.scoop`：增加 `MutableArray<Int>.toArray(): Array<Int>` 声明面，供 resolver/typecheck 可见；
   - `stdlib/mutable_array.scoop`：落地 `toArray()` 的复制实现（避免 aliasing）；
   - fixtures：新增 `tests/fixtures/typecheck/vararg_spread_mutable_array_to_array_bridge_ok.scoop` 覆盖 `MutableArray/MutableList` 两种 receiver。
 - 验收：
   - `cargo test --all`
   - `cargo run -p scoop -- test`

### T1325b [TODO] varargs spread：对常见集合提供“缺失桥接”的迁移诊断
- 描述：当 `*xs` 的 `xs` 为 `MutableArray/MutableList/MutableSet/MutableMap` 等常见集合类型时，诊断提示应使用约定转换（例如 `toArray()/asSet()/asMapView()`）再 spread。
- 目标：仅增强错误信息（保持稳定 error code）；不放开隐式 spread；不引入新的语义。
- 验收：新增至少 1 个 fail fixture 断言错误信息包含建议转换；`cargo test --all` 通过。
- 依赖：T1325a

### T1325c [TODO] varargs spread：迭代器视图桥接（iterator → Array）
- 描述：为满足 `iterator` 协议的视图提供显式转换为 `Array` 的 std helper（例如 `toArray()`），以支持 `*view.toArray()` 的 spread 用例。
- 目标：与 T1328 的 `Iterator.next(): Option<T>` 迁移一致；不在语言层放开 `*iterable` 隐式 spread。
- 验收：新增 typecheck +（可选）run-pass fixture 覆盖最小可迭代对象的 `toArray()` 后 spread；`cargo test --all` 通过。
- 依赖：T1328

### T1326 [TODO] delegated properties：线程安全语义与平台 policy
- 描述：补齐标准 delegated properties 的线程安全语义：`lazy` 的同步/发布/无锁策略，`observable`/`vetoable` 在并发场景下的可见性与回调规则，以及不同平台的 policy。
- 目标：让 `lazy` 等 API 不只“能跑”，还要在 desktop/server/embedded/WASM 等环境下有明确行为约定。
- 验收：language/std fixtures：支持的平台上线程安全语义可回归验证；不支持的平台有 capability matrix 或降级策略说明。
- 依赖：T1313、T1319

### T1327 [TODO] 类初始化兼容：复杂继承链与 effect 细节
- 描述：在已有类初始化顺序实现基础上，补齐复杂继承链、父类初始化交错、以及初始化期间 effect/异常传播的 Kotlin-like 细节。
- 目标：不改写 T1312 的基础顺序规则；本任务专门收敛复杂继承与异常/effect 交互的边角行为。
- 验收：language fixtures：多层继承中的属性初始化、`init` 链与 secondary constructor 行为稳定；初始化期 effect 违规或异常路径有明确诊断。
- 依赖：T1312、T0439、T0609

### T1328 [TODO] `for`/迭代协议升级：`Iterator.next(): Option<T>`（移除 `hasNext()`）
- 描述：把 `for (x in xs)` 的迭代协议从旧的三元调用（`iterator()/hasNext()/next()`）升级为单一 `next(): Option<T>`（见 `SCOOP_FULL_SPEC.md` §16.2），避免 iterator 需要内部缓存“下一元素”。
- 目标：
  - typecheck：`for (x in xs)` 只要求：
    - `xs.iterator(): It`
    - `It.next(): Option<T>`（`T` 为 loop binder `x` 的类型）
  - 禁止继续依赖 `hasNext()`；若类型仅提供旧协议，应给出清晰迁移诊断（指出需要 `next(): Option<T>`）。
  - `for` desugaring（HIR/MIR lowering）按 `Option` 分支展开：`Some(x)` 进入 body，`None` break。
- 验收：
  - 新增/更新 typecheck fixtures：
    - pass：自定义 `iterator/next()->Option<T>` 的类型可用于 `for`；
    - fail：缺少 `iterator`、缺少 `next`、或 `next` 返回非 `Option<T>` 时均报稳定错误码；
    - 删除/迁移旧的 `missing_has_next` fixtures（或保留为 “旧协议不再被接受” 的 fail 用例）。
  - 若 `for` lowering 已落地：新增 run-pass fixture：`for` 遍历一个最小可迭代对象输出序列正确。
  - `cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 依赖：T1304、T0407、T0705（for lowering 若开启）、`SCOOP_FULL_SPEC.md` §16.2

---

## T14：C runtime 长期路线（平台隔离 + GC/Immix）（阶段 12：长期稳定）

### T1401 [TODO] runtime ABI 最小化：集中声明 + 导出符号审计（PLAN §15.1）
- 描述：把对外导出的 runtime C 符号集中到单一头文件/清单（allowlist），并在 CI/本地测试中校验“新增导出必须显式登记”，防止 ABI 膨胀与平台细节泄漏到 Scoop 侧。
- 目标：
  - runtime ABI 只承载 Scoop 必需的最小能力（alloc/gc/effects/platform primitives），不把 libc/OS API 逐个映射到 sysroot；
  - 新增符号必须同时更新文档与 codegen 映射表；
  - 不要求一次性重构现有导出函数命名，只先建立可审计边界。
- 验收：
  - 新增 `runtime/c/scoop_runtime_api.h`（或等价清单）列出当前导出符号；
  - 新增一个检查（unit test 或 `scoop_tools` 子命令）：比对 `nm`/`objdump` 的导出符号与 allowlist，出现未登记导出时失败；
  - `cargo test --all` 通过。
- 依赖：T0806、T0901

### T1402 [TODO] 平台层抽象 v0：引入 `runtime/c/platform`（POSIX backend）（PLAN §15.2）
- 描述：在 C runtime 内部引入 platform 层 API（如 `platform.h` + `platform_posix.c`），把 `getenv/gettimeofday/open/read/write/pthread_*` 等 OS 调用收敛到 platform/backends；core/runtime 代码只调用 platform API。
- 目标：
  - 先以 host POSIX/pthread 为主线；Windows/其他平台只放占位接口与 build gate；
  - 把平台相关 `#ifdef` 限制在 `runtime/c/platform/*`，不散落到 GC/effect/core；
  - 不改变 sysroot API surface（除非发现明显平台泄漏）。
- 验收：
  - `runtime/c/` 下新增 `platform/` 目录与 `platform.h`；
  - 至少将 `env/time`（T1318a）与 `io`（T1318e）路径改为走 platform API；
  - `cargo test --all` 与 `cargo run -p scoop -- test` 通过。
- 依赖：T1318a、T1318e

### T1403 [TODO] 平台层扩展：thread/sync/channels/task/net 的 backend 隔离（PLAN §15.2）
- 描述：把 `scoop_sync_*`/`scoop_thread_*`/`scoop_channels_*`/task executor/net 等平台相关实现从“直接依赖 pthread/OS API”改为调用 platform 层接口，形成清晰的 backend 边界。
- 目标：
  - 明确哪些能力是“必须由 runtime C 提供的平台能力”，并避免在 Scoop 侧通过 FFI/extern 直接触达 OS；
  - 为将来 Windows backend 预留接口（不要求本任务实现 Windows）。
- 验收：
  - `runtime/c/platform/` 中新增 thread/sync/channel/task/net 所需的最小接口声明；
  - 现有 POSIX/pthread backend 可构建并跑通现有 run-pass fixtures（`std_sync_*`/`std_thread_*`/`std_channels_*`/`task` 若已落地）。
- 依赖：T1319b、T1319c、T1319d、T1319e

### T1404 [TODO] sysroot/std 平台 API surface 审计：不泄漏 OS 概念（PLAN §15.1）
- 描述：审计 sysroot 与 stdlib 中的平台 API（env/time/fs/path/io/process/thread/sync/channels/net），确保不暴露 `errno/FILE*/HANDLE/pthread_t` 等 OS 概念与平台差异；必要时调整为平台无关的 `Option/Result` 形状，并用 capability gating 表达“不支持”。
- 目标：
  - API 设计以“可移植 + 最小 surface”为优先；
  - 错误模型先允许简化（`Option` 或 `Result<Int, RuntimeError>`），但必须可扩展；
  - 不在 stdlib 侧引入 `cfg(target_*)` 分叉语义（由 runtime platform backend 解决）。
- 验收：
  - 输出一份清单：每个 sysroot 平台模块的 API surface、对应 runtime 符号、平台 backend 支持情况；
  - 新增至少 1 个 typecheck fixture：确认平台 API 不暴露平台相关类型；
  - `cargo run -p scoop -- test` 通过。
- 依赖：T1316、T1318、T1319

### T1405 [TODO] GC backend 抽象：编译期可替换 GC（baseline / Immix / embedded / adapter）（PLAN §15.3）
- 描述：为 GC 引入稳定抽象边界，使 runtime/allocator/roots 扫描与具体 GC 算法解耦，并支持编译期选择不同 backend（先 C 实现）。
- 目标：至少支持 baseline GC、后续 Immix、高裁剪 embedded/minimal backend，以及 hosted/adapter backend（如 WASM GC adapter）的插拔。
- 验收：构建系统可通过 feature/config 选择不同 GC backend；同一套核心 GC fixtures 可在至少两种 backend 下运行。
- 依赖：T0904、T1505、T1506

### T1406 [TODO] Immix v0：单线程、非移动 backend（PLAN §15.3）
- 描述：在 baseline GC 之外实现 Immix（line/block allocator + mark-region）作为可选 backend（先单线程、非移动）。
- 目标：先保证正确性与可回归；不要求第一版就替换默认 GC。
- 验收：构建时可切换到 Immix backend；核心 GC fixtures 通过；至少一组分配/碎片化 microbench 显示优于 baseline（可先本地运行）。
- 依赖：T1405

### T1407 [TODO] Immix：支持移动与压缩（moving/compaction）（PLAN §15.3）
- 描述：为 Immix 增加 evacuation/compaction：在 STW 阶段把存活对象从稀疏 block 搬迁到更紧凑的区域，使用 forwarding pointer 修复引用，并与 pin/unpin/FFI 语义兼容。
- 目标：
  - 先支持“选择性 block evacuation”与最小 forwarding 机制；
  - 明确并实现 pin policy（被 pin 对象不移动；必要时落入 non-moving 区域）。
- 验收：
  - 新增 GC fixtures：触发 compaction 后引用仍正确；被 pin 对象地址稳定；
  - 运行 `--gc-stress` 下多轮通过（无 use-after-move/悬挂指针）。
- 依赖：T1406、T0912、T1008、T1511

### T1408 [TODO] Immix：多线程正确性（PLAN §15.3）
- 描述：在 Immix backend 下保证多线程正确性：线程注册与 root 枚举、安全点/stop-the-world 协议、跨线程引用扫描、线程本地分配路径与 GC 元数据并发安全。
- 目标：先保证正确性与可回归；并行标记等性能优化留给后续任务。
- 验收：新增多线程 GC fixtures：多线程分配/回收/跨线程引用在 Immix backend 下稳定通过；TSAN（可选）不报 data race。
- 依赖：T1407、T1505

### T1409 [TODO] Immix：多线程性能（PLAN §15.3）
- 描述：在保证正确性的基础上优化 Immix 的多线程性能：TLAB/per-thread blocks、低争用全局 block 池、并行标记/并行 sweep 的渐进引入，并建立可重复的基准与回归阈值。
- 目标：优先降低分配路径锁争用与 STW 时间；不追求一次做到最佳。
- 验收：新增基准或 microbench：
  - 多线程分配吞吐在 N 线程下接近线性扩展（在合理范围内）；
  - STW 时间与碎片化指标有可观测改进或至少不回退。
- 依赖：T1408

### T1410 [TODO] Hosted / adapter GC backend：WASM GC adapter 与受限环境适配（PLAN §15.3）
- 描述：为不适合自带 GC 的环境提供 adapter backend，例如对接 WASM GC 或极简 hosted allocator/collector。
- 目标：先实现 backend 形状与 capability matrix；不要求一次覆盖所有宿主。
- 验收：至少一条 hosted/adapter 路径可编译并通过受限能力测试；WASM target 下的 capability matrix 明确哪些模块可用/不可用。
- 依赖：T1405、T1316

### T1411 [TODO] non-resuming effect / unwind backend：接入并复用 `libunwind`
- 描述：为 non-resuming effect（以及其他需要栈展开的路径）接入 `libunwind`（或等价后端），并与 GC 的 stack walking 共享同一套 platform/unwind 模块；避免把 unwind/ABI 细节泄漏到 Scoop 侧。
- 目标：先支持 host 平台；Windows / embedded / wasm 可按 backend 分层处理。
- 验收：新增 run-pass / runtime fixture：非恢复 effect 可正确展开、执行 cleanup，并生成稳定诊断（若启用）。
- 依赖：T0614、T0707、T1505

---

## T15：引用类型 + LLVM StackMap GC（端到端，目标是“可用且可移动”的 ref runtime）

> 背景：当前仓库存在 shadow stack 相关实现（T0905/T0816/T0909/T0911），但目标运行时需要以 LLVM StackMap/statepoint 为根集来源，打通引用类型（class/interface/closure/array/string/box）的对象模型、动态分发、typed alloc、精确 GC roots、以及移动 GC 的 roots 更新链路。本组任务以“完整链路”为验收标准，不依赖任何占位/手工 roots 方案。

### T1501 [TODO] 固化 ref 对象模型与 runtime ABI（Object Header + TypeDescriptor + Dispatch）
- 描述：输出一份可直接实现的 runtime ABI 规范：ref 指针表示、对象头字段与偏移、type descriptor 字段、vtable/itable 表示、RTTI（type id/parent/interface ids）与 `is/as/as?` 的运行期判定规则。
- 目标：
  - ref 指针在 LLVM IR 中统一为 `addrspace(1)`（GC-managed），与 `scoop.unsafe.Ptr<T>`（addrspace(0)）严格区分；
  - 对象头必须包含：`next`、`type_desc*`、`size_bytes`、`flags`、`mark`，并用 C `_Static_assert` 固化偏移；
  - type descriptor 必须包含：`size_bytes/align_bytes/trace_start_offset_bytes/trace_bitmap_words/trace_fn/release_fn/type_id/parent_type_desc/itable/vtable`（字段名可不同但语义必须等价）。
- 验收：
  - `SCOOP_RUNTIME.md` 写出完整字段表、偏移与调用约定；
  - 新增 `crates/scoop_runtime` 集成测试：对对象头与 type descriptor 的关键偏移做断言（与 codegen 侧一致）。
- 依赖：T0908、T0907

### T1502 [TODO] 编译器：GC 指针类型统一为 `addrspace(1)` 并贯穿 Type→MIR→LLVM
- 描述：把所有 GC-managed ref（Any/class/interface/function/closure env/array/string/box）统一建模为 `addrspace(1)` 指针，并确保 lowering/codegen 不再把 ref 混用为 `i8*`（addrspace(0)）。
- 目标：
  - `CgTy::Ref` 与所有“引用类 builtin”（如 `String`、`Array<T>`）在 LLVM 层统一为 `addrspace(1)`；
  - `Option<Ref>` 的 niche 统一为 NULL（addrspace(1) null）；
  - 任何进入 `@Extern`/C ABI 的 ref 必须通过明确的桥接 API：短期借用用 `GC.pin/unpin` + `scoop.unsafe.Ptr<T>` 显式转换，长期持有用 `GC.handleNew/handleGet/handleDrop`；禁止把 GC 指针直接透传到 native。
- 验收：
  - 新增 `--emit-llvm` 断言用例：IR 中出现 `addrspace(1)` 的 ref 指针类型，并且 `gc.statepoint`/stackmap 可识别这些指针；
  - `cargo test --all`、`cargo run -p scoop -- test` 通过。
- 依赖：T0803、T0908、T1009

### T1503 [TODO] 编译器：接入 LLVM statepoint/stackmap 管线并为 safepoint 生成可定位的 metadata
- 描述：在 LLVM 后端启用 statepoint 机制，生成 stackmap section，并保证运行时可以用 return address 精确定位到 stackmap record（用于 roots 枚举与 relocation 更新）。
- 目标：
  - 以 `scoop_gc_safepoint_poll/enter_native/leave_native/scoop_alloc_typed` 等 runtime helper 作为 safepoint 边界；
  - LLVM pass pipeline 必须包含 `rewrite-statepoints-for-gc`，并确保最终产物包含 stackmap section；
  - 为调试提供可导出的 “return_address → record” dump（`scoop dump-stackmaps`）。
- 验收：
  - 新增 `cargo test -p scoopc --features llvm` 单测：构建一个最小 module 后，stackmap section 非空且可解析；
  - 新增 `scoop dump-stackmaps <bin>` 输出可读信息，并在 CI/fixtures 中断言包含至少 1 个 safepoint record。
- 依赖：T1502、T0804、T0806

### T1504 [TODO] 运行时：stackmap registry（main binary + 静态链接对象）与解析器
- 描述：在 C runtime 中实现 LLVM StackMap 格式解析，并提供 module 注册 API，把 stackmap records 纳入统一查询表（按 return address 索引）。
- 目标：
  - 支持主线平台格式：Mach-O（segment `__LLVM_STACKMAPS` / section `__llvm_stackmaps`）、ELF（`.llvm_stackmaps`）、COFF（`.llvm_stackmaps`）；
  - registry 必须支持多 module 合并（main binary + 多个静态库/对象），并能处理地址重定位后的绝对地址；
  - 查询结构采用“按 return address 排序数组 + 二分查找”，保证确定性与可调试性。
- 验收：
  - `crates/scoop_runtime` 新增集成测试：给定一段内嵌/模拟 stackmap 数据，解析后能按 return address 找到 record；
  - 在真实链接产物上运行 smoke：`scoop_runtime_init()` 后 registry 非空，并能定位到 `main` 中的 safepoint record。
- 依赖：T1503、T0901

### T1505 [TODO] 运行时：safepoint poll + 线程状态机（Running/Parked/InNative）与停世界协议（含 stack walking 上下文）
- 描述：实现基于 statepoint/stackmap 的 stop-the-world：线程在 `scoop_gc_safepoint_poll()` 进入 Parked，线程在 `enter_native()` 进入 InNative。**GC 必须能扫描每个线程的完整调用栈（多帧）**，因此 Parked 线程必须提供可用于 stack walking 的上下文（由 runtime/c 负责，不泄漏到 Scoop 侧）。
- 目标：
  - `scoop_gc_safepoint_poll()`：
    - fast path：无 GC 请求时零开销返回（仅做一次原子读）；
    - slow path：检测到 GC 请求后，在进入 park 前捕获该线程的 unwind 上下文到 TLS（建议 `unw_context_t` 或等价结构；实现与平台绑定，必须收敛在 `runtime/c/platform/*`），随后切换状态为 Parked 并等待 GC 完成；
    - GC 完成后清理 TLS 上下文并恢复 Running。
  - `enter_native()` / `leave_native()`：
    - `enter_native()` 必须把**该 callsite 的 live roots（stackmap locations）**复制到 TLS `native_roots` buffer，并切换状态为 InNative；复制完成后 native 代码不得长期持有对象地址（长期持有必须走 handle，见 T1510）；
    - `leave_native()` 必须清理 `native_roots` 并切回 Running（必须在异常/early return 路径下配对）。
  - 停世界协议：
    - GC 发起方设置 `gc_requested + gc_epoch`，并等待所有线程进入 Parked 或 InNative；
    - 必须提供可诊断的超时/卡死信息：打印未到达 safepoint 的线程 id、最后一次状态、以及其上次 safepoint epoch。
- 验收：
  - 新增 `crates/scoop_runtime` 多线程测试：两线程循环 poll + 一线程触发 GC，确保两线程都能被停下并恢复；
  - 新增 run-pass fixture：深调用链（至少 3 帧）下由内层触发 GC，但 root 仅存在于外层帧，GC 后仍可访问（验证“多帧 stack walking roots”而非仅顶帧）。
- 依赖：T1504、T0911、T1402

### T1506 [TODO] 运行时：用 stackmap roots 替换 shadow stack roots（Visitor API + 精确可更新 slots）
- 描述：把 GC 的根集枚举从 shadow stack 改为 stackmap：对 Parked 线程使用其 TLS 保存的 unwind 上下文做 stack walking，逐帧定位 stackmap record，并以 `void** slot` 的形式枚举所有 roots（支持移动 GC 原地更新）。
- 目标：
  - Parked 线程 roots 枚举必须扫描**完整调用栈（多帧）**：对每一帧取得 `(return_address, stack_pointer[, frame_pointer])` 并查询 stackmap record；
  - roots 枚举必须同时覆盖：栈 spill slots、callee-saved spill slots、以及任何 LLVM stackmap 记录的 indirect locations；
  - 对无法转换为可更新 `void** slot` 的 stackmap location（例如寄存器 location）必须视为编译器/管线错误并给出稳定诊断（要求 codegen + statepoint 重写把 GC 指针落到可更新的 spill slot）；
  - 对 InNative 线程，roots 枚举以 `native_roots` buffer 为根集来源；
  - 保留 pin/unpin 语义：pinned 对象既是额外 roots，又是 relocation 禁止对象。
- 验收：
  - 新增 runtime 集成测试：多帧调用链下 outer frame 的 root 可保活对象；inner frame 触发 GC 后仍可访问；
  - 在启用移动/压缩前先通过 “非移动 GC + stackmap roots” 全量回归：`cargo test --all`、`cargo run -p scoop --features llvm -- test`。
- 依赖：T1505、T0910、T0912

### T1507 [TODO] 编译器：为 class/interface/box/closure env/array/string 生成 type descriptor（trace bitmap + dispatch tables）
- 描述：在编译期为所有可分配 ref 类型生成 `ScoopTypeDescriptor` 常量，并在对象分配时写入对象头 `type_desc`，使 GC 能扫描对象内部引用字段并支持动态分发。
- 目标：
  - class：包含所有实例字段（含继承）引用位图；生成 vtable（含 override slot）；生成 itable（按 interface id）；
  - interface：分配全局稳定 interface id，并生成 method slot 表；
  - box：值类型装箱为 ref（Any/interface）时使用 box type descriptor，box payload 的 trace bitmap 由被装箱类型决定；
  - closure env：capture 字段的 trace bitmap 必须覆盖所有 ref captures（含 CaptureBox）；
  - array/string：descriptor 必须包含 element trace 规则（ref element 时扫描每个元素），并正确处理变长对象 size。
- 验收：
  - 新增 `scoop dump-rtti` 输出：列出每个 type 的 `type_id`、父类、interfaces、vtable slots、trace bitmap；
  - 新增 run-pass fixtures：class 持有 ref 字段 + GC 后仍可访问；array<string> 分配 + GC 后元素仍正确。
- 依赖：T1501、T1502、T0907、T0401、T0512

### T1508 [TODO] 类型检查与 lowering：普通成员函数调用（class/interface）+ 动态分发分类
- 描述：把 `receiver.method(args...)` 从“存在性解析”升级为可执行语义：typecheck 完成 overload resolution，lowering 把调用分类为 direct/virtual/interface，并在 MIR 中显式表达。
- 目标：
  - class：非 override 的 `final`/private 可 direct call；可 override 的走 vtable；
  - interface：走 itable；
  - 支持 `super.method()`（按 Kotlin-like 规则）与 `override` 的一致性校验（签名 + effect row containment）；
  - `this` 与构造阶段调用规则必须保持 Appendix B.2.2 语义一致。
- 验收：
  - 新增 typecheck fixtures：member call 的 overload resolution、override 冲突、interface dispatch 的 pass/fail；
  - 新增 run-pass fixtures：class override 调用输出正确；interface 引用调用输出正确。
- 依赖：T0319、T0440、T1312、T1507

### T1509 [TODO] LLVM codegen：typed alloc + 字段访问 + 动态分发（vtable/itable）端到端
- 描述：在 LLVM 后端实现引用类型的端到端 codegen：对象分配、字段读写、方法调用（direct/virtual/interface）、以及 `is/as/as?` 的运行期检查调用。
- 目标：
  - 所有对象分配统一走 `scoop_alloc_typed(type_desc, size_bytes)`，并写入对象头 type_desc（fixed-size 传常量 size；变长对象传计算后的 size）；
  - 字段访问必须使用编译期计算的 offset（与 type descriptor 一致），并在写 ref 字段时满足 GC 的写入约束（STW 下可无 barrier，但必须保证 slot 写入是可被 GC 扫描的 `void**`）；
  - virtual/interface 调用必须使用 vtable/itable slot，调用约定固定为：`fn(this, ...args) -> ret`；
  - `is/as/as?` 调用必须落到 runtime type test（例如 `scoop_rtti_is_instance_of/scoop_rtti_cast`），并与 nullability/pointer-niche 规则一致。
- 验收：
  - 新增 run-pass fixtures：class 分配 + 字段 set/get + override call；interface 调用；`is/as/as?` 的 pass/fail；
  - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo test -p scoopc --features llvm` 通过。
- 依赖：T1508、T1503、T1507

### T1510 [TODO] GC 与 `@Extern`/FFI：native 过渡协议（roots 保存 + pin/handle）完整落地
- 描述：把 `@Extern` 调用纳入 GC 协议：编译器自动插入 `enter_native/leave_native`，并为“把对象指针交给 native 并跨 safepoint 存活”的场景提供稳定方案（pin/unpin + stable handle）。
- 目标：
  - `@Extern` 调用前必须 `enter_native`，调用后必须 `leave_native`（异常/early return 也要保证配对）；
  - 若 native 需要长期持有引用：必须走稳定 handle（`GC.handleNew/handleGet/handleDrop`），而不是把对象地址长期暴露给 native；pin/unpin 仅作为低层特权 API（例如短期借用或与外部 ABI 互操作）；
  - 禁止把 `addrspace(1)` GC 指针直接当作 `Ptr<T>` 传给 native（必须显式转换）。
- 验收：
  - 新增 unsafe_nogc/typecheck fixtures：禁止错误的 ref→Ptr 隐式转换；
  - 新增 run-pass fixture：调用一个模拟 extern（可在 runtime/c 提供测试符号）在 native 内部触发 GC，仍能正确访问对象。
- 依赖：T1505、T1008、T1009、T1006

### T1511 [TODO] 移动 GC：stackmap spill slots 与 heap 引用字段的统一更新（为 Immix compaction 做前置）
- 描述：实现移动 GC 的“指针修复闭环”：对象搬迁后，必须更新所有 roots（stackmap spill slots + native_roots）与 heap 内引用字段，使程序在恢复执行后无悬挂指针。
- 目标：
  - relocation 更新点：stackmap spill slots（按 record locations 写回）、native_roots buffer（逐槽写回）、stable handle 表（handle->obj 指针写回，见 T1510）、heap 内部字段（trace visitor 以 `void** slot` 形式可更新）；
  - 与 pin/unpin 一致：pinned 对象不搬迁；引用 pinned 对象的 slot 仍可正常扫描；
  - GC 结束后，所有线程恢复执行必须只通过 relocated pointers 继续（statepoint + gc.relocate 读取到的新值）。
- 验收：
  - 新增 `runtime_gc` fixtures：强制触发移动（可通过 env/flag），并验证对象地址变化后引用仍正确；
  - 在启用 Immix compaction 前，baseline moving 模式下同样一组 fixtures 通过（作为 T1407 的前置安全网）。
- 依赖：T1506、T1510

### T1512 [TODO] 回归与压力测试：ref types + stackmap GC 全量覆盖矩阵
- 描述：为引用类型与 GC 建立持续回归套件：覆盖 class/interface/closure/array/string/box、跨线程、native 过渡、pin/unpin、moving/compaction，并为每类提供最小 pass/fail + run-pass。
- 目标：
  - `tests/fixtures/runtime_gc/*`：新增按主题分组的 fixtures（roots 枚举、对象内引用扫描、循环引用、跨线程、native 过渡、pin/unpin、moving）；
  - `scoop test` 支持 `--gc-stress` / `--gc-move` / `--threads=N` 指令驱动（由 runner 注入 env/args）；
  - 每个特性至少 1 个 compile-fail（非法用法/unsafe 违规）与 1 个 run-pass（正确行为）。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `PATH=\"/opt/homebrew/opt/llvm@18/bin:$PATH\" cargo run -p scoop --features llvm -- test`
- 依赖：T1509、T1511

### T1513 [TODO] String 表示升级方案：`struct String` + `class StringData` backing（零拷贝 slicing/substring）
- 描述：评估并落地把 `String` 从“单一 ref class”升级为“薄 value struct + 共享 back buffer”的表示，以支持零拷贝 slicing/substring（例如 `trimStart`/`trimEnd`/`trim` 等仅调整边界的操作）。
- 目标：
  - 语言层语义（必须写进规范）：
    - `String` 仍是不可变、值语义（== 按内容；hash 按内容；copy 不共享可变状态）。
    - `String` 必须可表示“子串视图”：多个 `String` 可以共享同一个 `StringData`，但拥有不同的 `(start, len)`。
    - `trimStart/trimEnd/trim/substr/slice` 的返回值优先为 **O(1) 零拷贝**（仅调整 `(start, len)`），仅在必要时（例如需要重新编码/规范化）才分配新 `StringData`（是否需要规范化必须在 spec 中给出明确规则，而不是实现自选）。
  - 运行时布局（必须写进运行时文档）：
    - `struct String { data: StringData, start: Int, len: Int }`（字段名可不同，但语义必须等价；`data` 为 GC-managed ref）；
    - `class StringData` 为不可变 back buffer：包含字节存储（建议 UTF-8）与长度，并提供“取子范围”的边界检查辅助；
    - 明确“String 视图”如何向 native 暴露字节：短期 raw pointer 必须 pin；长期存活必须用 stable handle（复用 §15.10/§15.10.1 的规则）。
  - 编译器/GC 约束（必须写进计划/验收）：
    - `String` 作为 value type 含有 ref 字段，stackmap roots 与 heap trace 必须都能正确保活 `StringData`；
    - 将 `String` 作为 `Any`/interface 时触发 boxing（分配 box 对象并携带 type descriptor），不得破坏零拷贝 slice 语义。
- 验收：
  - `SCOOP_FULL_SPEC.md` §8 增补 `String` 表示与 slicing/substring/trim 语义（包含：共享/边界/复杂度/是否分配的判定规则）。
  - `SCOOP_RUNTIME.md` 增补 `String`/`StringData` 的内存布局与 GC/FFI 约束（pin/handle）。
  - `STDLIB_DESIGN.md` 的 `scoop.text` 分层补充：哪些 API 必须零拷贝、哪些 API 允许分配（作为性能契约的一部分）。
- 依赖：T1501、T1502、T1507、T1510、T1511

### T1514 [TODO] String 表示落地：sysroot + runtime + codegen + fixtures（按 T1513 规范）
- 描述：按 T1513 的规范，把 `String` 落地为 value struct + shared backing，并补齐 `trimStart/trimEnd/trim/substr/slice` 的端到端实现与回归。
- 目标：
  - sysroot：调整 `String` 的声明与最小 API（`length`/`get`/`slice`/`trim*`/`toString`/`hashCode` 等），并引入 `StringData`（仅 runtime 可构造）。
  - runtime/c：实现 `StringData` 分配与字节存储、字符串字面量/拼接（若已支持）、以及输出/比较/哈希等底层原语（按“ABI 最小化”原则集中导出）。
  - codegen：字符串字面量与基础字符串操作生成正确布局的 `String` 值；跨函数传参/返回与闭包捕获不破坏 `(data,start,len)`。
  - GC：`StringData` 的 type descriptor trace bitmap 正确；moving/compaction 后 `String.data` 的指针被正确更新（含栈/堆/handles/native_roots）。
- 验收：
  - 新增/更新 run-pass fixtures：`trimStart/trimEnd/trim` 输出正确；`slice/substr` 在共享 backing 下语义正确；
  - 新增 runtime_gc fixtures：在 `--gc-stress` 与 `--gc-move` 下大量 substring/trim 不产生悬挂指针；
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test` 通过。
- 依赖：T1513、T1512
