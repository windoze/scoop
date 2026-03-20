# TODO（Scoop 0.1：按“可单独实现 & 单独验证”的小步拆分）

> 生成时间：2026-03-18  
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
- 描述：`scoop test` 与 `scoop dump-ast` 可用；`build/run` 目前为占位错误提示。
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

### T0253 [TODO] Parser：use-site effect row 实参 `Type<eff Row>`（spec §3.4 / §5.8）
- 描述：在类型实参列表中支持 `Disposable<eff Async>`、`Disposable<eff (Async + Raise<IOError>)>` 这类 use-site effect row 参数写法。
- 目标：先只支持单个 `eff` clause，且必须出现在泛型实参列表最后；合法性检查留给 typecheck。
- 验收：新增 parse fixture：`Disposable<eff Pure>`、`Disposable<eff (Async + Raise<IOError>)>` 可解析；`<eff E, Int>` 之类非法顺序报错。
- 依赖：T0250、T0219

### T0254 [TODO] Parser：import alias `import foo.bar.Baz as Qux`（Appendix B.7）
- 描述：扩展 import 语法，支持 Kotlin 风格 alias import，并把 alias 名记录到 AST。
- 目标：先只支持顶层 import；不支持在表达式/局部作用域中出现 import。
- 验收：新增 parse fixture：普通 import、`*` import、alias import 混用可解析；缺 alias 名时报错。
- 依赖：T0009

### T0255 [TODO] Parser：pattern rest `..`（spec §4.2）
- 描述：在 pattern 语法中支持 `..`，用于忽略剩余字段/元素。
- 目标：先只把 `..` 解析进 AST；类型规则与“只能出现一次”等约束留给 typecheck。
- 验收：新增 parse fixture：tuple/struct pattern 中的 `..` 可解析；重复 `..` 或非法位置报错。
- 依赖：T0239、T0241、T0240

### T0256 [TODO] Parser：class `init { ... }` blocks（Appendix B.2.2）
- 描述：在 class body 中解析 `init { ... }` 初始化块，并把它作为成员节点纳入 AST。
- 目标：先只支持 class；初始化顺序与语义留给 resolver/typecheck。
- 验收：新增 parse fixture：含多个 `init` block 的 class 可解析；`init` 缺 block 报错。
- 依赖：T0207、T0201、T0248

### T0257 [TODO] Parser：secondary constructors（Appendix B.2.2）
- 描述：支持 class 内 `constructor(...) { ... }` 的最小语法，并保留可选 delegation call（如 `: this(...)` / `: super(...)`）的 span/AST。
- 目标：先只解析签名、delegation 头和 body；初始化顺序与调用合法性留给 typecheck。
- 验收：新增 parse fixture：含 secondary constructor 的 class 可解析；缺参数列表或缺 body 报错。
- 依赖：T0248、T0207

### T0258 [TODO] Parser：`object` / `companion object` 声明（Appendix B.9）
- 描述：支持 top-level / nested `object Name { ... }`，以及 class 内 `companion object { ... }` / `companion object Name { ... }`。
- 目标：先只做语法与 AST；单例语义、成员访问和初始化留给后续阶段。
- 验收：新增 parse fixture：top-level object、nested object、named/unnamed companion object 可解析；非法 companion 位置报错。
- 依赖：T0201、T0248

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

### T0309 [TODO] Resolver：泛型参数作用域与解析（type params 是符号）
- 描述：在 resolve 阶段把声明处 type params 纳入作用域，使 `TypeRef` 中的 `T` 可解析到泛型参数。
- 目标：先只支持同一声明内引用；不支持 where 约束。
- 验收：resolve fixture：`fun id<T>(x: T): T { x }` 通过；`fun f(x: T) {}` 报未定义类型参数。
- 依赖：T0218、T0308

### T0310 [TODO] Resolver：成员访问解析（`.`）绑定到字段/方法/属性
- 描述：把 `a.b` 的 `b` 绑定到 struct 字段或 class/interface 成员（先做存在性）。
- 目标：先只处理“静态可确定”的情况；动态分发/override 后续。
- 验收：resolve fixture：`p.x` 解析到字段；`p.m()` 解析到方法；不存在时报错并指向成员名 span。
- 依赖：T0302、T0210

### T0311 [TODO] Resolver：调用解析（把 `Call(Ident)` 绑定到具体函数）
- 描述：把 `f(...)` 的 callee 从“裸 ident”解析为某个 fun symbol（先要求唯一匹配）。
- 目标：先不支持重载；若同名多个定义则报歧义错误。
- 验收：resolve fixture：调用顶层函数成功；同名多个函数时报 `ambiguous_call`（新错误码）。
- 依赖：T0305、T0209

### T0312 [TODO] Resolver：扩展函数/扩展属性的分发优先级（spec §7.4 / §10.3）
- 描述：实现最小规则：member 优先于 extension；extension 需要 receiver 类型可匹配。
- 目标：先只在同包/同文件找 extension；跨包 import 扩展后续。
- 验收：resolve fixture：同名 member 与 extension 并存时解析到 member；只有 extension 时解析到 extension。
- 依赖：T0233、T0310

### T0313 [TODO] Resolver：`this`/构造参数/成员初始化作用域（class 场景）
- 描述：为 class 主构造参数、属性初始化表达式、成员函数体建立正确作用域（含 `this`）。
- 目标：先不实现 `super`；先不处理 capture/闭包。
- 验收：resolve fixture：class 成员函数可引用 `this` 与构造参数；未定义时报错。
- 依赖：T0248、T0308

### T0314 [TODO] Resolver：收集 `typealias` 并纳入 type 命名空间（为 sysroot 标准别名铺路）
- 描述：把 `typealias Name = Type` 作为一种 type-level symbol 纳入索引与 import 环境，使得 `Byte/UIntPtr` 等别名能被当作类型引用。
- 目标：resolve 阶段只做“名字可见性/冲突检测”；不做 alias 展开与循环检测（交给 typecheck）。
- 验收：新增 resolve fixture：`typealias Byte = UInt8; fun f(x: Byte): Byte { x }` 可解析；同名 typealias 与 struct/class 冲突时报错并定位两个声明。
- 依赖：T0251、T0301、T0308

### T0315 [TODO] Resolver：import alias 绑定与冲突规则（Appendix B.7）
- 描述：把 `import foo.bar.Baz as Qux` 引入的 alias 纳入 import table，并参与 type/value 名字解析与冲突检查。
- 目标：先只支持文件级 alias；同名 alias 与本地顶层声明/其他 import 冲突时报错。
- 验收：新增 resolve fixture：通过 alias 成功引用类型/函数；alias 冲突时报稳定错误码。
- 依赖：T0254、T0303、T0308

### T0316 [TODO] Resolver：class 初始化阶段作用域（property initializer / `init` / secondary constructor）
- 描述：为属性初始化表达式、`init` block、secondary constructor 建立正确作用域，固定 `this`、主构造参数、已声明成员的可见性边界。
- 目标：先只做名字解析与作用域；初始化顺序与必经 delegation 规则留给 typecheck。
- 验收：新增 resolve fixture：`init` 中可引用 `this` 与构造参数；非法前向引用报错并定位。
- 依赖：T0256、T0257、T0313

### T0317 [TODO] Resolver：`object` / `companion object` 的名字解析与成员访问（Appendix B.9）
- 描述：把 `object` / `companion object` 纳入符号表，并支持 `Obj.member`、`ClassName.member`（经 companion）这类最小解析规则。
- 目标：先只做 name resolution；单例初始化与 codegen 留给后续。
- 验收：新增 resolve fixture：top-level object 成员可解析；class companion 成员可通过 `ClassName.member` 解析；缺 companion 时给出清晰错误。
- 依赖：T0258、T0302、T0301

---

## T04：类型系统（阶段 3：先类型检查再优化）

### T0401 [TODO] Type 表示：建立 `scoopc::ty` 模块（TypeId/TypeKind）
- 描述：引入内部类型表示（区分引用/值类型），并支持 builtin（Any/Option/Nothing/Unit 以及内建整数族 Int/UInt/IntN/UIntN 等）。
- 目标：先只建数据结构与打印；不做推断/求解。
- 验收：新增单测：构造若干 Type 并格式化输出；`cargo test -p scoopc` 通过。
- 依赖：T0010

### T0402 [TODO] 从 sysroot 收集“内建类型/效果”的类型信息
- 描述：基于 sysroot AST 建立 type env（Any/Option/Raise），为后续 typecheck 提供起点。
- 目标：先只读取声明头（名字 + kind + 泛型参数个数），不做方法体。
- 验收：新增单测：加载 sysroot 后能查询到 `scoop.core.Option` 的泛型参数数量为 1。
- 依赖：T0010、T0401

### T0403 [TODO] `TypeRef` → `Type` lowering（支持 Path/Tuple/Nullable）
- 描述：把 AST 的 `TypeRef` 解析到内部类型（已 resolve 的前提下）。
- 目标：先做“存在性 + arity 检查”；不做 variance/star projection。
- 验收：新增 typecheck fixture：`fun f(x: Option<Any>): Any {}` 通过；`Option<Any, Any>` 报 arity 错误（新错误码）。
- 依赖：T0011、T0402

### T0404 [TODO] 类型检查 pass：仅检查顶层声明头（fun/val/type）签名合法
- 描述：实现 `typecheck::check_file_headers`，不进入函数体。
- 目标：先把“类型环境 + 错误诊断”跑通；不要求表达式 AST 完整。
- 验收：新增 `tests/fixtures/typecheck/`：至少 2 个 pass + 2 个 fail；在 `scoop test` typecheck phase 下回归。
- 依赖：T0101、T0403

### T0405 [TODO] 表达式类型检查 v0：字面量（Int/String/Bool/Unit）
- 描述：为 `Expr::IntLit/StringLit/...` 推导类型。
- 目标：先把 builtin 类型补到 sysroot（或在 compiler 内建）；不做数值提升。
- 验收：新增 typecheck fixture：`val x = 1` 推导为 Int（若支持推断）；或要求注解 `val x: Int = 1`。
- 依赖：T0206、T0401、T0418

### T0406 [TODO] 表达式类型检查 v0：变量引用（局部/参数/顶层）
- 描述：对 resolve 后的 ident 引用给出类型。
- 目标：先不支持 forward reference（或明确规则）；错误信息指向引用处。
- 验收：typecheck fixture：`fun f(x: Any) { val y = x }` 通过；`val y = missing` 报未定义符号（若 resolve 已报则这里不重复/或只报一次）。
- 依赖：T0305、T0405

### T0407 [TODO] 表达式类型检查：函数调用（无重载、按名称唯一解析）
- 描述：对 `Call(callee, args)` 做参数数量检查与类型匹配。
- 目标：先只支持调用“已解析到的 fun symbol”；不支持默认参数/命名参数。
- 验收：typecheck fixture：调用参数个数不匹配时报错（含错误码）；参数类型不匹配时报错并指出 arg span。
- 依赖：T0209、T0305、T0406

### T0408 [TODO] 表达式类型检查：成员访问 `a.b`（仅 struct 字段）
- 描述：先实现 value type `struct` 的字段访问类型检查（spec §2.3.1）。
- 目标：不支持 class/interface vtable；只支持直接字段。
- 验收：新增 typecheck fixture：定义 struct `Point(val x: Int)` 并访问 `p.x` 通过；访问不存在字段报错。
- 依赖：T0210、T0401、T0404

### T0409 [TODO] 声明类型：struct（仅字段，不含方法）
- 描述：typecheck 阶段收集 struct 字段列表、检查重复字段、类型合法。
- 目标：先限制字段全是 `val` 且需要类型注解；不支持默认值。
- 验收：typecheck fixture：struct 字段重复时报错；字段类型未解析时报错。
- 依赖：T0202、T0404

### T0410 [TODO] 值类型：tuple 与 Unit（spec §2.3.3）
- 描述：把 tuple 类型与 tuple 表达式加入类型系统；`Unit` 视为 0 元 tuple。
- 目标：先只支持 `(A, B)` 类型与 `(a, b)` 表达式；不支持解构。
- 验收：typecheck fixture：`val t: (Int, Int) = (1, 2)` 通过；元素类型不匹配报错。
- 依赖：T0211、T0405

### T0411 [TODO] Nullability：`T?` 作为 `Option<T>` 语法糖（spec §2.4）
- 描述：在 lowering 阶段把 `Nullable(TypeRef)` 映射到 `Option<...>`。
- 目标：先只做类型层映射；运行期表示后续 codegen 决定。
- 验收：typecheck fixture：`val x: Int?` 等价于 `Option<Int>`；`val y: Any?` 也可。
- 依赖：T0403、T0402

### T0412 [TODO] Cast 语义：`as`/`as?` 的类型规则（spec §4.4）
- 描述：实现 `as`/`as?` 的类型检查规则；运行期失败行为由 effect/RuntimeError 后续落地。
- 目标：先只做静态规则（可 cast/不可 cast）；不做 smart cast。
- 验收：typecheck fixture：`x as T` 类型为 `T`；`x as? T` 类型为 `T?`（即 Option<T>）。
- 依赖：T0213、T0411

### T0413 [TODO] `is`/`!is` + smart cast（val 场景）（spec §4.3）
- 描述：实现 flow-sensitive 类型收窄：`if (x is T) { x /* as T */ }`。
- 目标：先只支持不可变 `val` 与参数；不支持 `var` 与复杂控制流合流。
- 验收：typecheck fixture：在 `if` then 分支内使用 `x` 视为 `T`；在 else 分支保持原类型。
- 依赖：T0213、T0214、T0406

