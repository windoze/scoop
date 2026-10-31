# Skia 图形与段落依赖

固定 Skia revision 为 `f326a1e25fc736704063922b9ce76b527b1c5efe`。2026-10-31 检查其主线与模块接口，使用这一版 DEPS 指定的依赖组合，避免独立升级 shaping 与段落引擎。

| 输入 | 上游固定来源 |
| --- | --- |
| Skia | [google/skia](https://github.com/google/skia/tree/f326a1e25fc736704063922b9ce76b527b1c5efe) |
| HarfBuzz | [harfbuzz/harfbuzz](https://github.com/harfbuzz/harfbuzz/tree/9cb1fee51069b206effb4736e443b038d230789d) |
| ICU | [Chromium ICU](https://chromium.googlesource.com/chromium/deps/icu/+/d578f2e8b7bd5938e21cfb6bf15c079e0aa5b738) |
| libpng | [pnggroup/libpng](https://github.com/pnggroup/libpng/tree/d5515b5b8be3901aac04e5bd8bd5c89f287bcd33) |
| libjpeg-turbo | [Chromium libjpeg-turbo](https://chromium.googlesource.com/chromium/deps/libjpeg_turbo/+/e14cbfaa85529d47f9f55b0f104a579c1061f9ad) |
| D3D12MemoryAllocator（Windows） | [GPUOpen-LibrariesAndSDKs/D3D12MemoryAllocator](https://github.com/GPUOpen-LibrariesAndSDKs/D3D12MemoryAllocator/tree/169895d529dfce00390a20e69c2f516066fe7a3b) |
| SPIRV-Cross（Windows） | [KhronosGroup/SPIRV-Cross](https://github.com/KhronosGroup/SPIRV-Cross/tree/b8fcf307f1f347089e3c46eb4451d27f32ebc8d3) |
| zlib（Windows） | [Chromium zlib](https://chromium.googlesource.com/chromium/src/third_party/zlib/+/646b7f569718921d7d4b5b8e22572ff6c76f2596) |

归档保留上游 tar 内容，仅将 gzip 压缩改为 xz；源码、许可及版权声明均保留。常用许可另置于本目录，随既有发行许可收集流程发布。Darwin 使用所选 macOS SDK 的系统 zlib，不额外编入另一份 zlib。

Windows 不依赖外置 `icudtl.dat`；将同一 ICU common 数据通过上游的 C++ 数据生成器编入静态库。`patches/windows-toolchain.patch` 区分 Windows OS 与 MSVC driver 的参数约定，使 Microsoft 和 LLVM-MinGW 分别使用自己的工具链／CRT，并选择内嵌 ICU 数据。补丁同时适配 mingw-w64 为 DirectWrite 重载使用的 `CreateFontFaceReference_`／`GetGlyphImageFormats_` 名称，调用对应的原 COM 接口；MSVC 专用 warning pragma 仅在 `_MSC_VER` 下启用。D3D12MemoryAllocator、SPIRV-Cross 及 zlib 按上述 DEPS 固定版本一并静态合并，不从应用程序当前目录查找数据或下载依赖。

`scripts/build_skia.py` 显式准备 Ganesh Metal、CPU raster、SkParagraph、HarfBuzz、SkUnicode ICU 与 PNG／JPEG 解码。关闭 Graphite／Dawn／Vulkan／GL、PDF／SVG、工具和其他可选图像编码器。需要 Python、GN、Ninja、Xcode 和 Apple Clang；GN 可经 `--gn` 指定已有可执行文件，普通应用不构建工具、不运行依赖同步程序、不联网。

Windows 同一入口将 Metal 换为 Ganesh Direct3D 12，字体使用 DirectWrite；其他启用模块相同。三个完整 Windows target 使用各自的 `--cc`（Microsoft 为 clang-cl，MinGW 为相应 CRT prefix 的 clang），C++ driver 从同一目录选择或通过 `--cxx` 指定。Microsoft 构建在匹配发行配置的 x64 MSVC／Windows SDK 开发环境运行。各 target 使用独立 build-dir，输出仍为目标目录中的 `libscoop_skia.a`，格式为包含对应 COFF 对象的普通 archive。

Darwin ARM64 与 x86_64 分别在独立工作目录构建。部署目标和 CPU 来自 `scripts/release.toml`；SDK 默认使用当前 Xcode 的 macosx，也可用 `--sdk` 显式选择。`patches/macos-deployment.patch` 将 Skia 硬编码的 macOS 11 target 改为 GN 参数，让 C++／Objective-C++ 与 C／assembly 使用同一部署版本。没有屏蔽 SDK 可用性错误。

发布一个 `libscoop_skia.a`，合并实际构建的 Skia／段落／shaping／Unicode／codec 静态依赖；头文件位于目标资源的 `include/skia`。这些都是普通 Cone native 输入，不进入编译器或 runtime。缓存命中只判断输入更新与输出存在，发生更新后才解包、探测工具和构建。

graphics 绑定使用 `SK_RELEASE`，与预编译的 release Skia 保持相同头文件布局；应用的 Scoop profile 不改变该依赖的 C++ ABI。原生 C++ 标准使用 Cone driver 的 C++20 默认值，公共 Clang warning 选项使用 `-Wmost`，避免 clang-cl 将 `-Wall` 解释为 Microsoft 的全部 warning 等级。
