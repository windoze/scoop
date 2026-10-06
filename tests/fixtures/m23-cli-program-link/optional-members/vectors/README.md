# Optional member 测试数据

这些片段由现有 `BootstrapManifest::new`、`SlibMember::new` 和
`CanonicalSlibArchive::write_bootstrap` 生成，保存 manifest、ar header 与 padding。
各真实 member 的 payload 不在此复制；用例以系统 `tar` 读取本次编译产物，
将对应 payload 插入片段之间。原产物和重组产物均检查完整 SHA-256。

输入为 `dev.programlink:root:0.1.0`、`read-println.scoop` 的当前完整产物，
原 SHA-256 为 `3cf90419ffd8dcd9358a957cca872601a0f5f2e2c7f383ebb3a2845b98344381`。
保留 compatibility、Cone、direct dependencies、原 members、semantic fingerprints
和 sections，producer 改为 `m23-program-link-repack`。

`optional` 在此基础上增加以下数据：

- `dev.programlink/opaque-object/1`、logical key `object` 的 Optional ExtensionBlob；
- 相同 capability、logical key `diagnostic` 的 DiagnosticAttachment；
- `dev.programlink/compile-only/1`、purpose Compile、payload `ff` 的 manifest section。

前两个 member 均使用原第一个 LinkObject 的真实 Mach-O payload。
`required` 再增加 `dev.programlink/unknown-link-object/1`、logical key `object`、
requirement Link 的 ExtensionBlob，也使用该 payload。

物理顺序由正式 writer 按 member ID 决定，TOML 中列明每个 header 后接哪个原始
payload。若 compiler 或格式变化使原 SHA 不同，应通过上述 API 重建片段并审阅
成员、完整 plan 和运行结果；不能仅修改 checksum 来跳过输入变化。

M24 迁移对照本次正式构建的 manifest 与 member payload，同步依赖、语义摘要和
manifest section，保留以上 optional/required 成员及其原有 requirement。重组产物
继续由正式 linker 验证，optional 的链接计划和运行结果必须与原产物一致。

M26 迁移使用当前正式产物和同一组 writer API 重建 manifest、header 与 padding；
保留原有附加成员及其用途，并逐项核对片段拼接后的字节与 writer 输出完全一致。

M27 迁移保留同样的 optional/required 能力声明，使用当前 runtime ABI 的实际产物
重新生成片段。物理 member 顺序变化后，TOML 同步引用当前第一个 LinkObject 的 payload，
并核对所有片段拼接与正式 writer 产物逐字节一致。

M29 迁移保留原有 optional/required 成员、用途与 payload 顺序，重建当前 core
依赖对应的 manifest 和归档 header。最后一个奇数字节数的 metadata member
包含正常 archive padding；TOML 拼接也保留该字节，完整结果与正式 writer 一致。
