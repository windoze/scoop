# liburing 2.15

固定上游输入：[liburing-2.15](https://github.com/axboe/liburing/releases/tag/liburing-2.15)。
`2.15/src`、配置脚本和许可取自同名上游 tag，未修改 ring ABI、内存屏障或提交算法。
asyncio 的普通 Cone native 输入仅编译 setup、queue、register、syscall、version 五个 translation unit；使用目标 libc，不启用 nolibc、sanitizer 或 FFI 包装库。

asyncio Cone 的 `native/linux/config/gnu` 和 `native/linux/config/musl` 是在相应目标 C 工具链上执行上游 `configure --use-libc` 的生成头。
维护时在仓库外分别建立构建目录，设置非 `/tmp` 的 `TMPDIR`，GNU 使用 `--cc=gcc`，musl 使用 `--cc=musl-gcc`；musl 的 CFLAGS 仅补充独立 Linux UAPI include 根，不能加入 glibc 的普通头文件目录。
将 config-host.h、src/include/liburing/compat.h 和 io_uring_version.h 保存到对应配置目录。
普通编译不执行 configure，不探测宿主系统 liburing；manifest 根据目标 env 选择生成头。
