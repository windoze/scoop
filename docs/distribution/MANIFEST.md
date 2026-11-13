# 发行索引 schema 1

每个 release 发布一份完整 `manifest.toml`。`scoop-install` 使用此文件选择同一版本的组件；不解析文件名推导关系。索引是安装配置，不参与编译器的产物格式或授权判断。

```toml
schema = 1
release = "0.0.0-dev.20261111"
version = "0.0.0-dev.20261111"
revision = "<source revision>"
sources = "sources"
vendor = "vendor"

[hosts.aarch64-apple-darwin]
default-target = "aarch64-apple-darwin"
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
bin = "bin-aarch64-apple-darwin"

[hosts.aarch64-apple-darwin.requirements]
minimum_os = "27.0.1"
sdk = "27.0"

[targets.aarch64-apple-darwin]
base = "lib-base-aarch64-apple-darwin"
gui = "lib-gui-aarch64-apple-darwin"
profiling = "lib-profiling-aarch64-apple-darwin"

[targets.aarch64-apple-darwin.requirements]
minimum_os = "27.0.1"
sdk = "27.0"

[archives.sources]
path = "scoop-0.0.0-dev.20261111-sources.tar.xz"
size = 12345
sha256 = "<64 lowercase hex digits>"
```

示例省略其他宿主、target 和归档记录。完整索引包含每个引用的归档。`native` 仅在 target 需要额外 SDK 包时存在；`gui` 不存在就表示该发行目标不提供 GUI。`requirements` 沿用 `scripts/release.toml` 的平台基线，没有新的版本范围求解规则。

`profiling` 是同 target 的可选完整模板/采集组件，缺省安装不选择它。引用存在时，归档提供 Profiling/full 与 Release/full 两套系统库、训练位置映射、profiling runtime 和目标 compiler-rt profile archive；两套库均包含该目标所有系统 Cone。它依赖同 target 的 base 及原有必要 native 组件；使用 GUI 的 native 资源仍需 gui。不存在引用表示这个 release/target 未提供该组件。追加、移除与升级沿既有组件选择，不把它当作新的 target 或版本。

归档 `path` 可以是相对于本 manifest URL 的路径或绝对 HTTP(S) URL。生产脚本输出同目录文件名；镜像发布时依据明确的上传位置改写 path。`size` 与 `sha256` 从本次新归档生成，安装时在首次下载边界检查一次。已经安装的组件直接复用。

`package_toolchain.py` 输出本宿主的完整 manifest（可用于本地安装测试）、`manifest-<host>.toml` 和 `manifest-shared.toml`。CI 从三个宿主收集 host 片段，仅从 macOS 收集 sources／vendor 及 shared 片段，用 `scripts/package_manifest.py` 合并，避免把其他宿主生成的同名全局归档混入。`packages-<host>.list` 继续只列本次归档。

发布顺序为归档、固定 manifest、最后 latest。固定 release 内容不能被另一套构建覆盖，重打包使用新的版本／release。官网镜像的固定版本入口为 `versions/<version>.toml`，其中归档可引用实际存储目录；latest 使用：

```toml
schema = 1
release = "0.0.0-dev.20261111"
manifest = "versions/0.0.0-dev.20261111.toml"
```

安装状态保存解析后的固定 manifest 及精确版本，追加 target／组件时继续使用该版本，更新才重新解析 latest。离线或私有镜像可以显式传入本地 manifest 或 HTTP(S) manifest URL。
