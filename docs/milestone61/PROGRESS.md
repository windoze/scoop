# M61 实施进度

状态：功能实现完成，正在收尾目标链接与发行资源验证。readline 基线为 `2ac608841`。

## 实现范围

- IDNA／Punycode 使用 idna 1.1.0，NFC、字素分段、大小写与属性使用 ICU4X 2.3.0，标量宽度使用 unicode-width 0.2.2；固定 Unicode 数据为 17.0.0。
- IDNA 采用 UTS #46 non-transitional、STD3、CheckHyphens 与 DNS 长度检查，根点、单标签和错误映射由 Scoop 适配。允许的 profile 差异及 panic／OOM 边界已写入规范。
- 按用户决定保留纯 Rust，encoding_rs 0.8.42 与基础标量适配支持 43 个 canonical name，退役 65 个旧名称；集合与映射差异见 [编码列表](../../sysroot/src/scoop.charset/ENCODINGS.md)。增量、BOM、严格／有损及原始错误偏移合同保留。
- readline 的 Unicode 调用已迁入共同 Rust 后端。GNU libiconv、libcharset、libunistring、libidn2 的源码、维护构建和链接依赖已移除。
- 46 项锁定依赖的源码和许可随附于 vendor；普通应用消费预编译目标归档，不运行 Cargo。

## 验证范围与结果

测试使用 scoop.test，不新增 fixture。按用户要求，已移除底层补缺专测，以及新增的上游 Unicode 兼容性／完备性正负例和官方一致性数据。保留 Scoop 自有适配、状态与资源生命周期及 readline 真实编辑行为；不扩展或重跑 HTTP client 组合测试。

- Darwin、Linux GNU、Linux musl 的 Rust 资源准备、Scoop 库消费及 readline 行为测试已完成。readline 每目标 39 个主用例与 6 个 PTY 子用例通过；musl 的 PTY 辅助代码先清零 termios，再比较实际返回字段，相关 4 项重测通过。
- 已完成的 IDNA 11 项和 URL 14 项三目标消费者结果复用。本轮清理后只复测受影响的 Scoop 适配与 C ABI，不重跑上游规则检查。
- 两个正式链接入口、准备复用与发行资源收尾结果待补记。Linux 已完成的消费者没有原 GNU 文本依赖；GNU 使用既有系统库，musl 为静态链接。
- Rust fmt／clippy、Python ruff 0.16.10 与 API 文档零警告检查已完成；本轮测试清理继续执行相关格式与编译检查。

原始日志及工作目录位于仓库外 `../scoop-work/m61`，不提交验证日志。
