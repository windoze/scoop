# M61 实施进度

状态：已完成（2026-10-22），readline Unicode 后续迁移也已完成。readline 基线为 `2ac608841`。

2026-11-02 后续调整：实际 GUI／HTTP 链接暴露未使用的 Rust personality，文本后端改用 no_std + alloc。功能、版本和 Unicode 数据不变；新构建与平台验证单独记录在 [UI 实施记录](../ui/PROGRESS.md)，下列历史多平台结果不代替新归档的验收。

## 实现范围

- IDNA／Punycode 使用 idna 1.1.0，NFC、字素分段、大小写与属性使用 ICU4X 2.3.0，标量宽度使用 unicode-width 0.2.2；固定 Unicode 数据为 17.0.0。
- IDNA 采用 UTS #46 non-transitional、STD3、CheckHyphens 与 DNS 长度检查，根点、单标签和错误映射由 Scoop 适配。允许的 profile 差异及 panic／OOM 边界已写入规范。
- 按用户决定保留纯 Rust，encoding_rs 0.8.42 与基础标量适配支持 43 个 canonical name，退役 65 个旧名称；集合与映射差异见 [编码列表](../../sysroot/src/scoop.charset/ENCODINGS.md)。增量、BOM、严格／有损及原始错误偏移合同保留。
- readline 的 Unicode 调用已迁入共同 Rust 后端。GNU libiconv、libcharset、libunistring、libidn2 的源码、维护构建和链接依赖已移除。
- 46 项锁定依赖的源码和许可随附于 vendor；准备脚本还保存实际构建工具链的 Rust 标准库通知，发行时按目标收集。普通应用消费预编译目标归档，不运行 Cargo。

## 验证范围与结果

测试使用 scoop.test，不新增 fixture。按用户要求，已移除底层补缺专测，以及新增的上游 Unicode 兼容性／完备性正负例和官方一致性数据。保留 Scoop 自有适配、状态与资源生命周期及 readline 真实编辑行为；不扩展或重跑 HTTP client 组合测试。

- Darwin、Linux GNU、Linux musl 的 Rust 资源准备、Scoop 库消费及 readline 行为测试已完成。readline 每目标 39 个主用例与 6 个 PTY 子用例通过；musl 的 PTY 辅助代码先清零 termios，再比较实际返回字段，相关 4 项重测通过。
- 已完成的 IDNA 11 项和 URL 14 项三目标消费者结果复用。本轮清理后只复测受影响的 Scoop 适配与 C ABI，不重跑上游规则检查。
- 清理后的名称表、字节／标量索引转换、增量消费与输出队列共 3 项适配检查在三目标通过；C ABI 的长度、状态和所有权 3 项在三目标通过。未复测删除的上游规则用例。
- 三目标的 `scoop link` 与 `scoop-link` 均使用一个共同 Rust 文本归档并运行上述 C ABI 消费者，链接计划没有原 GNU 文本归档。GNU 的动态依赖为既有 libc／libm／加载器，musl 为静态链接；既有 unwind provider 保持唯一。
- 三目标维护准备和后续复用通过；Darwin 同一构建目录的并发准备均直接复用。未删除、移动或隐藏源码、工具或原产物。
- Darwin 使用实际预编译资源完成发行组装，包含 Rust 归档、头文件、46 项依赖通知和 Rust 标准库通知，排除退役 GNU 文本资源与生成的 target 目录；组装后的工具包编译、链接并运行 C ABI 3 项通过。修正了 Darwin 打包向仅依赖系统库的工具追加无用加载路径的问题；未新增专测。
- 发行验证限于本次资源组装和实际消费，没有重建整个 LLVM 工具链、生成全部 API 站点或制作正式发布压缩包。
- Rust fmt／clippy、Python ruff 0.16.10、C 的 Wall／Wextra／Werror、diff whitespace 及相关 API 文档零警告检查均已完成。

原始日志及工作目录位于仓库外 `../scoop-work/m61`，不提交验证日志。