### T0414 [TODO] `when` 类型规则：分支 LUB（spec §14.6）
- 描述：为 `when` 表达式计算结果类型（各分支类型的 LUB）。
- 目标：先只支持简单类型：相同类型则返回该类型，否则 fallback 到 `Any`（后续再做真正 LUB）。
- 验收：typecheck fixture：`when { ... }` 各分支 Int/Int → Int；Int/String → Any（或报错，按设计）。
- 依赖：T0215、T0405

### T0415 [TODO] 值类型更新：`with` 表达式类型检查与 path 校验（spec §2.6）
- 描述：检查 `with` 的 base 必须是 struct/tuple/enum（按设计）；path 必须存在且 RHS 类型匹配。
- 目标：先只支持 struct 字段更新；嵌套 path 可分后续任务。
- 验收：typecheck fixture：`p with { x: 1 }` OK；`p with { missing: 1 }` 报错并指向 path。
- 依赖：T0216、T0409、T0408

### T0416 [TODO] 变量绑定规则：`val/var` 赋值与重定义检查（spec §9）
- 描述：typecheck 阶段检查 `var` 可赋值、`val` 不可再次赋值；同一作用域重复定义报错。
- 目标：先只覆盖 block 内；不涉及闭包捕获与跨块。
- 验收：typecheck fixture：`val x = 1; x = 2` 报错；`var x = 1; x = 2` 通过。
- 依赖：T0227、T0304、T0405

### T0417 [TODO] 基础控制流语句：`return`（函数内）
- 描述：解析并类型检查 `return expr?`，并校验返回类型。
- 目标：先不支持 non-local return（spec §7.3）；只支持普通函数。
- 验收：typecheck fixture：返回类型不匹配时报错；`return` 在非函数体报错（若 parser 允许则 typecheck 报）。
- 依赖：T0226、T0404、T0405

### T0418 [TODO] sysroot：补齐内建标量类型（整数体系 + 标准别名 + Bool/String/Unit/Nothing）（spec §2.3.4 / runtime §3）
- 描述：在 sysroot 中提供“内建标量类型的可见声明”，包括：
  - word-sized：`Int` / `UInt`（随 target 指针宽度变化）
  - fixed-width：`Int8/16/32/64`、`UInt8/16/32/64`
  - 标准别名：`Byte/Short/UShort/Long/ULong`，以及 `UIntPtr = UInt`
  - 其他最小基石：`Bool`、`String`、`Unit`、`Nothing`
- 目标：只做“声明层”：类型名/可见成员最小化；不要求标准库实现齐全；不引入任何运行期行为。
- 验收：新增 resolve fixture：`import scoop.core.*` 后引用上述类型与别名都可解析；`scoop test` 通过。
- 依赖：T0010、T0011、T0251、T0314

### T0419 [TODO] sysroot：补齐 `RuntimeError` 与相关枚举值（spec §5.7 / §4.4 / Appendix B.3.3）
- 描述：新增 `enum RuntimeError { ClassCastFailed, NullAssertionFailed, ... }`（按 spec）并确保可在 Raise<RuntimeError> 中使用。
- 目标：先只定义错误枚举；不实现打印/堆栈。
- 验收：新增 resolve fixture：引用 `RuntimeError.NullAssertionFailed` 可解析；typecheck fixture：`Raise<RuntimeError>` 类型合法。
- 依赖：T0418、T0402

### T0420 [TODO] 类型关系：`Nothing` 作为 bottom type（spec §2.1/§5.7）
- 描述：在类型系统中实现 `Nothing <: T`（对任意 T），用于 `Raise.raise`、`return`、不可达分支等。
- 目标：先只实现 `Nothing` 子类型规则；不实现完整子类型系统。
- 验收：typecheck fixture：`fun f(): Any { Raise.raise(e) }` 允许 body 类型为 Nothing 兼容 Any 返回。
- 依赖：T0419、T0602

### T0421 [TODO] `!!`：not-null assertion 的类型与效果要求（Appendix B.3.3）
- 描述：`x!!`：若 `x: T?`，则结果为 `T`，并要求 `Raise<RuntimeError>`（除非被 handle/try 处理）。
- 目标：先只实现静态规则；运行期行为后续由 effect/runtime 落地。
- 验收：typecheck/effects fixture：在 `/ Pure` 的函数里使用 `x!!` 报 required effect；在 try/catch 内通过。
- 依赖：T0212、T0419、T0604、T0607

### T0422 [TODO] `?.` safe-call 与 `?:` Elvis 的类型规则（Appendix B.3.1/3.2）
- 描述：`x?.m()` 返回 `R?`；`x ?: y` 的结果类型为 `T`（若 y: T）。
- 目标：先只覆盖 Option<T>（nullable sugar）；不引入真正的 null 值。
- 验收：typecheck fixture：`val y: Int? = x?.len()` 合法；`val z: Int = x ?: 0` 合法。
- 依赖：T0229、T0411、T0407

### T0423 [TODO] struct literal 的类型检查（字段存在性/类型匹配）
- 描述：检查 `Point { x: 1, y: 2 }`：字段必须存在、不可重复、类型匹配、必填字段覆盖规则（按设计）。
- 目标：先只支持所有字段都必须提供的模式；默认值/可选字段后置。
- 验收：typecheck fixture：缺字段/多字段/重复字段都报错并定位到字段名或逗号位置。
- 依赖：T0224、T0409、T0405

### T0424 [TODO] `with`：嵌套 path 与并行求值语义（spec §2.6）
- 描述：支持 `p with { a.b: v }` 的嵌套更新，并保证 RHS 基于“原值并行求值”（无顺序依赖）。
- 目标：先只实现 typecheck 侧的规则与必要诊断；真正 lowering 放到 IR 阶段单独任务。
- 验收：typecheck fixture：嵌套字段类型不匹配时报错；同一字段多次更新报错或明确覆盖规则（需决定）。
- 依赖：T0415

### T0425 [TODO] 声明类型：enum（rich enum）类型表示与收集（spec §2.3.2）
- 描述：在 type env 中加入 enum variant 信息（tag + payload types），并检查重复 variant/字段。
- 目标：先只支持 enum variant（无方法/属性）；niche 优化后置。
- 验收：typecheck fixture：enum 重复 variant 名报错；variant 字段类型未解析报错。
- 依赖：T0236、T0404

### T0426 [TODO] 枚举构造表达式：`Some(x)` 的类型检查（spec §4）
- 描述：把 `Some(x)` 解析/绑定为某个 enum variant 构造，并检查参数数量与类型。
- 目标：先只支持同名唯一的 variant；重名/重载后续处理。
- 验收：typecheck fixture：`val o: Option<Int> = Some(1)` 通过；`Some()` 参数数不对时报错。
- 依赖：T0240、T0311、T0425

### T0427 [TODO] `when`：variant pattern 与 tuple pattern 的类型检查（spec §4）
- 描述：对 pattern 进行类型约束：variant pattern 仅用于 enum；tuple pattern 仅用于 tuple；绑定变量进入分支作用域。
- 目标：先不做穷尽性；先只做“每个分支内部类型正确”。
- 验收：typecheck fixture：`when(opt){ Some(x)->x; None->0 }` 通过；把 Some 用在非 enum 上时报错。
- 依赖：T0243、T0426、T0410

### T0428 [TODO] `when`：穷尽性检查（enum/Bool/Option）与 else 规则（spec §4.1）
- 描述：对可穷尽类型要求覆盖所有 variant（或允许 else）；非穷尽类型必须有 else/_。
- 目标：先只支持 enum 与 Bool 与 Option<T>；嵌套组合后续。
- 验收：typecheck fixture：缺少 None 分支时报错；覆盖完整且仍写 else 时产生 warning（先可仅记录 warning，不必 fixtures 断言）。
- 依赖：T0427

### T0429 [TODO] pattern guard：带 `if` 的分支视为非穷尽（spec §4.1/§4）
- 描述：当某个分支带 guard 时，穷尽性检查应要求 else/_（或把该分支不计入覆盖）。
- 目标：先只实现规则；不做路径敏感分析。
- 验收：typecheck fixture：`Some(x) if x>0 -> ...` 场景缺 else 时报错。
- 依赖：T0428

### T0430 [TODO] destructuring `val`：tuple/struct pattern 绑定（spec §4.2、§9）
- 描述：实现 `val (a,b)=expr`、`val Point { x, y } = expr` 的类型检查与绑定，并强制 `var` 不允许。
- 目标：先只支持 tuple/struct；enum destructuring 可复用 when pattern 后续再补。
- 验收：typecheck fixture：`var (a,b)=...` 报错；绑定变量类型正确；字段重命名后变量名类型正确。
- 依赖：T0244、T0410、T0409

### T0431 [TODO] 属性 v0：class 属性声明头与 backing field 规则（spec §10.1）
- 描述：实现 class 属性的类型检查：默认 getter/setter、`field` 可见性、是否生成 backing field 的判定。
- 目标：先只做静态规则与诊断；不做 codegen。
- 验收：typecheck fixture：setter 中引用 `field` 合法；未生成 backing field 的 computed property 禁止引用 `field`。
- 依赖：T0234、T0404

### T0432 [TODO] 属性 v1：value type（struct/enum）仅允许 getter-only computed（spec §10.2）
- 描述：对 struct/enum 中的属性限制：禁止 setter；禁止 backing field。
- 目标：先只在 typecheck enforce；parser 仍可解析。
- 验收：typecheck fixture：struct 内 `var` 属性或 setter 报错；getter-only 通过。
- 依赖：T0431、T0409、T0425

### T0433 [TODO] 扩展属性：必须 computed（无 backing field）（spec §10.3）
- 描述：实现 extension property 的规则：不能有 initializer/field；编译模型为静态 getter/setter。
- 目标：先只做静态检查；lowering 到函数在 IR 阶段做。
- 验收：typecheck fixture：`val String.lastChar get() = ...` 通过；写 initializer 报错。
- 依赖：T0233、T0234

### T0434 [TODO] 委托属性：解析后类型规则与最小 lowering 计划（spec §10.4）
- 描述：检查 delegated property：只能用于 class；delegate 必须实现 `getValue/setValue`；生成 `PropertyMeta` 参数（编译期元数据）。
- 目标：先只做静态规则与诊断；真正生成 `$delegate` 字段与转发函数留给 lowering。
- 验收：typecheck fixture：struct/enum 里写 `by` 报错；class 里写 `by` 需要满足接口（可先只检查方法名存在性）。
- 依赖：T0235、T0431、T1208

### T0435 [TODO] 函数类型（含 receiver + effects）的类型表示与子类型规则（spec §7.5、§5.8）
- 描述：在 `ty` 中加入 FunctionType：参数/返回/receiver/effect row，并定义最小子类型关系（参数逆变/返回协变 + effect row containment）。
- 目标：先只支持无泛型的函数类型；完整子类型与推断后续补齐。
- 验收：typecheck fixture：`val f: (Any)->Any / Pure = ...`；effect row 不满足时报错（或 defer 到 T06）。
- 依赖：T0219、T0401、T0608

### T0436 [TODO] 扩展函数：静态分发与 receiver 作为第一个参数（spec §7.4）
- 描述：typecheck 阶段将 extension fun 视为普通函数（receiver 第一个参数），并实现最小分发规则（member 优先）。
- 目标：先不支持同名多个 extension 的重载；歧义时报错。
- 验收：typecheck fixture：`fun Any.id(): Any { this }` 可被调用 `x.id()`；解析到 extension。
- 依赖：T0233、T0312、T0407

### T0437 [TODO] 泛型：声明处变型 `in/out` + star projection（spec §3.2~§3.3 / Appendix B.4）
- 描述：在 parser/type system 中支持 `in T`/`out T` 与 `*`，并实现最小合法性检查。
- 目标：先只解析并存储 variance/star；子类型规则可先限制为“只对引用类型参数生效”（按 spec）。
- 验收：typecheck fixture：`interface ReadOnlyProperty<in T, out V>` 可解析并 typecheck；非法 variance 位置报错。
- 依赖：T0249、T0401

### T0438 [TODO] 声明类型：class 的最小类型检查（字段/构造参数/方法头）
- 描述：实现 class：主构造参数作为字段（`val/var`）与成员方法头解析后的类型收集。
- 目标：先不实现继承/override；先把“类有成员”这件事跑通。
- 验收：typecheck fixture：`class User(val name: String) { fun get(): String { name } }`（按语法）通过。
- 依赖：T0248、T0203、T0404

### T0439 [TODO] 继承与 override：open/abstract/sealed + override 检查（Appendix B.2）
- 描述：实现最小规则：class 单继承；override 必须显式；被覆盖成员需 open/abstract；sealed 限制同编译单元。
- 目标：先只做静态检查；vtable/codegen 后置。
- 验收：typecheck fixture：override 缺失时报错；override 目标不是 open时报错；sealed 跨文件继承时报错（需多文件单元，见 T0307）。
- 依赖：T0438、T0245、T0307

### T0440 [TODO] interface：多实现与默认方法的限制策略（spec §2.2.2）
- 描述：实现 interface 声明收集与实现列表检查；默认方法可先允许但不要求 codegen。
- 目标：先只在 typecheck 检查签名一致性；冲突规则后续。
- 验收：typecheck fixture：class 实现 interface 并提供方法通过；缺少方法时报错。
- 依赖：T0438、T0439

### T0441 [TODO] Boxing：值类型装箱到 `Any`/interface（spec §2.5）
- 描述：实现“语义正确”的 boxing：当值类型被当作 `Any`/interface 使用时生成 box（类型系统层先建模）。
- 目标：先只在 typecheck 允许/禁止；真正分配与布局留给 codegen/runtime。
- 验收：typecheck fixture：`val a: Any = 1`（若 Int 是值类型）通过；`val i: IFoo = Point(...)`（若实现）按规则通过/报错。
- 依赖：T0418、T0405、T0440

