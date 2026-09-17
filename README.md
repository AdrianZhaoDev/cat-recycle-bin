# Cat Recycle Bin · 猫耳回收站

[English](#english) · [简体中文](#简体中文)

A tiny animated Windows Recycle Bin, extracted from the cat bin in Desktop Creatures. / 从 Desktop Creatures 独立出来的猫耳桌面回收站。

![Animated file drop preview](docs/images/drag-to-recycle.gif)

The GIF is an illustrated UI preview. The still image below is captured from the actual Windows app. / GIF 为交互演示动画；下方静态图截取自实际运行的 Windows 程序。

| 3D model preview / 模型预览 | Running Windows app at 200% / 200% 尺寸实际运行 |
| --- | --- |
| ![Cat bin model preview](docs/images/model-preview.png) | ![Actual running Windows bin](docs/images/runtime-200pct.jpg) |

## English

### Features

- Drop files, folders, or multiple selected items from File Explorer into the bin to request a move to the **Windows Recycle Bin**.
- Hover to open the lid. The original 3D model, sounds, movable desktop position, and happy expression after a successful drop remain intact.
- Double-click to open the system Recycle Bin. Right-click the bin or use its tray icon for settings and exit.
- **Confirm before recycling** is on by default and can be turned off. The preference is saved.
- Choose 100%, 150%, 200%, or 300% display size. The default is 200%; 100% restores the original 76 × 88 window. The renderer also uses extra pixel sampling for cleaner edges.

### Download and use

Download the Windows ZIP from [Releases](https://github.com/AdrianZhaoDev/cat-recycle-bin/releases/latest), extract it, and run `cat-recycle-bin.exe`. WebView2 is required. Move the bin by dragging it with the left mouse button.

The app accepts real file-system paths on fixed local disks. It refuses network and removable volumes, drive roots, paths inside the Recycle Bin, its own executable and configuration data, and overlapping parent/child paths in one drop. If Windows cannot recycle an item, it may show a separate **permanent deletion** warning; cancel that system warning if you need the item to remain recoverable.

### Build from source

On Windows, install Node.js 24, the Rust MSVC toolchain, and WebView2:

```powershell
npm ci
npm run check
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run desktop:build -- --no-bundle
```

The executable is written to `src-tauri/target/release/cat-recycle-bin.exe`. The Tauri app has its own identifier and settings directory; the original game is not needed.

## 简体中文

### 功能

- 从资源管理器拖入文件、文件夹或多个选中项目，程序请求将它们移入 **Windows 系统回收站**。
- 悬浮开盖，保留原有的 3D 模型、音效、桌面位置拖动，以及回收成功后的开心表情。
- 双击打开系统回收站；右键垃圾桶或点击托盘图标可打开设置和退出。
- **删除前提醒** 默认开启，可关闭，设置会保存。
- 可选 100%、150%、200%、300% 尺寸。默认 200%；100% 是原来的 76 × 88 窗口。渲染器同时提高内部采样，改善边缘清晰度。

### 下载与使用

从 [Releases](https://github.com/AdrianZhaoDev/cat-recycle-bin/releases/latest) 下载 Windows ZIP，解压后运行 `cat-recycle-bin.exe`。需要 WebView2。按住左键拖动垃圾桶即可调整位置。

程序接收固定本地磁盘上具有实际路径的文件和文件夹。网络位置、可移动卷、磁盘根目录、回收站内部、程序及配置数据，以及同批次重复或父子重叠的路径会被拒绝。如果 Windows 无法将项目放入回收站，系统可能另弹 **永久删除** 警告；想保留可恢复性时，请取消该系统警告。

### 从源码构建

在 Windows 上准备 Node.js 24、Rust MSVC 工具链和 WebView2：

```powershell
npm ci
npm run check
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run desktop:build -- --no-bundle
```

EXE 位于 `src-tauri/target/release/cat-recycle-bin.exe`。本项目使用独立的应用标识和设置目录，不依赖原游戏运行。

## License / 许可

The application code is released under [MIT](LICENSE). The original model, icon, and sounds are covered by [ASSET-LICENSE.txt](ASSET-LICENSE.txt); editable Blender source and generation records are included. Third-party components retain their own licenses; see [third-party notices](THIRD-PARTY-NOTICES.txt).

程序代码使用 [MIT 许可](LICENSE)。原创模型、图标和音效见 [ASSET-LICENSE.txt](ASSET-LICENSE.txt)；仓库保留可编辑的 Blender 源文件及生成记录。第三方组件见[许可说明](THIRD-PARTY-NOTICES.txt)。
