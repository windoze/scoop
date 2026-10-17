# 库 API 风格统一

依据 [API review](API-STYLE-REVIEW.md)，以 Kotlin 的日常调用词汇为主，保留 Scoop 的 Option、显式能力约束和值 codec。每项先修改规范，再迁移实现与仓库内消费者，完成定向验证后独立提交。下表记录各项实施时的定向验证，最终整体验收见后文。

| 项目 | 实施范围 | 状态 |
| --- | --- | --- |
| R01 | 高层字节输入 List<Byte>，只读与可写数组转换分名 | 已完成；fmt/clippy、23 项定向 fixture（失败已闭合）、3 项平台组合测试通过 |
| R02 | spawn(searchPath = false) 明确启动动作 | 已完成；fmt/clippy、CommandsTest 7 项通过 |
| R03 | IO override 和同步／异步参数名一致 | 已完成；fmt/clippy、BufferedStreamsTest 6 项通过 |
| R04 | 可选失败命名与证书验证 Result | 已完成；fmt/clippy、crypto 17 项、外部消费者 4 项、UTF-8 fixture 3 项通过；None 命名的后续修正见下文 |
| R05 | Closeable、isClosed 与 TLS takeOwnership 工厂 | 已完成；fmt/clippy、资源 fixture 2 项、platform 110 项、TLS 13 项、async TLS 12 项及外部消费者 3 项通过 |
| R06 | StringBuilder.append/toString 与字节 builder 链式追加 | 已完成；fmt/clippy、配套 CLI 构建及 51 项定向 fixture 通过（迁移遗漏已闭合） |
| R07 | ValueEncoder/ValueDecoder/ValueInspector 与 Json 边界命名 | 已完成；fmt/clippy、47 项 Rust 测试、181 项本机适用 fixture 通过（诊断与别名迁移遗漏已闭合）；musl 用例已由最终 Linux 验收覆盖 |
| R08 | 惰性 withIndex 与 copyOfRange 快照 | 已完成；fmt/clippy、12 项迭代、Sequence、有序集合及组合 fixture 通过 |
| R09 | Duration 趋零分量与 floorSeconds | 已完成；fmt/clippy、17 项 fixture、7 项平台时间／同步测试、3 项异步计时器测试通过；组合用例发现的聚合临时引用来源问题已修复，3 项 Rust 回归通过 |
| R10 | 普通整数转换名与二进制位宽命名 | 已完成；fmt/clippy、124 项 Rust 定向测试、159 项本机适用 fixture、3 项原生缓冲区数值测试通过；musl 用例已由最终 Linux 验收覆盖 |
| R11 | IndexGet/IndexSet 运算符接口 | 已完成；fmt/clippy、14 项下标、Map/Set 组合、跨库及重建 core fixture 通过 |
| R12 | ResourceUseException 继承 Exception | 已完成；fmt/clippy、资源 fixture 的 debug/release 与移动 GC 通过 |
| R13 | 普通签名算法名称与具体密钥生成工厂 | 已完成；fmt/clippy、crypto 17 项、外部消费者 5 项、OpenSSL 独立验签 1 项通过 |
| R14 | 枚举、查询、Path 扩展、native helper 与公共文档 | 已完成；fmt/clippy、133 项 fixture（迁移遗漏已闭合）、54 项 Rust 定向测试、platform 110 项、asyncio 文件组合 3 项、crypto 消费者 5 项及 TLS 13 项通过 |

本轮不保留无实际外部兼容需求的旧名称，调用方与依赖产物需要同步重编译。普通枚举源名字与默认派生编码名字一起迁移；协议算法码按对应规范保留原拼写。时长单位构造保留函数形式，因为它是可能溢出失败的受检转换。

初次风格统一的本机定向验证及 Linux 验收已完成。公共约定见 [Scoop 库 API 风格](specs/SCOOP-API-STYLE.md)。Linux 使用 x86-64 GNU／musl、LLVM 22.1.2 与 Rust 1.99；本机目标为 aarch64-apple-darwin。

| 初次完整验证范围 | 结果 |
| --- | --- |
| 格式化、workspace clippy 与三个配套 CLI 构建 | 本机与 Linux 均通过 |
| Linux Rust workspace，release profile | 4,959 项测试通过；全部 crate 的文档测试检查已执行 |
| Fixture runner 公共规则 | 50 项通过 |
| GNU 完整 CLI fixture 集 | 4,204 项全部纳入：4,174 项通过，30 项按目标声明不适用；覆盖 4,727 个变体、16,521 次进程调用 |
| musl API、优化、native、字节与日期相关 CLI fixture | 186 项纳入：175 项通过，11 项按目标声明不适用；覆盖 254 个变体、939 次进程调用 |
| GNU／musl 库及消费者矩阵 | 每个目标的 216 项测试均覆盖 debug、release、移动 GC，合计 1,296 个测试／目标／模式组合；GNU 的单项复验限制见下文 |

库矩阵包含 std、platform、asyncio、crypto、tls、tls.asyncio 六个生产库及四组外部消费者；core、json、cli、test 由完整 CLI fixture 集覆盖。十个新增 API 组合 fixture 在本机、GNU、musl 均通过。异步 TLS 的移动 GC 验证按六个现有测试容器分组执行。

Rust 验收按包保留已完成结果，修正后的 HIR 全组及此前未执行的组继续验收。GNU CLI 的七项失败和六项准备错误、musl 的六项失败，均在修正后定向复验通过；最终结果按用例名称合并完整报告与复验报告。修正内容为 core 别名导入、重建模板、诊断位置和优化期望等迁移遗漏。最小数值夹具改为读取真实标量目标型，本机与 Linux 的 1,474 项 HIR 测试通过，23 份无关 golden 无需扩张。

限制：GNU 在移动 GC 下整组执行 asyncio 时，排队写入用例触发自身的 15 秒时限，其余 44 项通过；该用例在相同模式下独立运行 0.413 秒通过。因此该组按 44 项成功结果加一项定向复验计入覆盖，保留整组超时记录，用例及其时限未调整。musl 同组 45 项整组通过。

后续命名修正：Scoop 的缺失值是 None，因此全库的 OrNull 改为 OrNone，mapNotNull／filterNotNull 改为 mapNotNone／filterNotNone；同步更新规范、R04 建议、调用方和浮点 fixture 生成器。此项只改名，保留 Option 层次、约定失败和异常传播；JSON null 与 FFI 空指针术语继续表达各自的领域含义。

该修正通过本机 fmt、workspace clippy 和固定版本 Ruff 格式化／lint；Darwin、GNU、musl 各 61 项定向 fixture 全部通过，每个目标覆盖 82 个变体、307 次进程调用，包含错误诊断、嵌套 Option、异常传播、跨 Cone 消费和移动 GC。另在本机完成六个生产库及四组外部消费者的全部 216 项 debug 测试。无失败或期望调整；本次未重复初次全量验收。

本记录仅保留范围、结果和必要限制；原始日志与逐次 JSON 报告置于仓库外。临时文件通过 TMPDIR 写入本任务目录，构建产物按完成阶段清理，配套命令单独保留。
