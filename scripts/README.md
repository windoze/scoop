# 开发 sysroot 与发行准备

这些脚本供维护仓库和制作发行包时手动运行，不属于 `scoop` 命令，也不由 driver、常规测试或 CI 自动调用。Python 脚本要求 Python 3.11 或更新版本；OpenSSL 解包使用 Python 3.12 或更新版本。

先构建配套的 `scoop`、`scoopc`、`scoop-link`，并用 `build_openssl.py` 准备所需目标的 OpenSSL。Linux 还需要按 `build_llvm_unwind.py --help` 准备同目标的 unwind。然后准备开发 sysroot，例如：

```sh
python3 scripts/prepare_sysroot.py \
  --target aarch64-apple-darwin \
  --scoop "$CARGO_TARGET_DIR/release/scoop" \
  --work-dir "$PWD/tmp/sysroot/aarch64-apple-darwin"
```

`--sysroot` 默认为仓库的 `sysroot`，`--runtime-root` 默认为仓库的 `runtime`；可显式选择 `--cache-dir`、`--scoopc` 和现有 native 工具参数。维护脚本默认将工作目录放在仓库已忽略的 `tmp` 下，避免占用 HOME 所在磁盘；也可显式指定其他磁盘。构建缓存、中间产物和辅助程序的 Rust target 目录保存在工作目录或显式 `CARGO_TARGET_DIR`，不要选择绝对路径 `/tmp`。目标输出为 `sysroot/lib/<triple>`，公开头文件为 `sysroot/include`。

脚本读取 `src/*/Cone.toml` 的生产依赖确定顺序，逐个执行普通 release library 构建；测试源码不进入发布库。`runtime_builder` 是独立于 Cargo 主 workspace 的脚本内部辅助程序，复用已有的 O2 runtime 编译与索引实现，不安装到发行包的 `bin`。准备过程不重新读取对象以核对摘要或复验产物；重复运行使用已有构建记录，输入发生变化时更新对应结果。runtime 与公开头文件先复制完整输出，再替换准备目录；复制失败保留原目录。

开发 sysroot 准备完成后，再运行相关的正式 CLI fixture 和库测试。准备 sysroot 不要求构建发行 LLVM、压缩归档或运行全套测试。
