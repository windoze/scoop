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

## M46-5：离线形状基线（实施中）

- Cone 等价 JSON 已完成普通 Scoop DTO、稳定视图包装校验、动态依赖 object、短长式依赖和 target 条件。完整配置、默认配置、显式空数组、全部 library kind、条件 AND／OR、typed 往返及 67 个准确错误路径覆盖 T1～T9；没有新增 Cone.json 构建入口或移植构建器。源码按职责分为 10 个文件，各不超过 91 行。映射见 [COVERAGE.md](COVERAGE.md)。
- 本机 fmt／全工作区 clippy 通过；Cone fixture 普通模式执行全部输入，moving GC 执行完整正向组合和首末两个代表性失败，合计约 37 秒通过。初轮修正 enum payload 的 warning 和测试中 null 字段路径误写；全错误矩阵的 moving 重复成本过高，主动中断后缩减到代表组合，普通错误矩阵保持完整。
- 报告：[编译首轮](validation/cone-initial-darwin.json.gz)、[运行首轮](validation/cone-runtime-initial-darwin.json.gz)、[路径诊断](validation/cone-path-darwin.json.gz)、[搬迁测试中断](validation/cone-moving-interrupted-darwin.json.gz)、[完成验证](validation/cone-verified-darwin.json.gz)。失败和中断不计为通过。
- Chat 基线完成 C1～C8：普通模式执行全部 typed 往返和 12 个准确错误路径，moving GC 执行完整多模态请求组合；本机 fmt／全工作区 clippy 和 fixture 通过，约 121 秒。源码按职责拆分，各不超过 114 行。没有生产库改动。
- Chat 首轮修正测试的泛型函数声明、Array 完整类型实参、Option<Array> 无相等性及 enum 裸绑定 warning。完整正向矩阵在普通模式通过后，中断成本过高的重复搬迁矩阵，改为代表组合后复验通过。[语法首轮](validation/chat-initial-darwin.json.gz)、[类型首轮](validation/chat-typed-initial-darwin.json.gz)、[搬迁中断](validation/chat-moving-interrupted-darwin.json.gz)、[完成验证](validation/chat-verified-darwin.json.gz)均保留。
- Responses 和 Messages 尚待完成，M46-5 尚未完成。

## 后续

继续 M46-5 三组 API，然后进行 M46-6 Linux 完整验收。M38 尚未开始。

## 构建目录

开始时 target 占用约 111 GB；使用 cargo clean 清理全部可重建产物，Cargo 报告移除 241,742 个文件、154.8 GiB 逻辑字节。重新构建时禁用 incremental，首次重建后目录约 3.8 GB；M46-3 完成本机构建及相关 Rust 测试后约 7.3 GB，继续禁用 incremental。