### T0442 [TODO] 语句/循环的类型检查：`while`/`break`/`continue`
- 描述：检查 while 条件为 Bool；break/continue 必须在循环内；循环体类型规则明确（Unit）。
- 目标：先不支持 label；不支持 for。
- 验收：typecheck fixture：`while(1){}` 报错；`break` 在函数顶层报错；合法 while 通过。
- 依赖：T0228、T0405

### T0443 [TODO] 赋值类型检查：lhs 可写性（var）与类型匹配（spec §9）
- 描述：实现 `x = y`：x 必须是 var 绑定或可写属性；y 类型必须可赋给 x。
- 目标：先只支持局部 var 与字段/属性（若已实现）；复合赋值后置。
- 验收：typecheck fixture：给 val 赋值报错；给 var 赋值但类型不匹配报错（指向 rhs span）。
- 依赖：T0227、T0416、T0406

### T0444 [TODO] `inline` 与 non-local return 的语义门禁（spec §7.2/§7.3）
- 描述：实现最小检查：只有 inline 函数的 lambda 参数允许 non-local return；其余场景报错。
- 目标：先只做静态限制，不做实际 inlining 优化。
- 验收：typecheck fixture：非 inline lambda 中 `return` 报错；inline 场景允许（具体语法按设计）。
- 依赖：T0245、T0226、T0222

### T0445 [TODO] `as` 失败语义：要求 `Raise<RuntimeError>`（spec §4.4）
- 描述：当使用不安全 cast `x as T` 时，编译器应把其失败语义建模为 `Raise.raise(RuntimeError.ClassCastFailed)`，因此要求 `Raise<RuntimeError>` 除非被 handle/try 捕获。
- 目标：先只做静态 required-effects 检查；运行期失败触发 raise 的 codegen 后置（与 T0614/T0818 联动）。
- 验收：effects fixture：在 `/ Pure` 函数中使用 `as` 报 required effect；在 try/catch 内通过。
- 依赖：T0412、T0419、T0604、T0607

### T0446 [TODO] typealias：别名展开与循环检测（最小实现，支撑 sysroot 标准别名）（Appendix B.10）
- 描述：在 typecheck 的 `TypeRef → Type` lowering 阶段支持 `typealias`：把别名引用展开为其底层类型，并检测循环别名（直接/间接）。
- 目标：先只支持同包/同编译单元内的别名；跨包可见性与导出规则后续与 Cone 联动；错误信息需指出循环链路中的至少两个声明点。
- 验收：typecheck fixture：`typealias Byte = UInt8; val b: Byte = 1` 通过（或至少到签名检查通过）；构造 `typealias A=B; typealias B=A` 报循环别名错误（新错误码）。
- 依赖：T0314、T0403、T0404

### T0447 [TODO] 整数/布尔：一元与二元运算的类型规则（含位运算与移位）（spec §2.3.4）
- 描述：为内建整数类型实现运算符的静态规则：
  - 算术：`+ - * / %`
  - 比较：`== != < <= > >=`
  - 位运算：`& | ^ ~`
  - 移位：`<< >>`（仅整数；shift count 类型规则需固定）
- 目标：先只支持“同类型输入→同类型输出”的规则（不做数值提升/混合宽度运算）；不引入溢出检查（运行期语义留给 codegen 按 spec wrap-around/shift mask 落地）。
- 验收：typecheck fixture：`val x: UInt8 = 1; val y = x << 3` 通过；`val z = true << 1` 报错并定位到操作符。
- 依赖：T0252、T0211、T0405、T0407、T0418

### T0448 [TODO] class 初始化模型：property initializer / `init` / secondary constructor 规则（Appendix B.2.2）
- 描述：实现 class 初始化相关的静态规则：属性初始化表达式、多个 `init` block、secondary constructor body 的类型检查与顺序约束。
- 目标：先固定 Kotlin-like 初始化顺序与最小限制；复杂继承链与 effect 细节后续补齐。
- 验收：typecheck fixture：`init` 中引用未就绪成员时报错；secondary constructor 非法 delegation 报错；合法初始化顺序通过。
- 依赖：T0256、T0257、T0316、T0406

### T0449 [TODO] enum 完整布局语义：niche 优化 / oversized variant boxing / disparity lint（spec §2.3.2）
- 描述：在类型系统层固定 rich enum 的布局选择规则：何时使用 niche optimization、何时对 oversized variant 自动 boxing、何时发出 size disparity lint。
- 目标：先把规则、诊断与 type metadata 固定下来；具体低层布局由 codegen 落地。
- 验收：typecheck fixture：`Option<RefType>` 命中 niche 优化路径；oversized variant 触发 boxing/lint（warning 可先记录不强制 golden）。
- 依赖：T0425、T0418

### T0450 [TODO] pattern rest `..`：类型检查与绑定规则（spec §4.2）
- 描述：实现 `..` 的静态规则：出现位置、只能出现一次、与 tuple/struct/variant pattern 的匹配关系，以及 rest 不引入绑定。
- 目标：先只支持 tuple/struct；variant positional rest 若语法允许则一并纳入，否则后续扩展。
- 验收：typecheck fixture：`val (x, ..) = t` 通过；多个 `..`、非法位置、对非解构类型使用 `..` 报错。
- 依赖：T0255、T0427、T0430

### T0451 [TODO] 委托属性：标准接口与标准 delegates 表面（spec §10.4）
- 描述：补齐 delegated property 的静态表面：`ReadOnlyProperty` / `ReadWriteProperty` 接口规则，以及 `scoop.delegates` 中 `lazy` / `observable` / `vetoable` / map-backed delegate 的最小声明面。
- 目标：先只固定签名与类型规则；具体库行为与线程安全语义后续任务补齐。
- 验收：resolve/typecheck fixture：`val x by lazy { ... }`、`var x by observable(...)`、map-backed delegate 均能过签名检查；缺少 `getValue/setValue` 报错。
- 依赖：T0434、T1208

### T0452 [TODO] `object` / `companion object`：类型规则与单例语义（Appendix B.9）
- 描述：实现 `object` / `companion object` 的类型检查规则：单例不可构造、成员访问类型正确、companion 可通过宿主类名访问。
- 目标：先只固定静态语义；初始化时机与 storage 留给 codegen/runtime。
- 验收：typecheck fixture：`object Foo` 不能像 class 一样调用构造；`ClassName.member` 在 companion 存在时通过；无 companion 时报错。
- 依赖：T0258、T0317、T0404

---

## T05：类型推断（阶段 4：约束生成与求解，逐步扩展）

### T0501 [TODO] 推断框架：引入 constraint 表示与求解器骨架（spec §14.9）
- 描述：为 `infer` 建立数据结构：`τ1 <: τ2`、相等、未知类型变量。
- 目标：先只实现相等约束与简单 unify；subtyping 后续任务。
- 验收：新增单测：`T = Int`、`T = String` 冲突时报错；`cargo test -p scoopc` 通过。
- 依赖：T0401

### T0502 [TODO] 局部变量推断：`val x = expr` 推断 x 类型（spec §14.3）
- 描述：当缺少类型注解时，从 initializer 推断类型。
- 目标：先只对字面量/简单表达式生效；复杂情况可要求注解并报“推断失败”。
- 验收：infer fixture：`val x = 1` 推断为 Int；`val x = if (...) 1 else 2` 也可。
- 依赖：T0405、T0501

### T0503 [TODO] `if`/`when` 分支类型的 LUB 推断（spec §14.6）
- 描述：在推断阶段计算分支合并类型（先简化规则）。
- 目标：先支持相同类型与 Any fallback；后续再做真正 LUB/union。
- 验收：infer fixture：`val x = if (c) 1 else 2` → Int；`if (c) 1 else "s"` → Any。
- 依赖：T0414、T0502

### T0504 [TODO] lambda 参数类型下推（spec §14.7.2）
- 描述：在调用点已知函数类型时，把参数类型下推到 lambda 参数。
- 目标：先只支持单参数 lambda；不支持多段推断链。
- 验收：infer fixture：`fun takes(f: (Int) -> Int) {}` + `takes { x -> x }` 通过并推断 x 为 Int。
- 依赖：T0219、T0222、T0501

### T0505 [TODO] 泛型实参推断 v0（spec §14.5）
- 描述：从调用参数推断泛型实参（例如 `id(1)` 推断 `T=Int`）。
- 目标：先只做单个类型参数；不处理 variance 与 star projection。
- 验收：infer fixture：`fun id<T>(x: T): T {}` + `val a = id(1)` 推断返回 Int。
- 依赖：T0218、T0502

### T0506 [TODO] 子类型约束与 subtype unification（spec §14.8）
- 描述：实现 `τ1 <: τ2` 约束的求解（先覆盖 Any/Option/tuple/function 的一小部分）。
- 目标：先不追求完整 Kotlin 子类型；先服务推断与错误信息。
- 验收：infer fixture：把 `Int` 赋给 `Any` 通过；把 `Any` 赋给 `Int` 失败并给出清晰诊断。
- 依赖：T0501、T0418

### T0507 [TODO] 返回类型推断（spec §14.6）
- 描述：当函数缺少返回类型注解时，从 `return`/最后表达式推断返回类型。
- 目标：先只支持单一 return 路径；多路径合并后续。
- 验收：infer fixture：`fun f(){ 1 }` 推断为 Int（或要求显式 `: Int`，需决定并固定规则）。
- 依赖：T0226、T0502

### T0508 [TODO] effect 推断 v0：private/internal 可推断 row，public 默认 Pure（PLAN §6.1）
- 描述：实现 effect row 的推断入口：public 函数默认 `/ Pure`（强制），private/internal 可推断。
- 目标：先只覆盖单文件；跨包 API 后续与 Cone 联动。
- 验收：effects fixture：public 函数 perform Raise 时报错；private 函数可推断出需要 Raise（或在 dump-hir 中可见）。
- 依赖：T0604、T0245

### T0509 [TODO] effect row 参数 `eff` 推断（spec §14.7.3 / §3.4）
- 描述：实现 `eff E = Pure` 这种 row type parameter 的推断/实例化规则。
- 目标：先只支持默认值与 `E1+E2` 实参；不做高阶 row 运算。
- 验收：effects fixture：`fun <eff E = Pure> ... / E` 在调用点省略 row 参数可推断为 Pure。
- 依赖：T0250、T0603、T0508

### T0510 [TODO] 推断失败诊断：最小可读解释（spec §14.7.4）
- 描述：把“推断失败”映射到具体 span，并给出最小解释（期望类型/实际类型/约束来源）。
- 目标：先覆盖常见：参数不匹配、分支类型不一致、lambda 参数推断失败。
- 验收：新增 infer-fail fixtures：每个错误都断言错误码 + 关键提示子串。
- 依赖：T0005、T0501

### T0511 [TODO] use-site `eff` row 参数：默认值与显式实参推断（spec §3.4 / §14.7.3）
- 描述：当类型或调用点使用 `Type<eff Row>` 时，支持 effect row 默认值、显式实参和由上下文/lambda body 反推的 row 参数实例化。
- 目标：先覆盖“单个 row 参数 + 默认值 + 简单并集”的场景；高阶 row 约束后续。
- 验收：effects/infer fixture：`Disposable` 省略 use-site row 时默认到 `Pure`；显式 `Disposable<eff Async>` 可参与调用检查。
- 依赖：T0253、T0509、T0603

---

## T06：效果系统（阶段 5：先静态，再逐步落地运行时）

### T0601 [TODO] Parser：`effect` 声明体内的操作签名（spec §5.2）
- 描述：在 effect type body 内解析 `fun op(args): Ret` 列表，并区分 effect operation 与普通方法。
- 目标：先不解析实现体（operation 应无 body）；不支持默认实现。
- 验收：parse fixture：`effect Raise<E> { fun raise(error: E): Nothing }` 能解析出 operation 列表。
- 依赖：T0203、T0218

### T0602 [TODO] Typecheck：effect operation 的类型规则与命名空间
- 描述：把 effect 看作“可 perform 的接口”，operation 生成对应的 perform signature。
- 目标：先只支持 sysroot 的 `Raise`；不做 effect polymorphism。
- 验收：typecheck fixture：`Raise.raise(e)`（按语法）能通过（或暂以 `perform Raise.raise(e)` 为准，按 spec）。
- 依赖：T0601、T0404

### T0603 [TODO] Parser：函数/函数类型上的 effect row `/ RowExpr`（spec §5.8、§7.5）
- 描述：在声明与类型位置支持 `/ Pure` 与 `/ E1+E2`。
- 目标：RowExpr 先只支持 `Pure`、单个 effect 名、`+` 并集。
- 验收：parse fixture：`fun f(): Int / Pure { ... }`（或无 body）能解析。
- 依赖：T0219

### T0604 [TODO] Typecheck：required effects 检查（spec §14.7.1）
- 描述：当函数体 perform 了 effect，但未在函数签名 row 中声明，也未被 handle，则报错。
- 目标：先只覆盖 `Raise`；先不实现 handler，允许“显式声明 row”。
- 验收：effects fixture：`fun f() { Raise.raise(e) }` 在 `/ Pure` 下失败；在 `/ Raise<...>`（按语法）下通过。
- 依赖：T0602、T0603

### T0605 [TODO] Parser：`handle { ... } with { ... }`（spec §5.4）
- 描述：解析 handle 表达式与 handler arms（先支持 non-resuming `->` 一种 arm）。
- 目标：先不实现 `-> resume` 与 `, k ->`；arm body 可用 block 表达式。
- 验收：parse fixture：最小 handle 示例可解析；语法错误能恢复到下一个 arm。
- 依赖：T0214、T0207

