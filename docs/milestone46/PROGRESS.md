# M46 实施记录

基线：`e548b9ebe`。按用户要求先完成整个 M46，再开始 M38；M38 的已有设计修改保持原样。

## M46-1：稳定解码视图

- 语言规范 11.13.2 明确稳定输入、重复标量／字段读取、独立 unkeyed 游标及局部 end；core 增加普通 isNull requirement。
- JSON 解码去除 selected、consumed、pending 和全树消费状态；保留完整 parser 检查与序列长度检查。keys 每次提供独立快照。
- 独立 fixture 覆盖标量错误后重试、missing/null/value、重复字段、end 后读取、独立游标；组合 fixture 覆盖合成候选、default 次数、tuple 精确长度及 moving GC。缺失 isNull 的实现有准确编译诊断。
- 同步既有 decoder 实现及旧一次性消费断言。没有修改 runtime、IR、产物格式或编译器合成逻辑。
- 本机 cargo fmt、全工作区 clippy、debug/release 三个 CLI 工具构建通过。相关 59 个 fixture 全部闭合：初轮 25 项通过后切到优化版编译器，其余 34 项中 31 项直接通过，3 项新用例修正 lambda 裸 return 后复验通过；每个运行型新用例同时执行普通和 moving GC。
- 原始报告：[初轮](validation/decoder-initial-darwin.json.gz)、[剩余项](validation/decoder-remaining-darwin.json.gz)、[修正复验](validation/decoder-retry-darwin.json.gz)。初轮中断不计通过；三项源码语法错误及后续通过均保留。未改 Rust／runtime，本批不运行无关 Rust 全量；Linux 完整验证放在本机功能完成之后。

## M46-2：共享编码容器与提交

- 语言规范 11.13.2 区分当前完成与最终提交。JSON 编码器用封闭的容器类别保存同类共享视图，record／sequence 在 end 后可继续写入，新写入使旧完成状态失效。
- pending 子入口在父级推进或 end 时提交，root 在 codec 返回后提交；提交后的 writer 不能修改输出。重复 key、跨类别请求、单值重复写入及未完成值仍有带 path 的错误。容器代码拆到 encoding_containers.scoop，两个实现文件分别为 99／64 行。
- 五个独立／组合 fixture 覆盖 keyed／unkeyed、多视图交替、空容器、end 后追加、合成 flatten／tagged、根与子 writer 过期引用、错误边界及 moving GC。
- 本机 fmt／全工作区 clippy 通过；18 项 CLI fixture 全部通过（5 项新编码、5 项解码协议回归、8 项既有编码／JSON／产物组合）。首轮被 JSON enum 的裸绑定 warning 阻断，改用已有明确匹配及取值形式后全部通过。报告：[初轮](validation/encoder-initial-darwin.json.gz)、[完成验证](validation/encoder-verified-darwin.json.gz)。

## M46-3：Serialize、nullable Option 与字段缺省

- 语言规范 11.13 和实现规范 2.17 先定义统一 Serialize、缺失／null、required／emitNull／ignore、直接 None default 的保守省略及泛型声明处规则；core 移除 SerialName／Transient，Option 的普通 codec 改为 nullable 值。
- HIR 合成复用现有 annotation 静态值、原声明类型和已绑定 default body；没有新增 IR／产物 section。编码只读字段一次，省略分支包住 child 创建及 codec 求值；解码保留先读取、end、依次 default、正常构造的顺序。原 T 与 T? 的区别在具体目标和依赖产物消费时均保留。
- 新增 8 个运行型 fixture（每项含普通及 moving GC）和 10 个准确 negative fixture，覆盖字段配置、default 副作用、完整 Option codec 注入、泛型／alias、class／enum／tuple、三态用户 enum，以及三 Cone 无源码消费与重导出。既有功能用例迁移注解和 Option wire 表示，不保存普通功能 IR 全量快照。新增默认值辅助模块 117 行，修改的合成子模块最大 208 行。
- 本机 fmt／全工作区 clippy、release 三个 CLI 工具构建及 [47 项相关 HIR 测试](validation/serialize-hir-darwin.log.gz) 通过。18 项新 fixture、156 项相关旧 fixture、6 项 std codec fixture 全部闭合，共 180 项：旧回归首轮 136 项通过，19 项诊断期望随改名／位置更新，1 项旧序列消费约束断言按 M46-1 修正，20 项复验全通过。新用例修正一次 lambda statement 和测试库构造器可见性；negative 初轮用于取得并逐项审查准确诊断，10 项复验全通过。
- 原始报告：[新功能首轮](validation/serialize-initial-darwin.json.gz)、[三态／产物首轮](validation/serialize-artifacts-initial-darwin.json.gz)、[产物通过与 negative 初轮](validation/serialize-negative-initial-darwin.json.gz)、[negative 复验](validation/serialize-negative-verified-darwin.json.gz)、[旧回归首轮](validation/serialize-regression-initial-darwin.json.gz)、[20 项复验](validation/serialize-regression-verified-darwin.json.gz)、[std codec](validation/serialize-stdlib-darwin.json.gz)。原始失败保留，不计作通过。

