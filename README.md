# Ramble Markdown

一个用 **Rust** 编写的轻量级 Markdown 编辑与实时预览桌面应用。单文件可执行、无需运行时依赖，双击即用。

![Rust](https://img.shields.io/badge/Rust-2024_edition-orange)
![egui](https://img.shields.io/badge/UI-eframe%2Fegui-blue)

---

## ✨ 功能特性

### 编辑与预览
- **实时预览**：左侧编辑，右侧即时渲染，改一个字预览立刻更新
- **三档视图模式**：分栏（编辑+预览）/ 仅编辑 / 仅预览，一键切换
- **滚动随动**：拖动任意一侧的滚动条，另一侧按内容比例跟随滚动（双向同步）
- **可拖动分栏**：左右宽度自由拖拽调整
- **mermaid 图表**：```mermaid 代码块自动渲染为图片，支持流程图、时序图、状态图、类图、ER 图、饼图、甘特图、时间线、思维导图、gitGraph 等
- **代码块语法高亮**：基于 syntect，覆盖 Rust / Python / JS / SQL / JSON / Shell 等主流语言
- **Markdown 支持**：标题、加粗/斜体/删除线、有序/无序/任务列表、引用、表格、分隔线、行内代码、代码块、链接、图片

### 查找与替换
- `Ctrl + F` 查找、`Ctrl + H` 查找替换，`F3` / `Shift + F3` 上下跳转
- 支持大小写敏感开关、匹配计数（`3 / 12`）、替换当前、全部替换
- 中英文混合文本安全匹配（按字符而非字节匹配，不会切坏多字节字符）

### 文件操作
- 新建 / 打开 / 保存 / 另存为，支持拖拽 `.md` 文件进窗口打开
- **最近打开列表**：工具栏「最近」菜单，保留最近 10 个文件（已删除的文件会提示并自动清理）
- 支持命令行传参：`ramble-markdown.exe 文档.md`
- **未保存保护**：关闭窗口或切换文件前弹窗确认（保存并继续 / 不保存 / 取消）
- 标题栏显示文件名与 `*` 脏标记，状态栏显示保存状态与字数统计（词数 / 字符 / 行数）

### 界面
- **VSCode 风格配色**：面板 `#181818`、编辑区 `#1e1e1e`、状态栏 `#007acc` 蓝条
- **暗色 / 浅色一键切换**（默认跟随系统主题）
- 启动自动加载系统中文字体（微软雅黑 / 黑体 / 宋体），中文与 emoji 正常显示

### Windows 集成
- **右键菜单**：在资源管理器中对 `.md` / `.markdown` 文件右键 → 「用 Ramble Markdown 打开」

---

## 🚀 快速开始

### 直接使用
双击 `ramble-markdown.exe`，或把 `.md` 文件拖进窗口，或右键选择「用 Ramble Markdown 打开」。

### 从源码构建
```bash
# 需要 Rust 1.85+（本机实测 1.93.1）
cd ramble-markdown
cargo build --release
# 产物：target/release/ramble-markdown.exe
```

---

## ⌨️ 快捷键

| 功能 | 快捷键 |
|------|--------|
| 新建 | `Ctrl + N` |
| 打开 | `Ctrl + O` |
| 保存 | `Ctrl + S` |
| 查找 | `Ctrl + F` |
| 查找 + 替换 | `Ctrl + H` |
| 下一个 / 上一个匹配 | `F3` / `Shift + F3` |
| 替换栏内跳转 | `Enter` / `Shift + Enter` |
| 关闭查找栏 | `Esc` |
| 放大 / 缩小界面 | `Ctrl + =` / `Ctrl + -` |
| 重置界面缩放 | `Ctrl + 0` |
| 拖拽打开 | 把文件拖入窗口 |

---

## 🔧 右键菜单管理

注册表位置（当前用户，**无需管理员权限**）：

```
HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.md\shell\RambleMarkdown
```

**安装 / 更新**（PowerShell，注意把 exe 路径换成实际位置）：
```powershell
$exe = 'E:\ramble\ramble-md\ramble-markdown\ramble-markdown.exe'
foreach ($ext in '.md', '.markdown') {
    $key = "HKCU:\Software\Classes\SystemFileAssociations\$ext\shell\RambleMarkdown"
    New-Item -Path "$key\command" -Force | Out-Null
    Set-ItemProperty -Path $key -Name '(Default)' -Value '用 Ramble Markdown 打开'
    Set-ItemProperty -Path $key -Name 'Icon' -Value $exe
    Set-ItemProperty -Path "$key\command" -Name '(Default)' -Value ('"{0}" "%1"' -f $exe)
}
```

**卸载**：
```powershell
Remove-Item -Path 'HKCU:\Software\Classes\SystemFileAssociations\.md\shell\RambleMarkdown' -Recurse -Force
Remove-Item -Path 'HKCU:\Software\Classes\SystemFileAssociations\.markdown\shell\RambleMarkdown' -Recurse -Force
```

> 注意：右键菜单指向的是注册时填写的 exe 路径。如果移动了程序位置，需要重新执行安装命令。

---

## 🏗 技术栈

| 用途 | 依赖 |
|------|------|
| 窗口 / GUI | `eframe` 0.35（`egui` 0.35） |
| Markdown 渲染 | `egui_commonmark` 0.24（基于 `pulldown-cmark`，`better_syntax_highlighting` 语法高亮 + `embedded_image` 内嵌图片） |
| 代码高亮 | `syntect` 5（纯 Rust 正则后端 `default-fancy`，无需 C 编译器） |
| mermaid 渲染 | `mermaid-rs-renderer` 0.3.1（纯 Rust，无浏览器/Node 依赖）+ `resvg` 栅格化 |
| 原生文件对话框 | `rfd` 0.17 |
| 图片解码 | `image` 0.25（png / jpeg） |
| base64 编码 | `base64` 0.23 |

### mermaid 渲染管线

```
```mermaid 代码块
      ↓  mermaid_rs_renderer::render()        （解析 + 布局，纯 Rust，约 3ms）
   SVG 字符串
      ↓  write_output_png() → resvg          （栅格化为 PNG）
   PNG 字节
      ↓  base64 编码
 data:image/png;base64,...  ← egui_commonmark 的 embedded_image 加载器直接显示
```

要点：
- 按**源码原文**缓存渲染结果，编辑正文时未改动的图表不会重渲染
- 渲染失败时保留原始代码块，并在下方追加一行 `⚠️ 图表渲染失败：...`，便于定位语法错误
- 未闭合的 ```mermaid（用户正在输入）原样输出，绝不会篡改正文

架构说明：源码按职责拆分为 6 个模块（`src/`）：

| 模块 | 职责 |
|------|------|
| `main.rs` | 应用状态 `RambleApp`、`eframe::App` 实现、工具栏 / 状态栏 / 查找栏布局、快捷键与未保存确认 |
| `theme.rs` | VSCode Dark+ / Light+ 配色、`Visuals` 与字号、系统中文字体加载 |
| `pane.rs` | 编辑区与预览区渲染，返回 `(滚动偏移, 最大偏移, 是否用户滚动)` 供滚动随动使用 |
| `mermaid.rs` | ```mermaid 代码块扫描、渲染、PNG 缓存与失败降级 |
| `find.rs` | 查找 / 替换状态机与查找栏 UI |
| `recent.rs` | 最近文件列表的读写与去重 |

**滚动随动原理**：每帧比较当前偏移与上一帧记录值来判断"是用户滚的还是程序滚的"，按 `偏移 / 最大偏移` 计算比例并写入对侧（`ScrollArea::vertical_scroll_offset`），从而避免两侧互相触发形成抖动死循环。

---

## 📁 项目结构

```
ramble-markdown/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── ramble-markdown.exe            # 已构建的可执行文件（约 20MB）
├── 欢迎文档.md                     # 功能演示文档
├── 示例-图表与高亮.md               # mermaid / 高亮 / 查找替换测试文档
├── src/
│   ├── main.rs                    # 应用与 UI 组装
│   ├── theme.rs                   # 配色与字体
│   ├── pane.rs                    # 编辑区 / 预览区
│   ├── mermaid.rs                 # mermaid 渲染（含单元测试）
│   ├── find.rs                    # 查找替换（含单元测试）
│   └── recent.rs                  # 最近文件（含单元测试）
└── target/                        # 构建产物
```

## ✅ 测试

```bash
cargo test --release
```

覆盖 mermaid 渲染管线（含缓存、失败降级、原文不被篡改）、查找替换（大小写、中文、区间正确性、循环跳转）与最近文件持久化，共 16 个用例。

## ⚠️ 已知限制

- mermaid 支持流程图、时序图、状态图、类图、ER 图、饼图、甘特图、时间线、思维导图、gitGraph 等；复杂的 `%%{init}%%` 主题配置仅部分支持
- mermaid 图表渲染为位图，跟随界面缩放会略微模糊
- 大文档（数千行）下每帧全量渲染预览，可能有轻微卡顿
- 目前仅测试于 Windows；字体加载逻辑包含 macOS / Linux 兜底路径但未验证

## 📌 待办

- [ ] 编辑器行号 / 当前行高亮
- [ ] 自动保存与文件变更外部监听
- [ ] 导出 HTML / PDF
- [ ] 大纲 / 目录侧栏
- [ ] 图片粘贴上传与本地图片相对路径支持

---

## 📝 更新日志

### v0.3.0
- 新增 **mermaid 图表渲染**（纯 Rust，无浏览器依赖），支持流程图 / 时序图 / 状态图 / 饼图等多种图表，渲染结果带缓存与失败降级
- 新增 **代码块语法高亮**（syntect）
- 新增 **查找替换**：`Ctrl+F` / `Ctrl+H` / `F3`，支持大小写敏感、匹配计数、替换当前与全部替换
- 新增 **最近打开文件列表**（最多 10 个，可在工具栏「最近」中访问与清理）
- 代码按职责拆分为 6 个模块，并补充 16 个单元测试

### v0.2.0
- 新增三档视图模式（分栏 / 仅编辑 / 仅预览）
- 新增编辑区与预览区双向滚动随动
- 重做界面：VSCode 风格配色，新增暗色 / 浅色切换
- 新增 Windows 右键菜单集成
- 项目更名为 `ramble-markdown`
- 修复：关闭窗口时点击「不保存」无反应（`Close` 触发的关闭事件被重复拦截）

### v0.1.0
- 首个版本：Markdown 编辑 + 实时预览、文件读写、未保存保护、拖拽打开
- 修复：中文字符显示为方块（加载系统 CJK 字体）
