<div align="center">

  # Maltoria
  *Visual Novels, Beyond Windows*

  > 本项目Fork自
  > [frankea/Whisky](https://github.com/frankea/Whisky),原本基于
  > [Isaac Marovitz (whisky-app/whisky)](https://github.com/whisky-app/whisky),并增强了作为视觉小说管理器和运行时环境的兼容性和功能.

  [![CI](https://img.shields.io/github/actions/workflow/status/daydalek/Maltoria/CI.yml?style=for-the-badge)](https://github.com/daydalek/Maltoria/actions)
  [![Releases](https://img.shields.io/github/v/release/daydalek/Maltoria?include_prereleases&style=for-the-badge)](https://github.com/daydalek/Maltoria/releases)
  [![Issues](https://img.shields.io/github/issues/daydalek/Maltoria?style=for-the-badge)](https://github.com/daydalek/Maltoria/issues)
  [![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg?style=for-the-badge)](./LICENSE)
</div>

---
## 下载和安装

目前提供面向macOS(aarch64)和Linux(x86_64)用户的安装包,点击下方链接即可直接下载最新版本

当前版本：[**v0.6.0（预发布）**](https://github.com/daydalek/Maltoria/releases/tag/v0.6.0) · [官网与界面预览](https://daydalek.github.io/Maltoria/)

<!-- 每次发布新版本后,请同步更新下表中的版本号与直链. -->
| 平台 | 安装包 |
| --- | --- |
| macOS (Apple Silicon) | [Maltoria-0.6.0-arm64.zip](https://github.com/daydalek/Maltoria/releases/download/v0.6.0/Maltoria-0.6.0-arm64.zip) |
| Linux · deb (Ubuntu 24.04) | [whisky_0.6.0_amd64.deb](https://github.com/daydalek/Maltoria/releases/download/v0.6.0/whisky_0.6.0_amd64.deb) |
| Linux · rpm (Fedora 44) | [whisky-0.6.0-1.x86_64.rpm](https://github.com/daydalek/Maltoria/releases/download/v0.6.0/whisky-0.6.0-1.x86_64.rpm) |
| Linux · AppImage | [Maltoria-0.6.0-x86_64.AppImage](https://github.com/daydalek/Maltoria/releases/download/v0.6.0/Maltoria-0.6.0-x86_64.AppImage) |

由于缺少Apple开发者证书,目前应用以ad-hoc方式签名,打开时会提示无法验证应用安全性,请至设置-隐私与安全性中信任应用.

---

## 游戏刮削与元数据来源（Metadata Data Sources）

游戏刮削的实现参考了 [PotatoVN](https://github.com/GoldenPotato137/PotatoVN)（遵循 [Apache License 2.0](https://github.com/GoldenPotato137/PotatoVN/blob/master/LICENSE)）。

| 数据源 | 接口说明与获取内容 | 状态与使用条款说明 |
| --- | --- | --- |
| **[Bangumi 番组计划](https://bgm.tv/)** | 通过 `api.bgm.tv/v0/search/subjects`（游戏分类 `type=4`）检索作品中文名/原名、简介、发行日期及多尺寸封面图 | 默认启用|
| **[VNDB](https://vndb.org/)** | 通过 VNDB Kana API（`api.vndb.org/kana/vn`）支持关键词或 `v<id>` 直接检索多语言标题、罗马音、别名、开发商、发售日、简介与封面 | 默认启用|
| **[Steam](https://store.steampowered.com/)** | 通过 Steam Store Search 与 `appdetails` 接口检索简中商店名称、简介、开发商、发行日及 `library_600x900` 纵向封面 | 默认启用 |
| **[月幕 Galgame (YMGal)](https://www.ymgal.games/)** | 通过[月幕官方开放 API](https://www.ymgal.games/developer)（公开 OAuth 客户端凭证）提供游戏搜索、中文名/原名、发售日、封面与档案简介；开发商名称取自搜索结果提供的 `orgName`（若有） | 默认启用 |
| **[NextMoe 鲲 Galgame 论坛](https://developer.nextmoe.dev/)** | 通过 NextMoe 公开目录 API（`api.nextmoe.dev/v2/catalog`，匿名只读）检索多语言标题、封面、简介、开发商、发售日、人物与声优 | 默认启用|

---

### 存储路径与彻底卸载

- **macOS 本地存储路径**：共享运行库安装在 `~/Library/Application Support/com.daydalek.Maltoria/Libraries/`；默认容器、文件夹游戏库配置（`Folder Games.json`、`Library Visibility.json`）及刮削缓存（`Game Metadata/`）位于 `~/Library/Containers/com.daydalek.Maltoria/`（或您自定义的 Bottle 路径）下。
- **Linux 本地存储路径**：程序文件安装在 `/opt/whisky/`（启动器为 `/usr/bin/whisky`）；Wine 运行库位于 `$XDG_DATA_HOME/whisky/runtime/wine/`（默认 `~/.local/share/whisky/runtime/wine/`），默认 Bottle、游戏库配置与刮削缓存位于 `~/.local/share/whisky/Bottles/`。
- **macOS 彻底卸载**：将 `Maltoria.app` 移至废纸篓默认保留运行库与 Bottle 数据。如需彻底清除（**请先备份重要存档与容器**），可删除以下目录：

```sh
rm -rf "/Applications/Maltoria.app"
rm -rf ~/Library/Containers/com.daydalek.Maltoria
rm -rf ~/Library/Containers/com.daydalek.Maltoria.WhiskyThumbnail
rm -rf ~/Library/Application\ Support/com.daydalek.Maltoria
rm -rf ~/Library/Caches/com.daydalek.Maltoria
rm -rf ~/Library/Logs/com.daydalek.Maltoria
rm -rf ~/Library/HTTPStorages/com.daydalek.Maltoria
rm -f  ~/Library/Preferences/com.daydalek.Maltoria.plist
```

- **Linux 彻底卸载**：deb 用 `sudo apt remove whisky`、rpm 用 `sudo dnf remove whisky` 卸载（卸载保留用户数据）；AppImage 直接删除文件。如需彻底清除用户数据，可删除：

```sh
rm -rf ~/.local/share/whisky
```

## 相关文档 (Documentation)

- **[Game Metadata Integration](docs/game-metadata.md)** - 文件夹游戏发现、元数据刮削数据源、缓存机制与多语言匹配规则
- **[Steam Launch Readiness](docs/steam-launch-readiness.md)** - Steam 会话就绪检测、进程确认与保守重试机制
- **[Launcher Troubleshooting](docs/LauncherTroubleshooting.md)** / **[Steam Compatibility Guide](docs/SteamCompatibility.md)** - Steam 等启动器兼容说明与排障指南
- **[Stability Troubleshooting](docs/StabilityTroubleshooting.md)** - 崩溃、卡死与稳定性诊断说明
- **[Runtime Dependencies](docs/DEPENDENCIES.md)** - 运行库组件版本与上游来源说明

---

## 版权、许可证与源代码来源声明 (License & Source Attribution)

本项目遵循 **[GNU General Public License v3.0 (GPL-3.0-or-later)](./LICENSE)** 开源许可证发布。完整第三方声明与版本固定记录见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)，许可证原文与构建归档说明见 [ThirdParty/](ThirdParty/)。

- **应用源码归属 (Application Source)**：本仓库（[daydalek/Maltoria](https://github.com/daydalek/Maltoria)）fork 自 **[frankea/Whisky](https://github.com/frankea/Whisky)**，最初由 **[Isaac Marovitz](https://github.com/IsaacMarovitz)** 创建于 **[whisky-app/whisky](https://github.com/whisky-app/whisky)**。
- **Wine 运行库归属 (Wine Runtime Source)**：配套 Wine 运行库由 **[daydalek/ge-proton-macos-wine-poc](https://github.com/daydalek/ge-proton-macos-wine-poc)** 构建并发布，基于 **CodeWeavers CrossOver Wine 26.3 开源核心** 与 **GE-Proton** 多媒体补丁移植。

### Credits & Acknowledgments（第三方组件与致谢）

Maltoria 得以实现，离不开以下优秀开源项目、协议标准与社区维护者的贡献：

#### 上游项目与桌面框架 (Upstream & UI Framework)

- **[Whisky](https://github.com/whisky-app/whisky)**（[Isaac Marovitz](https://github.com/IsaacMarovitz)）& **[frankea/Whisky](https://github.com/frankea/Whisky)**（`GPL-3.0-or-later`）— 酒瓶管理、配置模型与原生桥接基础（Bottle management, configuration model, and native bridge foundation）。
- **[gpui-kit](https://crates.io/crates/gpui-kit)** / **[Zed GPUI](https://github.com/zed-industries/zed)**（`Apache-2.0`）— 驱动 `whisky-gpui` 跨平台 GPU 加速原生界面的桌面 UI 框架（Cross-platform GPU-accelerated desktop UI framework powering `apps/whisky-gpui`）。

#### 实时文本捕获与翻译生态 (Text Hooking & Translation Pipeline)

- **[LunaTranslator / LunaHook](https://github.com/HIllya51/LunaTranslator)** by HIllya51（`GPL-3.0`）— Windows 游戏进程文本钩取、内嵌宿主与字体临时加载核心组件，包括 `LunaHook32/64.dll`、`LunaHost32/64.dll`、`LunaTmpFontLoader.dll`（Windows game text-hooking, injection host, and temporary font loader core）。
- **[MinHook](https://github.com/TsudaKageyu/MinHook)** by Tsuda Kageyu（`BSD-2-Clause`）— 轻量级 x86/x64 API Inline Hook 库，含 Vyacheslav Patkov 的 HDE32 / HDE64 反汇编器（Minimalistic x86/x64 API inline hooking library with HDE32/HDE64 disassemblers）。
- **[uchardet](https://gitlab.freedesktop.org/uchardet/uchardet)**（`GPL-2.0-or-later` 分发选项）— 多语言文本编码自动检测，支持 Shift-JIS / UTF-8 / GBK 等日文及多语言游戏编码识别（Multi-encoding charset detection）。
- **[nlohmann/json](https://github.com/nlohmann/json)** by Niels Lohmann（`MIT`）— `MaltoriaLunaHost` 辅助进程的 JSON IPC 通信库（JSON IPC serialization for `MaltoriaLunaHost32/64.exe`）。

#### Wine 运行库、图形转换与辅助工具 (Wine Runtime, Graphics & Utilities)

- **[CodeWeavers CrossOver](https://www.codeweavers.com/crossover)** & **[WineHQ](https://www.winehq.org/)**（`LGPL-2.1-or-later`）、**[GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom)** by GloriousEggroll — 经由 **[daydalek/ge-proton-macos-wine-poc](https://github.com/daydalek/ge-proton-macos-wine-poc)** 移植构建的 Wine 运行库核心与多媒体兼容补丁（Upstream Wine core and GE-Proton patches）。
- **[DXVK-macOS](https://github.com/Gcenx/DXVK-macOS)**（Gcenx, doitsujin）、**[DXMT](https://github.com/3Shain/dxmt)**（3Shain）、**[MoltenVK](https://github.com/KhronosGroup/MoltenVK)**（KhronosGroup）、**[msync](https://github.com/marzent/wine-msync)**（marzent）& **Apple D3DMetal** — Direct3D 到 Vulkan/Metal 的图形转换层、着色器翻译与内核同步机制（Direct3D translation layers, Vulkan-over-Metal, and synchronization primitives）。
- **[Winetricks](https://github.com/Winetricks/winetricks)**（Dan Kegel, Austin English et al., `LGPL-2.1+`）& **[cabextract / libmspack](https://www.cabextract.org.uk/)**（Stuart Caie, `GPL-2.0+`）— Windows 运行库组件安装脚本与 Microsoft Cabinet 压缩包解压工具（Runtime dependency installer and CAB archive extractor）。

#### 画质增强与视觉小说资料库生态 (Upscaling, Metadata & Toolchain)

- **[Real-CUGAN](https://github.com/bilibili/ailab)** & **[realcugan-ncnn-vulkan](https://github.com/nihui/realcugan-ncnn-vulkan)**（`MIT`）— 二次元封面超分辨率模型；macOS 使用 Core ML 分块推理，Linux 使用 `ncnn-vulkan`（Anime cover super-resolution via tiled Core ML on macOS and `ncnn-vulkan` on Linux）。
- **[Anime4K](https://github.com/bloc97/Anime4K)** by bloc97（`MIT`）— 实时 Metal 着色器超分方案 `CNN x2 S`（Real-time Metal shader cover upscaler）。
- **[PotatoVN](https://github.com/GoldenPotato137/PotatoVN)** by GoldenPotato137（`Apache-2.0`）— 多数据源刮削架构设计参考（Multi-source metadata scraper architecture reference）。
- **公开数据源 (Public Metadata & Artwork Sources)**：[Bangumi 番组计划](https://bgm.tv/)（[Open API](https://bangumi.github.io/api/)）、[VNDB](https://vndb.org/)（[Kana API](https://api.vndb.org/kana)）、[Steam](https://store.steampowered.com/)（Store Web API）、[月幕 Galgame (YMGal)](https://www.ymgal.games/)（[开放 API](https://www.ymgal.games/developer)）、[NextMoe](https://nextmoe.com/)（[API v2](https://developer.nextmoe.dev/docs/v2)）— 游戏标题、简介、开发商与封面数据版权归原平台及权利人所有。
- **Swift 工具链引脚（`WhiskyKit` / `WhiskyCmd`）**：[SemanticVersion](https://github.com/SwiftPackageIndex/SemanticVersion)（`Apache-2.0`）、[Sparkle](https://github.com/sparkle-project/Sparkle)（`MIT`）、[swift-argument-parser](https://github.com/apple/swift-argument-parser)（`Apache-2.0`）、[PostHog](https://github.com/PostHog/posthog-ios)（`MIT`，本分支已禁用遥测 / telemetry disabled in this fork）。

#### 特别鸣谢 (Special Thanks)

特别感谢 **HIllya51**、**GoldenPotato137**、**Gcenx**、**ohaiibuzzle**、**Nat Brown**、**[Frank](https://github.com/frankea)**、**[Isaac Marovitz](https://github.com/IsaacMarovitz)** 以及所有上游项目维护者与开源社区贡献者的长期支持与无私贡献！

For full licensing terms, pinned revisions, and third-party notices, see [LICENSE](./LICENSE), [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), and [ThirdParty/](ThirdParty/).

---

<table>
  <tr>
    <td>
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="./images/cw-dark.png">
          <img src="./images/cw-light.png" width="500">
        </picture>
    </td>
    <td>
        Maltoria doesn't exist without CrossOver. If you want a fully-supported commercial Wine experience on macOS, check out <a href="https://www.codeweavers.com/crossover">CrossOver</a> from CodeWeavers. (This fork has no affiliate arrangement and receives nothing from CrossOver sales.)
    </td>
  </tr>
</table>
