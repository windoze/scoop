# Scoop fixture schema 1

运行 `python3 tests/run_fixtures.py --all` 完整验收。开发时可用
`--suite tests/fixtures/m23-cli` 或 `--filter 'cli-*'`；`--list` 使用同一发现与校验。
所有被扫描的 `.scoop` 必须归属于声明的 `inputs` 或 `support`，支持文件不单独计为通过。
缺少工具、期望文件、未知字段、嵌套类型错误、重复载体与前向步骤引用均报配置／环境错误。
Python 3.11+ 只用标准库；格式化／lint 使用 `tests/requirements-dev.txt` 固定的 Ruff。
默认最多并行运行四个用例，可用 `--jobs N` 调整，`--jobs 1` 顺序运行。
每个用例的步骤与变体保持有序；报告按发现顺序保存。中断时清理运行中的进程，
未完成项标记 `interrupted`，退出码为 130。

每个用例选择一种载体：相邻 `name.fixture.toml`、目录中的 `fixture.toml`，
或源码开头 `// fixture:begin` 到 `// fixture:end` 之间每行带 `//` 的 TOML。
源码注释不会被删除，编译器读取原始源码。相对文件名以描述所在目录为基准。
发现阶段仅解码内联条件注释；源码正文中的非法 UTF-8 留给实际 compiler 诊断。

```toml
schema = 1
name = "example"
root = "hello.scoop"
inputs = ["hello.scoop"]
tools = ["scoop"]
targets = ["aarch64-apple-darwin"]
tags = ["cli"]

[[steps]]
name = "build"
argv = ["${scoop}", "build", "${root}", "--sysroot", "${sysroot}",
        "--cache-dir", "${cache}", "--target-dir", "${work}/target",
        "--emit", "all", "--dump-dir", "${work}/dumps", "--message-format", "json"]
exit = 0
stdout = ""
stderr = ""
json = "stderr"
diagnostics = []
checks = [{actual = "${build.result.kind}", equals = "executable"}]

[[steps]]
name = "run"
argv = ["${build.result.output}"]
exit = 0
stdout = "hello\n"
stderr = ""
```

顶层还可声明 `support`、`vars`、`variants`；每个变体只有 `name`、`vars`、`env`。
未写 variants 时运行 `normal`。变体按顺序执行，各有独立 `${work}`，共享本次私有
`${cache}` 与 `${sysroot}`。例如 `env = { SCOOP_GC_STRESS_MOVE = "1" }` 只影响该变体。
多 Cone 提交真实 `Cone.toml`，按普通 `copy` 步骤布置，runner 不合成入口或 manifest。
native companion 显式作为 input，并用普通 argv 步骤调用 `${cc}`、`${ar}`，指定
`-isysroot ${sdk}`、`-mmacosx-version-min=${deployment}` 和输出；随后传 `--library-path`。

`${fixture}`、`${root}`、`${repo}`、`${runtime}`、`${target}` 也是内置值；三个 Scoop
工具默认从 `target/debug` 取得，可用 CLI 参数或 `SCOOP_TEST_PAIRED_*` 指定。
`${step.result.output}` 等引用前一步实际 JSON；数组使用 `.0`，`.length` 取长度。
文本或 bytes 的 `.hex` 返回十六进制字节表示；文本使用文件系统编码并保留原始字节，
可组合 `${work.hex}2fff` 这类期望来比较 JSON 中的非 UTF-8 路径。字典的 `hex` 字段仍按普通字段读取。
传作 argv 时先在 vars 中声明 `raw_hex = "${work.hex}2fff"`，再写 `{hex = "${raw_hex}"}`；
静态 hex 或完整引用使用相同的预检和执行规则。
`${variants.normal.build.result.root.artifact_fingerprint}` 可比较前一变体。
整个字符串为引用时保留 JSON 类型。argv 可用 `{hex = "ff"}` 传递原始字节，
或 `{each = "${build.result.dependencies}", field = "path", prefix = "--dependency-slib"}`
展开实际清单。程序 JSON 中的非 UTF-8 path carrier 保留字节，不经过 shell。

每个进程必须声明 `exit` 或 `signal`（如 `"SIGABRT"`）之一，以及完整 stdout/stderr。
stdin 默认空；cwd 默认 `${work}`；env 继承调用者并应用显式键；timeout 默认 120 秒。
字节期望和 stdin 可写文本、`{hex = "..."}` 或 `{file = "expected.stdout"}`。
`json = "stderr"` 解析 Scoop schema 1 前缀，遇到成功结果即停止，后续字节归程序。
这种模式必须声明完整 `diagnostics` 数组或 JSON 期望文件；仅剥离 display，不改变
canonical source、span、code、message 或 notes。stdout/stderr 仍严格比较剩余原始字节。
`json = "stdout"` 用于普通工具的逐行 JSON，保留原 stdout。步骤结果还包含
`records`、`diagnostics`、`returncode`、`pid`、`stdout`、`stderr`、`raw_stderr`。

每一步的 `checks` 选择 `actual`（JSON 引用）、`file` 或 `glob`（排序后的路径数组），并选择一个比较：
`equals`、`not_equals`、`contains`、`not_contains`；文件还支持 `exists`、`type`、`sha256`、
`same_as`、`different_from`。`snapshot = "expected.hir.txt"` 比较完整文本或 JSON。
只有 `normalize = ["paths", "newlines"]` 两种公共非语义规则。显式
`--update-snapshots` 仅更新选中的 snapshot 文件，不改 exit/signal、诊断和普通输出期望。

文件准备步骤使用 `files = [{copy = {from = "...", to = "..."}}]`。
其余动作及必需参数：`move/symlink/hardlink(from,to)`、`remove/mkdir/touch(path)`、
`write(path,data)`、`replace(path,old,new)`（必须唯一匹配）、`patch(path,offset,hex)`
（不可越界）、`chmod(path,mode)`（八进制字符串）。修改均应指向本次私有工作区。
`concat(path,parts)` 按数组顺序拼接已有字节载体，不自动添加分隔符。例如
`parts = [{file = "first.scoop"}, "\n", {file = "second.scoop"}]` 保留原单文件组合用例；
先读取所有部分再写目标，输入缺失时不覆盖已有目标。原输入仍需声明在 `inputs` 中。
`remove` 删除目录树，或解除普通文件、符号链接和命名管道，不打开文件内容。
`truncate(path,size)` 将现有文件缩短为指定非负字节数；超过当前文件长度时报配置错误，
不扩展或修改文件。它与精确 byte patch 共用于 archive／cache corruption 用例。

并发进程声明 `background = true`，其结果期望在后续 `wait = "step"` 时检查。
`await_file = "${work}/ready"` 等待被测程序写出的就绪文件；
`send_signal = {process = "step", signal = "SIGTERM"}` 只向那个进程发送真实信号。
所有后台步骤必须有 wait。失败或中断时 runner 清理自己启动的进程并收割。
wait 之前只有后台进程的 `.pid` 可引用；result／stdout 等要等完成后才可用。
变量按每个变体分别校验，不能借用其他变体才声明的变量。
长诊断 JSON 文件在发现时加载和校验引用，避免启动编译后才发现配置错误。
不允许逐 case Python/shell 编排脚本、动态 predicate 或专用注册表。

报告单列用例、变体、进程和 golden 数量，不将支持文件或目标不适用项计作通过。
`--work-dir` 或失败时保留 report 与工作区，`--keep` 也可用于审阅成功运行。
