# M23-11 CLI JSON schema 1

每行是一个完整 JSON object，写入 stderr。公共字段 `schema = 1`、`kind`。成功 build/link 的 stdout 为空；run 启动程序后直接继承原始 stdio。

- `diagnostic`：`severity`（error/warning）、`code`、`message`、`origin`、`notes`；工具错误另有 `phase`。每个 note 含独立 `message`、`origin`、`display`，primary 的 `display` 也独立列出。
- `library`：`profile`、`output`、`root`、`dependencies`、`observations`。
- `executable`：与 library 相同，另有 `runtime_index`、`link_plan_fingerprint`。
- `link`：`output`、`root`、`dependencies`、`runtime_index`、`link_plan_fingerprint`；显式 `--dump-plan` 时另有 `link_plan`。

artifact 清单每项含 `coordinate`（group/name/version）、`identity`、`artifact_fingerprint`、`path`。root 与 dependency 的 path 均指向实际持久文件，`output` 是稳定用户产物的位置。`observations.nodes` 每项含 identity、origin（compiled/cache/prebuilt）、cache_key（prebuilt 为 null）；`child_invocations` 保留实际源码编译顺序。

origin 是封闭集合：`{"kind":"none"}`；`{"kind":"source","cone":...,"path":...,"start":...,"end":...}`；`{"kind":"host","path":...,"start":...,"end":...}`；`{"kind":"artifact","path":...,"member":...}`。source 的 path 是 canonical logical path，span 是 UTF-8 字节偏移。artifact 的 member 是 reader 提供的实际结构路径。缺少具体来源的工具错误使用 none，不伪造当前源码位置。

`display` 为 null 或当前调用的展示对象：path、可用时的 line/column，以及真实 span 的 start/end。源码文本未保存时不合成行列号。对 JSON 的语义比较应排除 display；完整 CLI presentation 的比较可以单独规范化当前临时目录。

文件系统 path 通常是 JSON string。非 UTF-8 Unix path 使用 `{"encoding":"unix-bytes","hex":...}`，Windows 非 Unicode path 使用 `{"encoding":"windows-wide","units":[...]}`，不以替换字符破坏后续 I/O。程序参数始终以原始 OsString 传递，不通过 JSON 或 shell 重建。