### T0606 [TODO] Typecheck：handler arms 的类型规则（non-resuming）
- 描述：校验 arm 的参数类型、返回类型、以及 handle 表达式整体类型。
- 目标：先只实现 “处理 Raise” 的 try/catch 等价形式；不实现 continuation 类型。
- 验收：effects fixture：`handle { ... } with { Raise.raise(e) -> ... }` 类型正确；错误 arm 返回类型不匹配时报错。
- 依赖：T0605、T0604

### T0607 [TODO] 语法糖：`try/catch/finally` → `handle`（spec §5.7）
- 描述：在 parser 层支持 `try { } catch (e: T) { } finally { }` 并 lowering 到 handle AST。
- 目标：先只支持单个 catch；finally 可选；不支持多 catch。
- 验收：parse fixture：try/catch/finally 可解析并 lowering；typecheck fixture：对应的 Raise 处理不触发 required effects。
- 依赖：T0605、T0606

### T0608 [TODO] RowExpr 静态语义：`Pure`/`+`/默认 effect/containment（spec §5.8）
- 描述：实现 effect row 的语义：并集、空行 `Pure`、默认 effect 规则、以及 `R1 ⊆ R2`（subeffecting）的最小判定。
- 目标：先把 row 当作“集合”；不实现高级归一化；泛型 row 变量后续任务补。
- 验收：effects fixture：`/ Pure` 可赋给 `/ Pure`；`/ Raise` 不能赋给 `/ Pure`；`/ Pure` 可视为 `/` 空行。
- 依赖：T0603

### T0609 [TODO] effect polymorphism：`eff` row 参数与 overriding 规则（spec §5.9）
- 描述：支持 `<eff E = Pure>` 这类 row 参数，并实现 overriding：`R_over ⊆ R_base`。
- 目标：先只对 member override 做静态检查；不做动态 dispatch。
- 验收：effects fixture：override 方法的 row 超集时报错；row 子集允许。
- 依赖：T0509、T0608

### T0610 [TODO] Program boundary：entry point 必须 `Pure`（spec §5.10）
- 描述：定义 entry point（例如 `fun main()`）并强制其 effect row 为 `Pure`（或可隐式推断但必须最终 Pure）。
- 目标：先只检查 `main`；多 entry point（库）后续。
- 验收：effects fixture：`main` 里 perform Raise（未处理）时报错；`main` 使用 try/catch 处理后通过。
- 依赖：T0604、T0607

### T0611 [TODO] Continuation 类型建模（spec §5.5）
- 描述：在类型系统中加入 `Continuation<T, /E>`（或等价表示），并把 `resume(value)` 的类型规则固定下来。
- 目标：先只建类型与 typecheck 规则；不做 codegen。
- 验收：新增 typecheck fixture：`k: Continuation<Int, /Pure>` 的参数/返回类型检查正确；`resume` 多次调用的静态限制可先不做。
- 依赖：T0609、T0435

### T0612 [TODO] HIR/MIR：在 IR 中表达 `perform` 与 `handle`（不做 lowering）
- 描述：为 effect 调用与 handle 表达式添加 IR 节点，确保能从 AST lowering 到 HIR/MIR 并 dump。
- 目标：先只覆盖 non-resuming arm；不实现 unwinding/state machine。
- 验收：`scoop dump-hir`/`dump-ir` 能输出含 perform/handle 的 IR；新增 fixtures/hir 或 snapshot golden 覆盖。
- 依赖：T0605、T0702

### T0613 [TODO] lowering step 1（部分）：定义 runtime ABI（perform slot + flag）并在 codegen 侧可调用
- 描述：固定 runtime C ABI（函数/全局符号名），codegen 能生成对其的读写调用。
- 目标：先只支持单个 slot 类型（例如指针/整型）；复杂 payload 后续。
- 验收：`--emit-llvm` 产物里包含对 runtime 符号的引用；链接阶段不报未定义符号。
- 依赖：T0906、T0804

### T0614 [TODO] lowering step 1（部分）：`Raise.raise` 的 flag-based unwinding（只支持最小示例）
- 描述：实现 `Raise.raise(e)`：写 perform slot + set flag + 早退；调用边界检查 flag 并向外传播；try/catch 在边界消费 slot。
- 目标：先只支持 `Raise` + `try/catch`（无 finally、无用户自定义 effect）；先不支持跨函数捕获复杂状态。
- 验收：新增 run-pass fixture：`try { Raise.raise(...) } catch { ... }` 能运行并输出预期；新增 compile-fail：未处理 Raise 报 required effects。
- 依赖：T0613、T0106b2、T0807

### T0615 [TODO] lowering step 1（补齐）：`finally` 的清理语义（spec §5.7）
- 描述：确保 `finally` 在正常路径与 raise/unwind 路径都执行一次。
- 目标：先只支持 try/catch/finally；不支持 nested handler stack。
- 验收：新增 run-pass fixture：finally 中打印日志；无论 raise 与否都出现一次且顺序正确。
- 依赖：T0614、T0707

### T0616 [TODO] lowering step 2：`-> resume`（栈 state machine）（PLAN §6.3.2）
- 描述：把 handle body 分段、提升跨段 locals，并用 while-loop state machine 实现立即恢复。
- 目标：先只支持单个 perform 点；`resume` 必须恰好一次的检查可先只做运行期断言。
- 验收：新增 run-pass fixture：自定义 effect + `-> resume` 能恢复并继续执行；多次 resume 报错（运行期）。
- 依赖：T0615、T0703

### T0617 [TODO] lowering step 3：`, k ->`（堆 continuation + one-shot）（PLAN §6.3.3）
- 描述：实现 continuation 对象捕获 handler stack，支持跨线程 `resume`，并用原子状态位保证 one-shot。
- 目标：先只支持单线程 resume；跨线程作为后续子任务。
- 验收：新增 run-pass fixture：保存 continuation 后稍后 resume；重复 resume 失败（错误/诊断明确）。
- 依赖：T0616、T0914

### T0618 [TODO] 跨线程 `resume`：恢复 captured handler stack 到当前线程 TLS（spec §5.5）
- 描述：实现跨线程 resume 的语义与 runtime 支持（TLS handler stack 切换）。
- 目标：先只支持 2 线程；不实现调度器。
- 验收：新增 run-pass fixture：在新线程 resume continuation，程序输出符合预期且不崩溃。
- 依赖：T0617、T0915

### T0619 [TODO] async/await（作为 `Async` effect 的语法糖）（spec §5.7）
- 描述：解析并 typecheck `async/await`，lowering 到 effect perform/handle（或库函数）模型。
- 目标：先只实现单线程、无取消；spawn/结构化并发后续。
- 验收：新增 run-pass fixture：最小 async/await demo 输出正确；required effects 规则一致。
- 依赖：T0616、T0807

### T0620 [TODO] `spawn`：结构化并发最小模型（spec §5.7）
- 描述：实现 `spawn` 语法糖与 runtime 支持（join/取消语义先简化）。
- 目标：先只支持 join；取消后置。
- 验收：新增 run-pass fixture：spawn 两个任务并 join；输出顺序/值正确。
- 依赖：T0619

### T0621 [TODO] generator/yield：库级实现验证（spec §5.7）
- 描述：基于 continuation 或 effect，提供最小 `yield`/迭代器 demo（无需专用语法）。
- 目标：先只作为库/fixtures 验证，不强依赖语法。
- 验收：新增 run-pass fixture：生成器 yield 多次并消费；输出正确。
- 依赖：T0617

### T0622 [TODO] `Task<T>`：类型/库模型与 lazy 语义（spec §5.3 / §5.7）
- 描述：在 sysroot/type system 中引入 `Task<T>` 的最小模型，固定“懒执行直到 `await` 或显式启动”的语义，并为 `spawn/async` 共享同一任务抽象。
- 目标：先只固定类型面与基础语义；取消/结构化并发细节后续。
- 验收：effects/typecheck fixture：`val t: Task<Int> = ...` 合法；`await` 仅接受 `Task<T>`；未启动任务不要求立即执行。
- 依赖：T0611、T0820

### T0623 [TODO] `async fun`：desugar 到 `fun ...: Task<T>`（spec §5.3 / §5.7）
- 描述：实现 `async fun foo(): T` 的签名与 lowering 规则：对外暴露 `Task<T>`，而不是 `T / Async`；`/ Async` 只存在于 Task 的计算上下文。
- 目标：先只覆盖函数声明与调用点；与 executor 的交互后续由 runtime 任务补齐。
- 验收：effects fixture：`async fun fetch(): Int` 的调用点类型为 `Task<Int>`；把它当作 `Int / Async` 使用时报错。
- 依赖：T0619、T0622

### T0624 [TODO] effect rows：use-site `Type<eff Row>` 的实例化与检查（spec §3.4 / §5.8）
- 描述：在类型检查阶段支持 `Type<eff Row>` 的显式实参与默认化，并与 overriding、required effects、subeffecting 联动。
- 目标：先只支持单个 row 参数；语法合法性由 parser 先行保证。
- 验收：effects fixture：`Disposable<eff Async>` 调用需要 Async；`Disposable` 省略时默认 `Pure`；非法多 `eff` clause 报错。
- 依赖：T0253、T0609、T0511

### T0625 [TODO] Appendix A 一致性：嵌套 handler 的语义契约与 lowering 校验
- 描述：在 lowering/semantics 层明确并验证：嵌套 `handle` 必须遵循“最近匹配 handler”分发，且 handler arm body 在其自身 dispatch scope 外执行。
- 目标：先只覆盖 `Raise` 与最小自定义 effect；实际 runtime 支持由 T0916 补齐。
- 验收：effects + run-pass fixture：嵌套 handler 的最近匹配规则成立；arm 内 re-perform 不会自捕获。
- 依赖：T0615、T0916

---

## T07：IR 与单态化（阶段 6：为 LLVM 做准备）

### T0701 [TODO] HIR：定义已解析/已类型化的中间表示骨架
- 描述：新增 `scoopc::hir` 模块：表达式/语句/声明节点携带类型与解析后的 symbol 引用。
- 目标：先覆盖：fun、val、block、call、literal；其余节点可用 `Todo`/`Unimplemented` 占位。
- 验收：新增 `scoop dump-hir <file>`（可先打印 Debug）；对一个最小文件能输出 HIR。
- 依赖：T0404、T0305、T0207、T0002

### T0702 [TODO] AST → HIR lowering（声明头 + 简单函数体）
- 描述：实现从 AST 构造 HIR：把 `TypeRef` lower 为 `TypeId`，把 ident 绑定为 `SymbolId`。
- 目标：先只支持无控制流的函数体；不支持闭包捕获。
- 验收：新增 fixtures/hir 目录（或用 dump-hir 命令行 golden）；最小程序 lowering 不报错。
- 依赖：T0701

### T0703 [TODO] MIR：基本块 + 显式控制流骨架（为后续 finally/effect 做准备）
- 描述：定义 MIR 的 BB/terminator/locals；支持顺序执行与 return。
- 目标：暂不实现 if/when lowering；先把数据结构立起来。
- 验收：新增单测：手工构造 MIR 并验证 CFG 连通；或对最小 HIR lowering 生成 1 个 BB + return。
- 依赖：T0702

### T0704 [TODO] 单态化缓存键：`Symbol + type args + effect row args`（spec §3.1、PLAN §7.2）
- 描述：定义 MonomorphKey，并实现 Hash/Eq 与 debug 输出。
- 目标：先只对函数生效；不实现真实复制生成。
- 验收：新增单测：同一 key 去重；不同 type args key 不同。
- 依赖：T0701、T0401

### T0705 [TODO] HIR：补齐控制流与语句节点（if/when/while/assign/return）
- 描述：把 parser/typecheck 已支持的控制流语法在 HIR 中建模出来（含 type 与 span）。
- 目标：先只覆盖：if/when/while/return/assign；for 后续。
- 验收：`scoop dump-hir` 对包含上述语法的文件能输出；无 `todo!()`/panic。
- 依赖：T0701、T0214、T0215、T0228、T0227、T0226

### T0706 [TODO] AST → HIR lowering：把 Stmt/Expr 降到 HIR（含符号绑定结果）
- 描述：实现 block 内语句/表达式 lowering（含局部绑定、赋值、return、调用、成员访问）。
- 目标：先不做 pattern lowering（when 先用简化分支表达）；pattern 后续任务补。
- 验收：新增 HIR snapshot fixtures：至少 1 个包含局部变量与调用；输出稳定。
- 依赖：T0705、T0305、T0443

### T0707 [TODO] MIR：cleanup/finally 的基本模型（为 try/finally 与 effect unwinding）
- 描述：在 MIR 中引入 cleanup block 或显式 drop/cleanup 机制，让 lowering 可以表达“无论如何都执行”的语义。
- 目标：先只支持 `try/finally`；不实现析构（语言尚无 drop）。
- 验收：新增单测：构造一个带 cleanup 的 MIR 并验证 CFG；后续可被 codegen 使用。
- 依赖：T0703

### T0708 [TODO] MIR lowering：if/when → 基本块 + terminator
- 描述：把条件分支 lowering 为 CFG（br/switch），并在 merge 点管理临时变量。
- 目标：先只支持 expression-when（每分支一个表达式）；带 guard 的 pattern 后置。
- 验收：`--dump-ir`（或 dump-mir）对 if/when 示例输出多个 BB；并能被 codegen 接受。
- 依赖：T0706

### T0709 [TODO] MIR lowering：while/break/continue
- 描述：把 while lowering 为 loop CFG，并正确处理 break/continue 跳转目标。
- 目标：先不支持 label；后续再补。
- 验收：新增 IR snapshot fixture：while 内 break/continue 的 CFG 正确（可用文本快照验证）。
- 依赖：T0706

