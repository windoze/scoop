# Skia 图形与段落依赖

固定 Skia revision 为 `f326a1e25fc736704063922b9ce76b527b1c5efe`。2026-10-31 检查其主线与模块接口，使用这一版 DEPS 指定的依赖组合，避免独立升级 shaping 与段落引擎。

| 输入 | 上游固定来源 |
| --- | --- |
| Skia | [google/skia](https://github.com/google/skia/tree/f326a1e25fc736704063922b9ce76b527b1c5efe) |
| HarfBuzz | [harfbuzz/harfbuzz](https://github.com/harfbuzz/harfbuzz/tree/9cb1fee51069b206effb4736e443b038d230789d) |
| ICU | [Chromium ICU](https://chromium.googlesource.com/chromium/deps/icu/+/d578f2e8b7bd5938e21cfb6bf15c079e0aa5b738) |
| libpng | [pnggroup/libpng](https://github.com/pnggroup/libpng/tree/d5515b5b8be3901aac04e5bd8bd5c89f287bcd33) |
| libjpeg-turbo | [Chromium libjpeg-turbo](https://chromium.googlesource.com/chromium/deps/libjpeg_turbo/+/e14cbfaa85529d47f9f55b0f104a579c1061f9ad) |

归档保留上游 tar 内容，仅将 gzip 压缩改为 xz；源码、许可及版权声明均保留。常用许可另置于本目录，随既有发行许可收集流程发布。zlib 使用所选 macOS SDK 的系统库，不额外编入另一份 zlib。

`scripts/build_skia.py` 显式准备 Ganesh Metal、CPU raster、SkParagraph、HarfBuzz、SkUnicode ICU 与 PNG／JPEG 解码。关闭 Graphite／Dawn／Vulkan／GL、PDF／SVG、工具和其他可选图像编码器。需要 Python、GN、Ninja、Xcode 和 Apple Clang；GN 可经 `--gn` 指定已有可执行文件，普通应用不构建工具、不运行依赖同步程序、不联网。

Darwin ARM64 与 x86_64 分别在独立工作目录构建。部署目标和 CPU 来自 `scripts/release.toml`；SDK 默认使用当前 Xcode 的 macosx，也可用 `--sdk` 显式选择。`patches/macos-deployment.patch` 将 Skia 硬编码的 macOS 11 target 改为 GN 参数，让 C++／Objective-C++ 与 C／assembly 使用同一部署版本。没有屏蔽 SDK 可用性错误。

发布一个 `libscoop_skia.a`，合并实际构建的 Skia／段落／shaping／Unicode／codec 静态依赖；头文件位于目标资源的 `include/skia`。这些都是普通 Cone native 输入，不进入编译器或 runtime。缓存命中只判断输入更新与输出存在，发生更新后才解包、探测工具和构建。
