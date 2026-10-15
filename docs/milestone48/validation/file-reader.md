# Compile 依赖文件按需读取

显式依赖加载器保留文件句柄，通过一次解析的 manifest 和成员目录定位数据，仅在请求成员时读取并缓存缓冲区。文件和内存输入共用目录偏移计算与成员头解析；同一文件摘要和成员重复访问直接复用。Compile 只读取 HIR／MIR／LIR metadata，机器对象留在文件中。移除整包 `Vec`、读取前后 metadata 比较、regular-file 审计和相应错误分支；实际 open／seek／read 错误继续报告。

文件入口为 124 行、测试 69 行，目录偏移模块为 61 行。旧目录入口不再逐成员检查未使用内容或比较整包预期长度。末尾截断必须在初始 open 阶段失败的旧测试退役；新测试在完整真实归档上核对成员内容、指针复用、文件读取位置和未读取的对象，并覆盖目录输入的实际 I/O 错误。没有新增严格 reader 或缓存恢复路径。

## 本机验证

- 格式化与 workspace clippy通过。
- 569 项 slib、7 项 preflight 输入和 1 项缺失依赖通过。首轮 slib 仅旧的整包截断复核测试失败，按 M48 合同退役后通过；保留首轮结果。
- release 构建通过。7 个 CLI 组合覆盖源码／prebuilt core、泛型 siblings、native、GC leaf、M46 序列化与输入错误，共 9 variants、76 processes；13 个 sysroot 负例共 13 variants、15 processes。
- 确认没有本机 cargo／rustc／clippy 进程后，清理 incremental 2,842.3 MiB。

## Linux 验证与真实读取

使用独立 `target/m48-summary-reader`，完整命令见验证脚本。fmt、clippy、570 项 slib、7 项 preflight、1 项缺失输入及 release 构建通过。GNU 和 musl各通过 20 个 CLI 用例、22 variants、91 processes，与本机两组选择一致。

保留源码、工具和全部产物，对独立 `scoopc build` 做 GNU 系统调用采样：41,595,590 字节的 core 归档只打开一次，读取 29,476,197 字节；读取范围恰好是 manifest 和三个 metadata 成员，4,208 个 ELF 对象成员全部未读。区间没有重叠或重复读取；采样结果与命令、脚本和编译 stderr保留。该结果证明 Compile 文件按需读取，不代表 metadata 本身已缩小或语义重放已经删除。

## 范围与后续

本批完成 Compile 的文件加载入口，ProgramLink／parent／dump 的整包读取和输入快照尚待迁移。Compile 的语义／布局重放、ProgramLink 的对象／注册数据重放及证明结构、dump 最小阶段复用、历史测试迁移、最终全量与五轮性能复测仍属于 M48。M38 未启动。