### T0710 [TODO] 闭包与函数值：lambda → `{ env_struct, fn_ptr }`（PLAN §7.3）
- 描述：在 HIR/MIR 中引入 closure 表示，支持捕获变量布局与调用约定。
- 目标：先只支持不捕获 lambda（env 为空）；捕获后续子任务。
- 验收：新增 typecheck + IR fixture：把 lambda 赋给函数类型并调用；编译通过并能 codegen（后续与 T0810 联动）。
- 依赖：T0222、T0435、T0706

### T0711 [TODO] 捕获闭包：计算 capture set 并生成 env struct（PLAN §7.3）
- 描述：分析 lambda 体对外部局部变量的引用，生成 env struct，并在调用点传递 env 指针。
- 目标：先只支持捕获 `val`；捕获 `var`（可变捕获）后置或以 box 处理。
- 验收：新增 IR fixture：lambda 捕获外部 val 并使用；codegen/run-pass（后续）输出正确。
- 依赖：T0710、T0304

### T0712 [TODO] 单态化（monomorphization）：生成具体实例 MIR（PLAN §7.2）
- 描述：对每个 `MonomorphKey` 生成专用实例（函数/类型），并缓存避免重复。
- 目标：先只对函数泛型实例化；类型泛型与 effect row 参数后置。
- 验收：新增 `tests/fixtures/codegen/monomorph_id_int.scoop`：`id(1)` 与 `id("s")` 生成两个实例（可用 dump-ir 验证）。
- 依赖：T0704、T0505、T0703

### T0713 [TODO] effect lowering：把 perform/handle 降到 MIR（non-resuming 先占位）
- 描述：在 MIR 中表达 perform 与 handler boundary（先用占位 terminator），为 T0614 的 codegen 做准备。
- 目标：先只覆盖 Raise 与 try/catch；resume/continuation 后置。
- 验收：dump-mir 能看到 perform/handler 相关 terminator；无 panic。
- 依赖：T0612、T0703

---

## T08：LLVM 后端与链接（阶段 7：inkwell codegen）

### T0801 [TODO] 引入 inkwell（或等价 LLVM 绑定）并完成最小编译
- 描述：在 `scoopc` 新增 feature-gated `inkwell` 依赖，确保 workspace 在 CI 环境可构建。
- 目标：先只做到 `cargo build`；不生成任何 IR。
- 验收：`cargo build --all` 通过；若 CI 缺 LLVM，则需要在文档/CI 明确安装步骤或先使用 feature gate 默认关闭。
- 依赖：T0001

### T0802 [TODO] 代码生成 v0：生成空 `main` LLVM module（可打印 IR）
- 描述：为一个最小 Scoop 程序生成 LLVM IR（哪怕只返回 0）。
- 目标：先不处理用户函数；先把 pipeline 与 target triple 跑通。
- 验收：新增 `scoopc` API 或 CLI `--emit-llvm` 能输出 `.ll`；对最小 fixture 可生成文件。
- 依赖：T0801、T0703

### T0803 [TODO] 目标机器与数据布局（target machine）初始化（PLAN §8.1）
- 描述：在 codegen 中按宿主平台初始化 target，设置 module data layout，并把 pointer size 等 target 信息暴露给后续类型映射（例如 `Int/UInt/UIntPtr` 的 word size）。
- 目标：先只支持 host；交叉编译后续。
- 验收：生成的 LLVM module 带 data layout；并可用 `llvm-as`（若有）验证（可选）。
- 依赖：T0802

### T0804 [TODO] 生成 object 文件（`.o`）并落盘
- 描述：从 LLVM module 生成目标文件（object），为后续链接做准备。
- 目标：先只生成单个 `.o`；不做 LTO。
- 验收：新增 `scoop build --emit-obj`（或 `scoopc` 命令）；产出 `.o` 文件存在且非空。
- 依赖：T0803

### T0805 [TODO] driver：实现 `scoop build <main.scoop> -o <bin>` 的“前端 + 产物路径”流程
- 描述：让 `scoop build` 至少能：读取文件 → parse/resolve/typecheck →（暂时）不 codegen 也能成功退出并准备输出路径。
- 目标：先把 CLI 与诊断体验打磨出来；codegen 后续任务接入。
- 验收：新增 fixtures（或集成测试）：`scoop build tests/fixtures/spec_doctest/overview_minimal_main.scoop -o /tmp/a` 返回 0。
- 依赖：T0404、T0002

### T0806 [TODO] 链接：把 `.o` 与 `scoop_runtime` 静态库链接为可执行文件
- 描述：实现最小链接器调用（可用 clang 或 `cc` crate）把 runtime 拉进来。
- 目标：先只支持 host 平台；暂不处理多文件/包。
- 验收：`scoop build ...` 产出可执行文件；运行后退出码正确（哪怕 main 空）。
- 依赖：T0804、T0014、T0805

### T0807 [TODO] driver：实现 `scoop run <main.scoop>`（build + exec）
- 描述：在 build 成功后执行产物，并把 stdout/stderr 透传。
- 目标：先不做 sandbox；超时/退出码断言留给 fixtures。
- 验收：`scoop run tests/fixtures/spec_doctest/overview_minimal_main.scoop` 返回 0。
- 依赖：T0806

### T0106b2 [TODO] run-pass fixtures：默认使用 `scoop run` 执行 + 增加 1 个可执行 fixture
- 描述：当 `scoop run`（T0807）可用后，fixtures runner 通过 `scoop run <fixture>` 真正执行 fixture，并断言 stdout。
- 目标：先只做 stdout；stderr/超时/退出码仍留给后续任务。
- 验收：新增 1 个 run-pass fixture（例如打印固定字符串）；`cargo run -p scoop -- test` 能编译并运行且通过。
- 备注：该任务本身属于测试体系，但其验收依赖完整执行链路（`scoop run` + link/runtime/codegen），因此放在这里以便按依赖顺序推进。
- 依赖：T0106b1、T0807

### T0108 [TODO] fixtures：支持环境变量开关（如 `SCOOP_GC_STRESS=1`）（PLAN §10.4）
- 描述：允许 fixture 通过 `// ENV: KEY=VALUE`（或统一用 `ARGS`）配置测试运行环境。
- 目标：先只支持设置环境变量；不做进程级 sandbox。
- 验收：新增 1 个 run-pass fixture：在运行时读取 env 并打印/分支；runner 能正确设置 env。
- 备注：该能力需要 run-pass fixtures 的“真实执行”（T0106b2）才能通过 fixture 验收。
- 依赖：T0106b2、T0102

### T0808 [TODO] codegen v1：整数/布尔字面量 + 运算（含位运算/移位）+ return（spec §2.3.4）
- 描述：为最小表达式子集生成 LLVM IR：Int/Bool 字面量、算术/比较、位运算 `& | ^ ~`、移位 `<< >>`、return。
- 目标：按 spec 固定整数语义：
  - wrap-around（LLVM 默认不加 `nsw/nuw` 即可）
  - signed `>>` 用算术右移，unsigned `>>` 用逻辑右移
  - shift count 必须 mask（例如 `shift % bitWidth`），避免 LLVM 对超范围 shift 的 UB
- 验收：新增 run-pass fixture：位运算与移位得到稳定结果（含 `UInt8` 的 `>>`）；输出正确。
- 依赖：T0802、T0708、T0106b2

### T0809 [TODO] codegen v2：局部变量（alloca）与赋值
- 描述：把 HIR/MIR locals 映射到 LLVM alloca/load/store，支持 `var` 赋值更新。
- 目标：先只支持函数内局部；不实现逃逸分析。
- 验收：新增 run-pass fixture：`var x = 1; x = x + 1; print(x)` 输出 2。
- 依赖：T0808、T0443

### T0810 [TODO] codegen v3：函数调用 ABI（参数传递/返回值）
- 描述：支持调用用户定义函数与 sysroot/extern 函数（先按简单 C ABI）。
- 目标：先不支持可变参数/泛型实例化跨模块；只编译单模块。
- 验收：新增 run-pass fixture：定义 `add` 并调用；输出正确。
- 依赖：T0809、T0311

### T0811 [TODO] codegen：值类型布局（struct）与字段访问（PLAN §8.2）
- 描述：为 struct 生成 LLVM struct type，支持按字段索引 GEP 访问。
- 目标：先只支持无 padding 调整（交给 LLVM layout）；不实现 repr 属性。
- 验收：新增 run-pass fixture：构造 struct、读取字段、打印；输出正确。
- 依赖：T0810、T0409

### T0812 [TODO] codegen：tuple/Unit 的表示与传递（spec §2.3.3）
- 描述：为 tuple 生成 LLVM struct（或 aggregate），支持构造/解构访问（先用于表达式/赋值）。
- 目标：先只支持定长 tuple；不实现 variadic tuple。
- 验收：新增 run-pass fixture：`val t = (1,2); print(t.0+t.1)`（按语法）输出 3。
- 依赖：T0811、T0410

### T0813 [TODO] codegen：rich enum（tagged union）最小布局（PLAN §8.2）
- 描述：为 enum 生成 `{tag, payload}` 表示（payload 用最大变体的 LLVM struct/union），支持构造与判别。
- 目标：先不做 niche 优化；先只支持小 payload。
- 验收：新增 run-pass fixture：构造 `Some(1)` 与 `None`，when 分支输出不同结果。
- 依赖：T0812、T0425、T0708

### T0814 [TODO] codegen：`when` lowering（switch + pattern tests）
- 描述：把 `when`（至少 enum/Bool）降到 LLVM switch；tuple/struct pattern 用字段比较实现。
- 目标：先不支持 or-pattern/guard；后续再扩展。
- 验收：新增 run-pass fixture：when on Option/Bool 输出正确；缺 else 的错误在 typecheck 阶段已挡住。
- 依赖：T0813、T0428

### T0815 [TODO] runtime 集成：生成的 `main` 调用 `scoop_runtime_init`
- 描述：在入口函数里调用 runtime init（以及必要的 thread register）。
- 目标：先只在 main 调用一次；多线程后续再处理。
- 验收：链接后的程序运行不崩溃；可通过运行时 debug 输出确认 init 被调用（若启用）。
- 依赖：T0901、T0806

### T0816 [TODO] GC 接口：shadow stack 插桩（函数 prologue/epilogue）（PLAN §8.3）
- 描述：为包含 GC 引用的函数生成 `GcFrame` push/pop，并在需要处写 roots。
- 目标：先只支持单线程；先只插桩“明显活跃的引用局部变量”。
- 验收：新增 run-pass fixture：分配若干对象（先可用 malloc 代替 GC）并触发一次“伪 GC 扫描”（仅遍历 roots）不崩溃。
- 依赖：T0905、T0817

### T0817 [TODO] heap 分配：为 boxing/引用对象生成 `scoop_alloc` 调用（PLAN §9.1）
- 描述：在 codegen 中为 box/object 分配调用 runtime `scoop_alloc`，并写入最小对象头/类型描述指针（若已定义）。
- 目标：先只支持 boxing `Int`/简单对象；不实现移动 GC。
- 验收：新增 run-pass fixture：`val a: Any = 1` 运行不崩溃；并可通过调试打印确认对象非空。
- 依赖：T0902、T0441、T0810

### T0818 [TODO] effect codegen：flag-based Raise/try-catch（对接 T0614）
- 描述：把 MIR 中的 perform/handle terminator 生成 LLVM IR，与 runtime slot/flag 交互，实现最小 Raise 处理。
- 目标：先只支持 `Raise<RuntimeError>` 与 try/catch；finally 在 T0615 补齐。
- 验收：run-pass fixtures：Raise 被 catch 捕获；未捕获时报错或退出（按设计）。
- 依赖：T0713、T0614

### T0819 [TODO] driver：`--emit-llvm/--emit-obj/--emit-asm` 选项与 fixtures 支持（PLAN §9.3）
- 描述：在 `scoop build` 增加 emit 选项，并允许 fixtures 通过 `ARGS` 触发生成产物用于排查。
- 目标：先只支持单文件输出；不做多产物目录管理。
- 验收：新增 1 个 fixture：`// ARGS: --emit-llvm` 能生成 `.ll` 文件；`scoop test` 通过。
- 依赖：T0102、T0804

### T0820 [TODO] sysroot：最小 I/O API（`print/println`）与字符串基础（spec §8）
- 描述：在 sysroot 声明最小 `print/println`（可标为 `@Extern` 或 `@Intrinsic`），并把 `String` 作为 reference type 的最小表面固定下来。
- 目标：先只声明 API；实现可在 runtime（C）中提供。
- 验收：resolve/typecheck fixture：`println("hi")` 可通过（至少到 typecheck）；未声明时报错。
- 依赖：T0418、T1001

### T0821 [TODO] runtime：最小字符串对象与 `scoop_println` 实现（C）
- 描述：实现 runtime 字符串承载（可先用 C 字符串包装）与打印函数，供 early run-pass 使用。
- 目标：先只支持 UTF-8 字面量与拼接后置；不实现完整 String API。
- 验收：链接后程序调用 `println("hi")` 能输出；run-pass fixture 通过。
- 依赖：T0820、T0106b2、T0902

### T0822 [TODO] codegen：字符串字面量与调用 `println`（spec §8.1）
- 描述：把 `"..."` 与 raw string lowering 为 runtime 字符串对象（或常量指针），并生成对 `scoop_println` 的调用。
- 目标：先只支持纯字面量；插值字符串后续任务补。
- 验收：新增 run-pass fixture：`fun main(){ println(\"hello\") }` 输出 `hello`。
- 依赖：T0821、T0810

