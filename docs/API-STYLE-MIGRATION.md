# 库 API 风格统一

依据 [API review](API-STYLE-REVIEW.md)，以 Kotlin 的日常调用词汇为主，保留 Scoop 的 Option、显式能力约束和值 codec。每项先修改规范，再迁移实现与仓库内消费者，完成定向验证后独立提交；最后执行 Linux 全量验收。

| 项目 | 实施范围 | 状态 |
| --- | --- | --- |
| R01 | 高层字节输入 List<Byte>，只读与可写数组转换分名 | 已完成；fmt/clippy、23 项定向 fixture（失败已闭合）、3 项平台组合测试通过 |
| R02 | spawn(searchPath = false) 明确启动动作 | 已完成；fmt/clippy、CommandsTest 7 项通过 |
| R03 | IO override 和同步／异步参数名一致 | 已完成；fmt/clippy、BufferedStreamsTest 6 项通过 |
| R04 | OrNull 失败命名与证书验证 Result | 已完成；fmt/clippy、crypto 17 项、外部消费者 4 项、UTF-8 fixture 3 项通过 |
| R05 | Closeable、isClosed 与 TLS takeOwnership 工厂 | 已完成；fmt/clippy、资源 fixture 2 项、platform 110 项、TLS 13 项、async TLS 12 项及外部消费者 3 项通过 |
| R06 | StringBuilder.append/toString 与字节 builder 链式追加 | 待实施 |
| R07 | ValueEncoder/ValueDecoder/ValueInspector 与 Json 边界命名 | 待实施 |
| R08 | 惰性 withIndex 与 copyOfRange 快照 | 待实施 |
| R09 | Duration 趋零分量与 floorSeconds | 待实施 |
| R10 | 普通整数转换名与二进制位宽命名 | 待实施 |
| R11 | IndexGet/IndexSet 运算符接口 | 待实施 |
| R12 | ResourceUseException 继承 Exception | 已完成；fmt/clippy、资源 fixture 的 debug/release 与移动 GC 通过 |
| R13 | 普通签名算法名称与具体密钥生成工厂 | 待实施 |
| R14 | 枚举、查询、Path 扩展、native helper 与公共文档 | 待实施 |

本轮不保留无实际外部兼容需求的旧名称。普通枚举源名字与编码名字的变化在对应规范中明确；协议算法码保留原拼写。时长单位构造保留函数形式，因为它是可能溢出失败的受检转换。

验证记录仅保留范围、结果和限制；原始日志与临时构建目录置于仓库外。工作树内不写入 `/tmp`，清理由本任务创建的构建产物。