## M46-4：不可变 JsonValue 与树值入口

- 语言规范 11.13 增加公共不可变 JsonValue class、普通 companion codec 和 Json.encodeValue／decodeValue。现有树重命名为 internal JsonNode，公开 wrapper 只有 internal 构造器及节点，不公开可变集合；文本与树值入口共用根编码／提交和 decoder。
- 捕获直接共享既有合法节点；编码逐层使用普通容器，数字沿 Cone 内部精确 token 写入。没有新增 core operation、runtime ABI、反射、平行树或出口重新解析。新 codec 文件 45 行，JSON 实现子文件均不超过 112 行。
- 5 个新运行型 fixture 含普通／moving GC，覆盖全部 JSON 节点、超出 ULong／Double 的数字、负零与长小数、字符串与对象区别、子树错误后复读、延迟 typed 解码、未知字段 LinkedHashMap、allow／deny／collect、flatten 冲突、过期 writer、格式入口不匹配、非法未知子树，以及三 Cone 无源码生产／消费。1 个 negative fixture 精确断言节点不能被外部访问。
- 本机 fmt／全工作区 clippy 通过；6 项新增用例和 13 项必要既有回归全部通过。首轮 3 项通过，1 个新用例错误地要求 JsonValue 具有 Equals，改用普通 Option 模式检查后通过；没有为测试增添公开相等性能力。跨产物通过，访问负例取得并审查诊断后复验通过。报告：[首轮](validation/values-initial-darwin.json.gz)、[产物与修正用例](validation/values-artifacts-darwin.json.gz)、[访问负例复验](validation/values-hidden-darwin.json.gz)、[既有回归](validation/values-regression-darwin.json.gz)。本批没有 Rust／runtime 实现改动，不重复相关 Rust 测试。

## M46-5：离线形状基线

- Cone 等价 JSON 已完成普通 Scoop DTO、稳定视图包装校验、动态依赖 object、短长式依赖和 target 条件。完整配置、默认配置、显式空数组、全部 library kind、条件 AND／OR、typed 往返及 67 个准确错误路径覆盖 T1～T9；没有新增 Cone.json 构建入口或移植构建器。源码按职责分为 10 个文件，各不超过 91 行。映射见 [COVERAGE.md](COVERAGE.md)。
- 本机 fmt／全工作区 clippy 通过；Cone fixture 普通模式执行全部输入，moving GC 执行完整正向组合和首末两个代表性失败，合计约 37 秒通过。初轮修正 enum payload 的 warning 和测试中 null 字段路径误写；全错误矩阵的 moving 重复成本过高，主动中断后缩减到代表组合，普通错误矩阵保持完整。
- 报告：[编译首轮](validation/cone-initial-darwin.json.gz)、[运行首轮](validation/cone-runtime-initial-darwin.json.gz)、[路径诊断](validation/cone-path-darwin.json.gz)、[搬迁测试中断](validation/cone-moving-interrupted-darwin.json.gz)、[完成验证](validation/cone-verified-darwin.json.gz)。失败和中断不计为通过。
- Chat 基线完成 C1～C8：普通模式执行全部 typed 往返和 12 个准确错误路径，moving GC 执行完整多模态请求组合；本机 fmt／全工作区 clippy 和 fixture 通过，约 121 秒。源码按职责拆分，各不超过 114 行。没有生产库改动。
- Chat 首轮修正测试的泛型函数声明、Array 完整类型实参、Option<Array> 无相等性及 enum 裸绑定 warning。完整正向矩阵在普通模式通过后，中断成本过高的重复搬迁矩阵，改为代表组合后复验通过。[语法首轮](validation/chat-initial-darwin.json.gz)、[类型首轮](validation/chat-typed-initial-darwin.json.gz)、[搬迁中断](validation/chat-moving-interrupted-darwin.json.gz)、[完成验证](validation/chat-verified-darwin.json.gz)均保留。
- Responses 基线完成 R1～R7：15 类 mixed item、7 类工具、16 个完整事件、动态 typed prompt variables、缺失 type 的简式消息与 12 个错误路径。本机 fmt／全工作区 clippy 和普通／代表 moving GC 首轮通过，约 184 秒；1137 行源码按职责分为 21 个文件，最大 111 行。[原始报告](validation/responses-verified-darwin.json.gz)。本批没有生产库修改。
- Messages 基线完成 A1～A8：15 块混合内容、7 类工具、5 类引用、14 个完整事件、递归工具结果、双层 discriminator、signature／opaque 数据与 15 个错误路径。官方 schema 复核发现 nullable diagnostics 实为 tagged object，补齐 CacheMiss 六分支及 null 后复验；没有把已知结构当作开放 JSON。22 个源码文件共 1107 行，最大 103 行。本机 fmt／全工作区 clippy、普通／代表 moving GC 通过，约 139 秒。[首轮](validation/messages-initial-darwin.json.gz)、[完成验证](validation/messages-verified-darwin.json.gz)。
- 四组基线及 B1～B16 组合证据已关联到 [COVERAGE.md](COVERAGE.md)。2026-10-15 的 [49 项 fixture 公共规则测试](validation/runner-darwin.log.gz)通过；`--all --list` 验证 4091 项声明与全部源码归属。此项不冒充 4091 项执行通过。