### T0823 [TODO] f-string 插值：`f\"...{expr}...\"` 的 lowering（spec §8.2）
- 描述：实现插值字符串的 lowering：拆分为片段并拼接（或调用格式化 runtime），至少支持 `{Int}`/`{String}`。
- 目标：先不实现 `trimIndent`；先不做 locale/format spec。
- 验收：新增 run-pass fixture：`val s = f\"hi {name}\"; println(s)` 输出正确。
- 依赖：T0217、T0822、T0809

### T0824 [TODO] tuple 字段访问语法对齐 spec：`._0` / `._1`（spec §2.3.3）
- 描述：补齐 tuple 字段访问的 lowering/codegen，并把相关 fixtures 与文档样例统一到 spec 语法 `t._0` / `t._1`。
- 目标：不修改既有任务定义；通过新增任务把语法差异显式收口。
- 验收：新增 run-pass fixture：`val t = (1,2); print(t._0 + t._1)` 输出 `3`；`t.0` 不作为合法 tuple 访问被接受。
- 依赖：T0812、T0210、T0410

### T0825 [TODO] codegen：`when` lowering 补齐 or-pattern / guard（spec §4.2）
- 描述：在已有 `when` lowering 基础上补齐 or-pattern 与 guard 的代码生成：or-pattern 共享后继块，guard 在匹配成功后再判定条件。
- 目标：先不追求最优 CFG；先保证语义正确与诊断稳定。
- 验收：新增 run-pass fixture：`A | B` 分支与 `pattern if cond` 分支都能得到正确结果。
- 依赖：T0814、T0429、T0450

### T0826 [TODO] codegen：enum niche 优化 / oversized variant boxing / disparity lint（spec §2.3.2）
- 描述：为 rich enum 落实完整布局策略：能使用 niche 时消除显式 tag；oversized variant 自动 boxing；size disparity 明显时发出 lint warning。
- 目标：先覆盖 `Option<RefType>` 等高价值场景；更复杂嵌套 niche 后续可继续扩展。
- 验收：新增 codegen/run-pass fixture：`Option<RefType>` 正常工作；oversized variant case 通过并伴随 lint（warning 可先文本断言）。
- 依赖：T0449、T0813

### T0827 [TODO] `trimIndent()`：运行期 fallback 与字符串 API 对接（spec §8.4）
- 描述：当 `trimIndent()` 的接收者不是编译期常量时，生成普通运行期调用并接到最小字符串 API。
- 目标：先只支持最常见的 raw string 场景；不做额外格式化 API。
- 验收：新增 run-pass fixture：raw string 调用 `trimIndent()` 后输出去缩进结果；非 raw string 也可走同一路径。
- 依赖：T0822、T1216

### T0828 [TODO] codegen：`object` / `companion object` 单例存储与成员访问（Appendix B.9）
- 描述：实现 `object` / `companion object` 的最小 codegen：单例存储、一次初始化、静态成员访问 lowering。
- 目标：先只覆盖单线程；线程安全初始化后续可由 runtime 原语增强。
- 验收：新增 run-pass fixture：多次访问同一 object 获得同一实例语义；`ClassName.member` 可访问 companion 成员。
- 依赖：T0452、T0918

---

## T09：早期运行时（C + clang）（阶段 8：可执行与可观测）

### T0901 [TODO] runtime：补齐 `scoop_runtime_init` 的可观察行为（最小日志/断言）
- 描述：在 C runtime 初始化时设置最小全局状态，并可选输出 debug（受宏控制）。
- 目标：先不引入 GC；只让链接后的程序能调用 init 不崩溃。
- 验收：新增一个 Rust 集成测试（或小 C harness）调用 `scoop_runtime_init()` 通过；CI 通过。
- 依赖：T0014

### T0902 [TODO] runtime：实现 `scoop_alloc` 的最小可用版本（先用 `malloc`）
- 描述：把当前返回 0 的占位改为真正分配（暂时非 GC）。
- 目标：为后续 codegen 做最小保障；GC 语义后置。
- 验收：新增测试：调用 `scoop_alloc(16)` 返回非空；重复调用不崩溃。
- 依赖：T0901

### T0903 [TODO] runtime：引入线程注册接口（占位）与 TLS 骨架
- 描述：提供 `scoop_thread_register/unregister`（先空实现），为 GC/effect TLS 铺路。
- 目标：API 稳定、可跨平台；实现可后置。
- 验收：链接通过；新增测试在主线程调用 register/unregister 不崩溃。
- 依赖：T0901

### T0904 [TODO] GC v0：mark-sweep 的数据结构骨架（不要求可用）
- 描述：定义 heap、object header、free list 等结构体与接口。
- 目标：先立结构与接口，具体算法/性能后续。
- 验收：C 编译通过（clang warnings as errors 若开启）；新增最小单测/运行时自检（可选）。
- 依赖：T0902

### T0905 [TODO] Shadow stack：定义 `GcFrame` 结构与 TLS 链（PLAN §8.3）
- 描述：在 runtime 中定义 `GcFrame { prev, roots[] }` 与 `current_frame` TLS。
- 目标：先不做扫描；只把数据结构与 push/pop API 做出来。
- 验收：新增测试：push/pop 两层 frame 后 `current_frame` 指针正确回退。
- 依赖：T0903

### T0906 [TODO] effect runtime v0：TLS slot + flag（为 `->` handler 做准备）（PLAN §6.3.1）
- 描述：在 runtime 中增加 `__scoop_effect_active` 与 perform slot（结构体/union 先占位）。
- 目标：先不实现 dispatch；只提供 set/clear API。
- 验收：新增测试：set flag 后可读回；clear 后恢复初值。
- 依赖：T0903

### T0907 [TODO] runtime：类型描述（type descriptor）v0（trace bitmap/回调）
- 描述：定义 type descriptor 结构（大小、字段 trace 信息），供 GC 扫描对象内引用字段使用。
- 目标：先只支持 struct/box；interface/class 后续。
- 验收：新增 C/Rust 测试：构造一个 descriptor 并用它扫描一段内存（假设布局）不越界。
- 依赖：T0904

### T0908 [TODO] runtime：对象头（object header）与最小 heap 对象布局
- 描述：定义 heap 对象头：指向 type descriptor、flags/size 等，配合 `scoop_alloc` 返回指针。
- 目标：先只实现非移动对象；不实现压缩。
- 验收：新增测试：alloc 后 header 字段可读写；对齐满足基本要求。
- 依赖：T0902、T0907

### T0909 [TODO] GC v0：shadow stack root 扫描（单线程）
- 描述：实现扫描当前线程 `GcFrame` 链并枚举 roots，供 mark 阶段使用。
- 目标：先只支持单线程；不 stop-the-world。
- 验收：新增测试：构造 2 层 frame，每层 2 个 roots，扫描回收集到 4 个 roots。
- 依赖：T0905、T0907

### T0910 [TODO] GC v0：最小 mark-sweep（单线程）可用版本
- 描述：在 `scoop_alloc` 中分配对象并记录到 heap 列表；实现一次 mark-sweep（手动触发）。
- 目标：先不做触发策略；先提供 `scoop_gc_collect()` 手动调用。
- 验收：新增 run-pass fixture：分配大量对象并手动触发 collect，不崩溃且能回收未引用对象（可用计数验证）。
- 依赖：T0909、T0106b2

### T0911 [TODO] 线程注册 + stop-the-world 扫描所有线程（PLAN §9.1）
- 描述：实现线程注册表，GC 时暂停所有注册线程并扫描其 shadow stack。
- 目标：先只支持 2 线程；暂停策略可先用全局 mutex + 条件变量。
- 验收：新增运行期测试：两线程各自持有对象引用，GC 扫描到两边 roots；程序不崩溃。
- 依赖：T0903、T0910

### T0912 [TODO] pin/unpin API（spec §15.10 / PLAN §9.1）
- 描述：在 runtime 中提供 `scoop_pin/scoop_unpin`，并定义 pin 计数或列表，为未来移动 GC 做准备。
- 目标：在非移动 GC 中可先是计数/no-op，但语义与错误检查要固定。
- 验收：新增测试：pin/unpin 计数配对；重复 unpin 报错或断言。
- 依赖：T0910

### T0913 [TODO] effect runtime：handler stack push/pop + 最近匹配分发规则（Appendix A）
- 描述：实现 handler stack（TLS），并按“最近匹配 handler”分发；arm body 在 dispatch scope 外执行。
- 目标：先只支持单层 handler；多层嵌套后续。
- 验收：新增 run-pass fixture：嵌套 handle 时最近者优先；在 arm 内再次 perform 不会捕获到同一个 handler（按 Appendix A.4）。
- 依赖：T0906、T0106b2

### T0914 [TODO] continuation 对象：one-shot 状态位 + resume API（PLAN §6.3.3）
- 描述：定义 continuation 结构：捕获 handler stack + 目标状态；实现原子 one-shot。
- 目标：先只支持单线程；并发 resume 后续。
- 验收：新增运行期测试：同一 continuation resume 两次，第二次失败（返回错误码或 abort，需固定）。
- 依赖：T0913

### T0915 [TODO] 跨线程 continuation resume：TLS handler stack 切换（spec §5.5）
- 描述：实现把 continuation 捕获的 handler stack 安装到当前线程，并在 resume 后恢复原 TLS。
- 目标：先不支持并发同时 resume；只支持单次跨线程。
- 验收：新增 run-pass fixture：在新线程 resume continuation，结果与单线程一致。
- 依赖：T0914、T0911

### T0916 [TODO] effect runtime：多层 handler stack 嵌套 dispatch（修正 T0913 的单层目标，Appendix A）
- 描述：在已有 handler stack 原语之上补齐多层嵌套 handler：按“最近匹配 handler”分发，并保证 arm body 在自身 handler 的 dispatch scope 外执行。
- 目标：保持与 T0913 兼容，不修改既有任务；本任务专门补齐“多层嵌套”能力。
- 验收：新增 run-pass fixture：三层嵌套 handler 下最近匹配规则成立；arm 内 re-perform 命中外层 handler。
- 依赖：T0913

### T0917 [TODO] runtime：`Task<T>` / executor 最小原语（spec §5.7）
- 描述：提供支撑 `Task<T>` / `async` / `spawn` 的最小 runtime 原语：任务状态、入队/恢复、完成回调、可选显式 start。
- 目标：先只实现 cooperative、最小可观测版本；取消与复杂调度后续。
- 验收：新增运行期测试：创建 task、入队、完成后恢复 continuation；状态转换与回调顺序稳定。
- 依赖：T0906、T0914、T0622

### T0918 [TODO] runtime：`object` / `companion object` 的 once 初始化原语（Appendix B.9）
- 描述：若 codegen 采用 runtime 辅助初始化，则提供 once/guard 原语以支持 `object` / `companion object` 的一次初始化。
- 目标：先只支持单进程内初始化一次；跨 DLL / 动态链接细节后续。
- 验收：新增 run-pass fixture：多次并发前的重复访问不会重复初始化；初始化副作用只出现一次。
- 依赖：T0901

---

## T10：注解系统与系统编程通道（阶段 9）

### T1001 [TODO] Parser：注解使用 `@Name(...)` 的 AST 与解析（spec §15.3）
- 描述：允许在声明前出现一个或多个注解，并把它们记录到 AST 节点上。
- 目标：先只支持无参/仅字面量参数；不解析复杂表达式参数。
- 验收：新增 parse fixture：`@Unsafe fun f() {}` 可解析；`@Extern("c_name") fun g()` 可解析（若支持字符串参数）。
- 依赖：T0218

### T1002 [TODO] Parser：注解声明 `annotation class X(...)`（spec §15.2）
- 描述：支持声明注解类型，并在 type env 中识别其为“注解类”。
- 目标：先只支持 data-only（无方法体）；target/retention 规则后续。
- 验收：新增 parse+typecheck fixture：定义注解并使用；错误用法给出诊断。
- 依赖：T1001、T0404

### T1003 [TODO] Typecheck：内建注解 `@Unsafe/@NoGC/@Extern/@Intrinsic` 的合法性检查（PLAN §11）
- 描述：实现基础规则：`@Extern` 隐含 `@NoGC`；调用点需要 unsafe context（按 PLAN 建议）。
- 目标：先只做静态检查，不生成任何 codegen 行为。
- 验收：新增 `tests/fixtures/unsafe_nogc/*`：违规路径 compile-fail；合法路径 pass。
- 依赖：T0101、T1001、T0404

### T1004 [TODO] Typecheck：`@Unsafe { ... }` 块语法与上下文传播（spec §15.9.2）
- 描述：在表达式/语句层支持 unsafe block，并让检查器在该区域放宽限制。
- 目标：先只做上下文标记；不实现指针 API。
- 验收：unsafe_nogc fixture：在 unsafe block 内允许调用 `@Extern`，块外禁止。
- 依赖：T1003、T0207

### T1005 [TODO] `@NoGC`：调用限制与“可能分配”静态判定（spec §15.8）
- 描述：实现 `@NoGC`：禁止堆分配、禁止调用非 `@NoGC/@Extern`；当编译器无法证明无分配时必须保守报错。
- 目标：先只做基于“已知分配点”的保守分析；不做全程序逃逸分析。
- 验收：unsafe_nogc fixture：在 `@NoGC` 函数里调用 `scoop_alloc`（或构造 box）报错；调用纯函数通过。
- 依赖：T1003、T0817

### T1006 [TODO] `@Extern`：FFI 符号名与 ABI 约定（spec §15.8.3）
- 描述：为 `@Extern` 函数定义名称映射（如 `@Extern("puts")`）与最小 ABI（C ABI）。
- 目标：先只支持简单参数/返回类型（Int/ptr）；结构体传递后续。
- 验收：新增 run-pass fixture：调用 `@Extern("puts")` 打印字符串（或调用自带 runtime 打印 API）；输出正确。
- 依赖：T1001、T0810、T0106b2

### T1007 [TODO] `@Intrinsic`：sysroot 声明与编译器 lowering（spec §15.7）
- 描述：在 sysroot 中声明 intrinsic，并在 lowering/codegen 阶段把它们替换为内建操作（例如算术、类型反射）。
- 目标：先只实现 1~2 个 intrinsic（例如 `sizeOf<T>()`/`panic()`）。
- 验收：新增 comptime/typecheck fixture：调用 intrinsic 通过；codegen 侧不产生真正函数调用。
- 依赖：T0418、T1204

### T1008 [TODO] pin/unpin 语言层 API：从 sysroot 暴露到 runtime（spec §15.10）
- 描述：在 sysroot 增加 `pin/unpin` 声明，并在 codegen 中 lower 到 runtime 的 `scoop_pin/scoop_unpin`。
- 目标：先只支持对引用类型/box 对象；value types 不允许 pin。
- 验收：新增 run-pass fixture：pin 后在 GC collect 过程中对象不移动（非移动 GC 下可用“仍可访问”替代验证）；unpin 后仍可访问。
- 依赖：T0912、T0817

### T1009 [TODO] `@Unsafe`：最小 unsafe 原语（`Ptr<T>`/内存读写/地址转换）的语法与门禁
- 描述：引入最小 unsafe 原语（例如 `Ptr<T>`、`load/store`、`addrOf`、指针↔整数转换），并确保只能在 unsafe context 使用。
- 目标：先只提供极小集合以支撑 runtime/FFI；完整系统编程能力后续逐步补齐。
- 验收：unsafe_nogc fixture：在非 unsafe context 使用 ptr 操作报错；unsafe block 内通过。
- 依赖：T1004

### T1010 [TODO] sysroot：新增 `scoop.unsafe` 模块声明（`Ptr<T>` + 指针/整数转换 intrinsics）（spec §15.9.4）
- 描述：在 sysroot 增加专门的 unsafe 模块（建议 `package scoop.unsafe`），声明：
  - `@Intrinsic struct Ptr<T>`
  - `@Intrinsic @NoGC @Unsafe fun <T> ptrToUIntPtr(p: Ptr<T>): UIntPtr`
  - `@Intrinsic @NoGC @Unsafe fun <T> uintPtrToPtr(addr: UIntPtr): Ptr<T>`
- 目标：先只做“可见声明”；intrinsic 的具体 lowering 留给后续 codegen；模块命名与路径固定以便审计。
- 验收：新增 resolve fixture：`import scoop.unsafe.*` 后能引用 `Ptr<Int>`、`ptrToUIntPtr`；`scoop test` 通过。
- 依赖：T0418、T1001

### T1011 [TODO] Typecheck：`Ptr<T>` 的 GC-free pointee 限制（spec §15.9.4 / runtime §4.1）
- 描述：实现 `Ptr<T>` 的 well-formedness：`T` 必须是 GC-free value type（不允许直接/间接包含 GC ref），并在违反时给出清晰诊断。
- 目标：先做保守检查（宁可拒绝也不放过）；对 `Option<RefType>` 这类也应拒绝（因为表示里含 GC pointer）。
- 验收：unsafe_nogc/typecheck fixture：`Ptr<Int>` 通过；`Ptr<String>`、`Ptr<Option<String>>` 报错（新错误码）并指向 `Ptr<...>` 的类型参数位置。
- 依赖：T0402、T0403、T1003

### T1012 [TODO] Typecheck：指针↔整数转换只能通过 sysroot intrinsics，且必须在 unsafe context（spec §15.9.4 / runtime §5）
- 描述：把“pointer/int casts 的限制点”固定为：仅允许调用 sysroot 提供的转换 intrinsics（例如 `ptrToUIntPtr/uintPtrToPtr`），并要求调用点处于 unsafe context；明确 **不** 把 `as/as?` 当作指针转换。
- 目标：先只做静态门禁与错误信息；不做 codegen（lowering 到 LLVM 留给后续任务）。
- 验收：unsafe_nogc fixture：在非 unsafe context 调用 `ptrToUIntPtr` 报错；在 `@Unsafe { ... }` 内通过；`p as UIntPtr` 不被当作合法指针转换（按普通 cast 规则处理并产生对应诊断/required effects）。
- 依赖：T1010、T1004、T0412

### T1013 [TODO] 注解系统：补齐内建注解与 `AnnotationTarget`（spec §15.5）
- 描述：在 sysroot/typecheck 中补齐内建注解集合：`@TailRec`、`@AllowIntrinsic`、`@Suppress`、`@CLayout`、`@Target`、`@Retention`，并引入 `AnnotationTarget` enum。
- 目标：先固定声明面与最小合法性检查；复杂行为（如真正 TCO）后续由各子系统消费。
- 验收：新增 parse/typecheck fixture：这些注解可被声明/使用；非法 target 名报错。
- 依赖：T1002、T0418

### T1014 [TODO] 注解 use-site targets：`field:/property:/param:/get:/set:/file:`（spec §15.3）
- 描述：支持 use-site target 前缀语法，并在注解附着时区分实际目标元素。
- 目标：先只覆盖 property / param / field / file；getter/setter 的细化可在同任务内保留占位实现。
- 验收：新增 parse/typecheck fixture：`@property:Rename`、`@param:Validated`、`@file:AllowIntrinsic` 可解析并附着到正确目标。
- 依赖：T1001、T1013

### T1015 [TODO] namespaced annotations：`@Namespace.Annotation(...)`（spec §15.4）
- 描述：支持命名空间注解的解析与绑定：例如 `@Serialization.Rename("x")`。
- 目标：先只支持以 path 形式引用注解类；命名空间对象本身的完整语义可与 object 任务联动。
- 验收：新增 parse+resolve fixture：namespaced annotation 可解析并绑定；未定义路径时报错。
- 依赖：T1001、T0258、T0317

### T1016 [TODO] meta-annotations：`@Target/@Retention` 合法性与导出策略（spec §15.5）
- 描述：实现 meta-annotations 的最小规则：`@Target` 限制注解可应用位置，`@Retention` 决定是否仅编译期可见或保留到 `.cone` 元数据。
- 目标：先只支持 comptime-only 与 cone-preserved 两档；更细粒度 policy 后续再补。
- 验收：新增 typecheck + cone fixture：被 `@Target` 禁止的位置报错；保留到 `.cone` 的注解在下游可见。
- 依赖：T1013、T1103、T1209

---

## T11：Cone（包/稳定 IR/分发）（阶段 10）

### T1101 [TODO] Cone.toml：解析 manifest（spec §13.7、PLAN §12）
- 描述：实现 `Cone.toml` 的解析（可用 toml crate），并暴露结构体给 driver。
- 目标：先只解析 package name/version/deps；不实现构建图。
- 验收：新增单测：解析最小 Cone.toml；新增 fixture：带 `Cone.toml` 的 package 目录可被发现。
- 依赖：T0002

### T1102 [TODO] 包加载：按 Cone 目录结构发现源文件（spec §13.2）
- 描述：实现 “package root → sources 列表” 的加载规则。
- 目标：先不做增量编译；不做 sysroot 之外的标准库。
- 验收：新增集成测试：构造临时目录 package，`scoop build` 能找到 main 并 parse/resolve。
- 依赖：T1101、T0805

### T1103 [TODO] scoopir v0：定义稳定 IR schema（仅 public API）
- 描述：定义一个最小可序列化 schema（JSON/CBOR/自定义）表达 public API（类型/函数签名）。
- 目标：先只覆盖 type + fun header；不包含函数体。
- 验收：新增单测：从 HIR/type env 导出 scoopir；快照测试保证 schema 稳定（带版本号）。
- 依赖：T0702

### T1104 [TODO] `.cone` 归档 v0：打包 scoopir 与元数据（PLAN §12.2）
- 描述：用 zip/tar 实现最小归档：包含 `Cone.toml`、`api.scoopir`、sources hash。
- 目标：先只实现写包；读包后续任务。
- 验收：`scoop package`（新命令）能生成 `.cone` 文件并列出内容；新增测试验证归档包含必需文件。
- 依赖：T1101、T1103

### T1105 [TODO] `.cone` 读取：加载 `api.scoopir` 并参与下游类型检查（spec §13.3）
- 描述：实现从 `.cone` 读取 IR 与元数据，把依赖包的 public API 注入 type env。
- 目标：先只支持同平台/同版本；版本兼容后续任务。
- 验收：新增 cone fixture：A 包导出一个类型/函数，B 包依赖 A 并能通过 typecheck。
- 依赖：T1104、T0402

### T1106 [TODO] IR 稳定性与版本协商（spec §13.4）
- 描述：为 scoopir 增加显式版本号，并实现“旧版本可读/不兼容报错”的策略。
- 目标：先只做版本号检查；不实现自动升级。
- 验收：新增单测：构造一个旧版本 header 读取成功或按规则失败；错误码稳定。
- 依赖：T1103、T1105

### T1107 [TODO] consumer 编译与链接流程（多包）（spec §13.3）
- 描述：实现 `scoop build` 能处理依赖图：先加载依赖 cone，再编译当前包，最后链接。
- 目标：先不做增量；先只支持 DAG，无循环依赖。
- 验收：cone fixture：两包依赖编译并链接成可执行，运行输出正确。
- 依赖：T1105、T0806

### T1108 [TODO] pre-specialize：从 Cone.toml 指定常用单态化实例（spec §13.7）
- 描述：支持在 Cone.toml 中列出需要预编译的泛型实例，并在打包时写入 `.cone`。
- 目标：先只支持函数实例；类型实例后续。
- 验收：新增 cone fixture：指定 `id<Int>` 预编译；下游消费时无需再次单态化（可用 dump 日志/计数验证）。
- 依赖：T0712、T1104

---

## T12：编译期执行与反射（阶段 11）

### T1201 [TODO] const fun：在 AST/HIR 中标记 `const fun`（spec §6.2）
- 描述：解析并在 HIR 中标记 const 函数，便于后续解释器选择入口。
- 目标：先只做标记与语法限制检查（如禁止 effect）。
- 验收：新增 parse/typecheck fixture：`const fun add(a:Int,b:Int):Int { ... }`；非法操作在 typecheck 报错（规则可先简化）。
- 依赖：T0246、T0404

### T1202 [TODO] const interpreter v0：只支持 value types + 纯表达式（PLAN §13）
- 描述：实现解释器能执行：整数运算、tuple/struct/enum 构造、`String` 操作、函数调用（仅 const）。`String` 虽为引用类型，但具备值语义，在 comptime 中特殊处理。
- 目标：不支持堆分配（`String` 除外）、不支持 effects、不支持循环（可先限制）。
- 验收：新增 `tests/fixtures/comptime/*`：const 计算结果可用于类型/代码生成位置（可先以编译期常量折叠为目标）。
- 依赖：T1201、T0702

### T1203 [TODO] `comptime { ... }` block 语法与执行入口（spec §6.3）
- 描述：解析 comptime block 并在编译时执行（Pure 限制）。
- 目标：先只支持 block 内 `val` 与 `comptime if`（spec §6.3.2）；`comptime for` 后续。
- 验收：comptime fixture：生成若干声明/常量（即使仅影响常量值）；失败时诊断可读。
- 依赖：T1202

### T1204 [TODO] 反射 intrinsics v0：`nameOf/sizeOf/fieldsOf` 的声明与解释器支持（spec §6.4）
- 描述：在 sysroot 中声明内建反射函数，并在 comptime 执行时实现其行为。
- 目标：先只支持 struct 的字段列表与基本类型 size；RTTI 后续。
- 验收：comptime fixture：基于 `fieldsOf<T>()` 生成序列化代码片段（可先只打印/导出元数据）。
- 依赖：T1203、T0010

### T1205 [TODO] Splice operator（`value.[field]`）最小实现（spec §6.4）
- 描述：支持 `.[field]` 语法，在 comptime for 中通过 FieldMeta 访问值的特定字段（先限用于声明位置）。
- 目标：先只做“生成声明列表”；不做表达式 splice。
- 验收：comptime fixture：通过 splice 生成一个函数/struct，后续解析/类型检查通过。
- 依赖：T1203、T0201

### T1206 [TODO] RTTI v0：为类型生成运行期描述符（spec §6.6）
- 描述：定义运行期类型信息结构（type id/field layout），先能在调试/反射中使用。
- 目标：先只生成静态表；GC trace bitmap 后续。
- 验收：新增 `--dump-rtti`（或测试）可打印某个类型的 RTTI；输出稳定。
- 依赖：T0802、T0409

### T1207 [TODO] `comptime for`：编译期循环（spec §6.3.1）
- 描述：实现 `comptime for` 的语法与执行（对编译期常量范围/列表迭代）。
- 目标：先只支持整数范围与固定数组；break/continue 后置。
- 验收：comptime fixture：用 comptime for 生成重复的字段/函数；生成代码能通过 parse/typecheck。
- 依赖：T1203

### T1208 [TODO] 编译期元数据类型：`TypeMeta/FieldMeta/PropertyMeta`（spec §6.4、§10.4）
- 描述：实现编译期可用的元数据结构（字段名/类型/属性名/owner 等），供反射与委托属性生成使用。
- 目标：先只支持 struct 字段与 class 属性；注解信息后续任务补。
- 验收：comptime fixture：`fieldsOf<T>()` 返回 FieldMeta 列表；并能读取 name/type。
- 依赖：T1204、T0409

### T1209 [TODO] 编译期注解访问（spec §15.6）
- 描述：在 comptime/reflection API 中暴露读取注解的能力（限定在编译期使用）。
- 目标：先只支持读取注解名与字面量参数；复杂表达式参数后置。
- 验收：comptime fixture：读取 `@Deprecated("x")` 的参数并生成代码/诊断；输出稳定。
- 依赖：T1001、T1208