## M46-6：跨 target 验收（执行中）

- 本机各新增功能和必要既有回归已分批闭合。Linux 计划运行 Rust workspace、公共 fixture 规则及 GNU 完整文件 fixture；musl 运行 M46 全部与序列化、浮点数、stdlib codec 等必要回归，保留原始报告。
- 2026-10-15 初次连接 nuc12 超时；用户恢复环境后连接成功。远端从同一基线切到 `codex/m46-m38`，同步至 `e3348cbc`；显式选择已有 Cargo 1.99.0／Rust 1.99.0 和 LLVM 22.1.2，不使用默认 PATH 中的 LLVM 21。见[环境记录](validation/environment-linux.log.gz)。
- Linux [fmt／全 workspace clippy](validation/clippy-linux.log.gz)、[release 三个 CLI 构建](validation/build-linux.log.gz)、[49 项公共规则测试](validation/runner-linux.log.gz)均通过；复用已安装的私有 LLVM libunwind，GNU PIE、musl static／PIE 的[实际 cleanup／catch／delete 检查](validation/unwind-linux.log.gz)通过。
- GNU 完整发现 4091 项，30 项 target 不适用。初轮 4 路并行完成 37 项后改为 12 路继续剩余项；恢复时发现用例序号目录与旧只读产物冲突，保存报告后隔离旧 cases，仅复用编译缓存。权限失败项已在干净用例目录通过。保留[初轮中断](validation/gnu-initial.json.gz)及[恢复目录冲突](validation/gnu-resume-initial.json.gz)，中断和失败不计作通过。GNU 剩余项仍在运行。
- Rust workspace 的前 24 组共 5570 项通过，最后的 driver debug 端到端测试运行约 10 分钟后中止，保留[原始日志](validation/rust-initial-linux.log.gz)。只将 driver library 改为 release、4 路测试线程继续验收，之后补 driver binary 与 workspace 文档测试，不重复已通过的其他 crate。
- musl [224 项首轮](validation/musl-initial.json.gz)中 220 项通过，std codec 错误矩阵与三个 API 基线在 build 步骤触发 300 秒时限。隔离旧 cases 后串行复验，std codec 约 38 秒通过，Chat 仍超过 300 秒；[复验原始报告](validation/musl-retry-initial.json.gz)保留失败与中断。临时给三个大型 DTO 的 build 设置 600 秒，并将该串行 runner 限定到 nuc12 的性能核 0～11，Chat 仍[超时](validation/musl-timeout600.json.gz)。停止其余两项复验，检查实际热点；临时时限已撤回，保留原 300 秒默认时限。[临时配置的本机 clippy](validation/final-clippy-darwin.log.gz)也归档。

### 大型 DTO 暴露的 image 注册表查询开销

- Linux 慢编译的[实际调用栈](validation/chat-backtrace.log.gz)定位到 image safepoint 注册表验证：每个表项线性扫描整个 relocation、Strong binding 及符号集合。实现规范先明确复用查询索引；实现对本 atom 的 relocation 按 offset 查询，复用既有 `bindings_at`，并为 member 的符号表序号建立一次索引。保持原有数量、唯一 offset、target、role、宽度及编码值检查，不改变产物格式、ABI 或语言语义。
- 本机 fmt、[全 workspace clippy](validation/registration-clippy-darwin.log.gz)、[571 项 slib 测试](validation/registration-tests-darwin.log.gz)通过；补充注册数量与错误 target owner 负例、符号序号查询断言。新 CLI 的 [Chat／Serialize 产物／JsonValue 产物 fixture](validation/registration-fixtures-darwin.json.gz)均通过，包含普通运行和 moving GC。Chat 在新工作区约 163 秒，其余两项约 17／18 秒；此值不与旧暖缓存用例直接作性能比。Linux 将使用修复后的工具继续验收。

## 后续

完成 M46-6 并提交验收记录后才开始 M38。

## 构建目录

开始时 target 占用约 111 GB；使用 cargo clean 清理全部可重建产物，Cargo 报告移除 241,742 个文件、154.8 GiB 逻辑字节。重新构建时禁用 incremental，首次重建后目录约 3.8 GB；M46-3 完成本机构建及相关 Rust 测试后约 7.3 GB，继续禁用 incremental。

Linux 验收前远端 target 实占 68 GB；恢复旧 fixture 只读目录的 owner 写权限后完成 cargo clean，保留 sysroot/native 中的私有 unwind。重新构建禁用 incremental 和 dev／test debuginfo，release 三工具及 clippy 完成时 target 约 880 MB。

验收期间 target 增至约 33 GB 后，按 runner 已报告 passed 的用例名与固定选集序号对应关系，清理 326 个已完成的隔离 cases 目录，降至约 14 GB。保留共享 cache、失败现场、活跃用例目录及原始报告，不改 runner 或缓存合同。