### T1210 [TODO] 委托属性 lowering 所需：生成 `PropertyMeta` 常量并传参（spec §10.4）
- 描述：为 delegated property 生成静态 `PropertyMeta` 常量，并在 getter/setter 转发时传入。
- 目标：先只生成元数据与调用形状；delegate 的具体库实现不要求存在。
- 验收：typecheck/IR fixture：delegated property lowering 后 getter 调用包含 `PropertyMeta` 参数（可用 dump-hir 验证）。
- 依赖：T0434、T1208

### T1211 [TODO] `const fun` 规则：allowed/prohibited 清单的静态 enforcement（spec §6.2）
- 描述：实现 const fun 的限制：禁止 perform/分配/IO/闭包等；允许纯计算、value types 操作与 `String`（值语义特例）。
- 目标：先保守（宁可多报错）；后续逐步放宽。
- 验收：comptime fixture：在 const fun 中调用非 const fun 报错；在 const fun 中进行纯算术通过；`String` 操作通过；使用闭包/lambda 报错。
- 依赖：T1201、T1005

### T1212 [TODO] 运行期值上的反射回退路径（spec §6.4 末尾说明）
- 描述：当反射 API 的 receiver 是运行期值时，遵循 const fun 规则回退为普通运行期调用（不做编译器特殊处理）。
- 目标：先在文档与测试中固定该行为；实现上不需要特殊 case。
- 验收：新增文档/fixture：同一个反射调用在 comptime 与 runtime 下分别走不同路径但语义一致。
- 依赖：T1204

### T1213 [TODO] sysroot：补齐 scope functions 的 effect-polymorphic 签名（spec §11）
- 描述：在 sysroot 中声明 `let/run/also/apply` 的推荐签名（含 `<eff E = Pure>` 与 receiver function type）。
- 目标：先只做声明；标准库实现可后置。
- 验收：typecheck fixture：调用 `x.run { ... }` 的 effect row 会传播（在推断/required effects 上可观测）。
- 依赖：T0509、T0219

### T1214 [TODO] 反射 intrinsics 完整化：`variantsOf/alignOf/superTypesOf/annotationsOf/paramsOf`（spec §6.4 / §15.6）
- 描述：在 sysroot + comptime evaluator 中补齐缺失的反射 intrinsic：`variantsOf<T>()`、`alignOf<T>()`、`superTypesOf<T>()`、`annotationsOf<T>()`、`paramsOf(fn)`。
- 目标：先只覆盖 language spec 已列出的最小集合；复杂跨包元数据后续与 Cone 联动。
- 验收：新增 comptime fixture：读取 enum variants、alignment、super types、函数参数与注解列表；输出稳定。
- 依赖：T1204、T1209、T0418

### T1215 [TODO] 编译期元数据补齐：`VariantMeta/ParamMeta/FunctionMeta/AnnotationMeta/AnnotationArgMeta`（spec §6.4 / §15.6）
- 描述：补齐反射所需的元数据结构，并让它们可在 comptime 中被访问和迭代。
- 目标：先只支持只读 metadata；不支持运行期动态修改。
- 验收：comptime fixture：`variantsOf<T>()` 返回 `VariantMeta`；`paramsOf(fn)` 返回 `ParamMeta`；注解参数可通过 `AnnotationArgMeta` 读取。
- 依赖：T1208、T1214

### T1216 [TODO] `trimIndent()`：编译期求值 + 普通运行期回退约定（spec §8.4 / §6.2）
- 描述：把 `trimIndent()` 纳入 `String` 的 `const fun` 语义：接收者是编译期常量时在编译期求值，否则保留为普通运行期调用。
- 目标：先只覆盖 raw string 与普通 string 的常见路径；不做额外字符串 API 扩展。
- 验收：新增 comptime fixture：raw string `.trimIndent()` 在编译期折叠；运行期 fixture 由 T0827 验证 fallback 路径。
- 依赖：T1202、T1211

### T1217 [TODO] sysroot/stdlib：标准 delegated properties API surface（spec §10.4）
- 描述：在 sysroot 或标准库层补齐 delegated properties 的 API surface：`scoop.delegates.lazy`、`observable`、`vetoable`，以及 map-backed delegate 所需接口。
- 目标：先只固定声明面与最小文档/fixture；行为实现与线程安全语义后续补齐。
- 验收：新增 resolve/typecheck fixture：引用 `scoop.delegates.lazy` 等 API 可通过；缺失导入时报错。
- 依赖：T0451、T1210

---

## T13：Kotlin 语义兼容项（阶段 11+，按需补齐）

### T1301 [TODO] 操作符重载：解析 `a + b` 到约定方法名（Appendix B.8 / PLAN §14）
- 描述：在 typecheck/bind 阶段把二元运算映射到 `plus/minus/...`。
- 目标：先只实现 `+` 与 `-`；只对 struct/class 方法生效。
- 验收：language fixture：自定义 `plus` 后 `a + b` 通过；缺少方法时报错。
- 依赖：T0211、T0302、T0407

### T1302 [TODO] typealias：完善语义（泛型 alias / 跨包导出 / Cone 交互）（Appendix B.10）
- 描述：在已有“最小 typealias 展开”（见 T0446）的基础上，补齐 Kotlin 风格的 typealias 能力：支持泛型 typealias、跨包引用与可见性/导出规则（与 Cone 的 public API 对齐）。
- 目标：不改变既有别名展开语义；重点解决：泛型参数作用域、跨包循环检测、以及在 `.cone` 导出时的表现形式（展开还是保留 alias，需固定策略）。
- 验收：typecheck + cone fixtures：A 包导出 `typealias`，B 包依赖并使用；泛型 alias 可被正确实例化；非法循环在跨包时仍能报错。
- 依赖：T0446、T0218、T1105

### T1303 [TODO] object / companion object（Appendix B.9）
- 描述：按需支持 singleton 对象声明与伴生对象。
- 目标：先只实现语法与 name resolution；codegen 可后置。
- 验收：parse+resolve fixture：object 声明可解析；引用解析正确。
- 依赖：T0201、T0301

### T1304 [TODO] 范围与 for 协议（Appendix B.12）
- 描述：实现 `for (x in range)` 语法与 lowering 到迭代协议（如 `iterator/next/hasNext`）。
- 目标：先只做语法 + typecheck 规则；运行时库后续。
- 验收：language fixture：for 语法可解析并类型检查；缺少迭代协议方法时报错。
- 依赖：T0207、T0407

### T1305 [TODO] 默认参数语义：调用点补齐默认值（Appendix B.5.2）
- 描述：在类型检查/调用解析阶段实现默认参数：调用时省略参数自动补齐默认值表达式。
- 目标：先只支持尾部参数默认值；中间省略与命名参数结合后续。
- 验收：typecheck+run-pass fixture：`fun f(x:Int=1,y:Int=2)` 调用 `f()` 输出 3（或等价行为）。
- 依赖：T0230、T0810

### T1306 [TODO] 命名参数语义：重排与混用规则（Appendix B.5.3）
- 描述：实现命名参数：按参数名匹配并重排；禁止与位置参数混用的非法形式（按 Kotlin 规则）。
- 目标：先不支持 varargs 与命名参数组合。
- 验收：typecheck fixture：`f(y=2,x=1)` 通过；重复命名/不存在参数名报错并指向 name span。
- 依赖：T0231、T1305

### T1307 [TODO] trailing lambda 与类型推断联动（Appendix B.5.4）
- 描述：当 call 最后一个参数是 lambda 时，支持 `f { ... }`/`f(1) { ... }` 并正确进行 lambda 参数下推推断。
- 目标：先只支持单个 trailing lambda；多 trailing lambda 后置。
- 验收：infer fixture：`takes { it }` 推断成功；run-pass fixture：lambda 被正确调用。
- 依赖：T0232、T0504

### T1308 [TODO] varargs：`vararg` 参数与 spread（Appendix B.5.5）
- 描述：解析/类型检查 `vararg` 参数，并支持调用点 `*arr` spread（若语言采用同语法）。
- 目标：先只支持数组/tuple 的最小 spread；集合转换后置。
- 验收：typecheck fixture：vararg 调用通过；不支持 spread 的类型时报错。
- 依赖：T0218、T0407

### T1309 [TODO] 操作符重载：补齐位运算/移位映射（`& | ^ ~ << >>` → `and/or/xor/inv/shl/shr`）（Appendix B.8）
- 描述：在已实现的操作符重载绑定机制上，加入位运算与移位的 operator→方法名映射，并对 unary `~` 映射到 `inv()`。
- 目标：只做“绑定规则”补齐；不引入新的优先级（优先级在 parser 已固定）；不处理复合赋值（如 `shlAssign`）除非 spec 明确要求。
- 验收：language fixture：自定义 `and`/`shl`/`inv` 后 `a & b`/`a << 1`/`~a` 可通过；缺少方法时报错并指向操作符。
- 依赖：T1301、T0211、T0252

### T1310 [TODO] import alias：`import foo.bar.Baz as Qux`（Appendix B.7）
- 描述：在 Kotlin 兼容层补齐 alias import 的完整语义：可见性、shadowing、与普通 import / wildcard import 的交互。
- 目标：在 parser/resolve 子任务基础上补齐语言级规则；不改变既有 import 语义。
- 验收：language fixture：alias import 与普通 import 混用时解析正确；冲突时报清晰诊断。
- 依赖：T0254、T0315

### T1311 [TODO] `object` / `companion object`：补齐类型检查、静态访问与初始化语义（Appendix B.9）
- 描述：在已有 parse/resolve 任务基础上，补齐 `object` / `companion object` 的语言级行为：单例语义、通过类名访问 companion 成员、初始化时机与可见性。
- 目标：不修改既有 T1303；本任务专门把它从“语法/解析”推进到“完整语言语义”。
- 验收：language fixture：top-level object、nested object、named/unnamed companion object 的成员访问和初始化行为符合预期。
- 依赖：T1303、T0452、T0828

### T1312 [TODO] 类初始化语义：property initializer / `init` / secondary constructor 顺序（Appendix B.2.2）
- 描述：补齐 Kotlin-like 类初始化顺序与规则：属性初始化、多个 `init` block、secondary constructor body 的执行顺序和可见性边界。
- 目标：先覆盖单类/单继承常见情况；复杂继承链细节后续扩展。
- 验收：language fixture：初始化顺序输出稳定；非法在初始化早期访问未就绪成员时报错。
- 依赖：T0256、T0257、T0448

### T1313 [TODO] 标准 delegated properties：`lazy` / `observable` / `vetoable` / map-backed（spec §10.4）
- 描述：在语言兼容层补齐标准 delegated properties 的行为与示例，确保 `by` 不只停留在语法和最小接口层。
- 目标：先覆盖最常见的行为：lazy 首次访问缓存、observable/vetoable 回调、map-backed 属性读取；更复杂线程安全语义后续。
- 验收：language/run-pass fixture：`lazy` 只初始化一次；`observable` / `vetoable` 回调按预期触发；map-backed delegate 可读取字段。
- 依赖：T1217、T0434

---

## T14：GC 迁移到 Scoop（阶段 12：自举路线，先铺垫再替换）

### T1401 [TODO] 明确自举前置条件检查清单（PLAN §15.1）
- 描述：把 `@NoGC/@Unsafe/FFI/线程/原子` 的“必须具备项”固化成 checklist 文档/fixtures。
- 目标：先是文档 + 测试，不需要实现 GC。
- 验收：新增 `tests/fixtures/unsafe_nogc/checklist_*.scoop` 覆盖关键限制；文档链接到这些 fixtures。
- 依赖：T1003

### T1402 [TODO] Scoop 侧实现 GC 算法库骨架（仍由 C runtime 驱动）（PLAN §15.2）
- 描述：用 Scoop 编写 mark-sweep 的算法部分（遍历、标记、清扫）并暴露 C ABI glue。
- 目标：只迁移算法，不迁移 OS/线程暂停；保持可回退到纯 C。
- 验收：同一套 `runtime_gc` fixtures 可在 “C-only” 与 “hybrid” 两个 runtime 模式下跑通（先可只跑少量）。
- 依赖：T0904、T1003、T0806

### T1403 [TODO] 类型描述与扫描逻辑迁移到 Scoop（PLAN §15.2）
- 描述：把 trace bitmap/扫描函数从 C 迁移到 Scoop，并保证 ABI 稳定。
- 目标：先只支持 struct/closure env；class/interface 后续。
- 验收：新增 GC fixture：包含嵌套 struct 的分配与回收（需要 run-pass harness，见后续任务）。
- 依赖：T1402、T0905

### T1404 [TODO] 双 runtime 模式：C GC vs Scoop GC 的可切换构建（PLAN §15）
- 描述：让构建系统支持选择 runtime 实现（环境变量/feature）：同一套 fixtures 可在两种 runtime 下跑。
- 目标：先只支持本地切换；CI 双跑后续。
- 验收：`SCOOP_RUNTIME=hybrid cargo run -p scoop -- test` 与默认模式都能跑过至少一组 GC fixtures。
- 依赖：T1402、T0106b2

### T1405 [TODO] 最终替换 C GC：把 GC 驱动层也迁移到 Scoop（PLAN §15.2）
- 描述：把 stop-the-world/线程枚举/OS API glue 逐步迁移，最终 C runtime 只保留极薄启动层或完全移除。
- 目标：以“可回退”为原则，分步骤替换；每一步都有 run-pass 回归。
- 验收：在 Scoop GC 模式下，GC fixtures 与 effect fixtures 均可通过；C runtime 中不再包含 GC 核心算法。
- 依赖：T1404、T0911、T1009
