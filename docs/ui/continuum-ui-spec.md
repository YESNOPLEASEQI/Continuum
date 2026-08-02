# Continuum UI 设计规格

> 最终方向：方案 2「动态档案编辑台」。本规格取代第一轮常驻侧栏方案。目标不是给旧页面换皮，而是把项目、会话、上下文与执行记录组织为同一张可展开、可重排的本地工作桌。

## 已读取的原始 Motion Prompts

当前会话未暴露可调用的 motionprompts MCP，因此完整读取本机 `C:\Users\www30\Desktop\motionprompts-mcp` 中的原始 `prompt.md`：

- `contact-form`：字符级 reveal、基线遮罩、全屏覆盖层及同一暂停 timeline 的正反播放。
- `project-page-overlay-animation`：项目详情从屏外进入、倾斜恢复为平面，以及关闭时镜像反向退出。
- `creative-clutter`：`Flip.getState()`、写入新布局、`Flip.from()` 解释同一内容的空间重排。
- `sidebar-slide-out-menu`：主内容退后、检查内容分组错峰进入、关闭时反向退出。
- `page-transitions`：双层 10×11 block grid，在完全覆盖时执行重大切换并阻止重入。
- `audemarspiguet-menu`：左右全屏面板以中心内侧为轴，从 ±180° 与 2× scale 旋转合拢；汉堡变为 X；遮罩文字在面板到位后 stagger 揭开；同一 paused timeline 正反播放。
- `brandappart-sticky-cards`：透视卡组、pinned scroll、前卡上移并 rotationX 35° 退场、后卡逐级前移放大。

## 视觉方向

Continuum 是“动态档案编辑台”，不是 AI SaaS 仪表盘。

- 背景：矿物雾灰 `#d8dfdc`，不是暖奶油色。
- 桌面：冷瓷白 `#eef1ed`。
- 主墨色：深海军蓝 `#15243d`。
- 强调：安全琥珀 `#d38a2f`。
- 次强调：氧化珊瑚 `#c85c52`。
- 静默信息：矿物灰 `#687371`。
- 夜间覆盖层：墨蓝黑 `#0d1727`。

显示字体使用 Windows 自带 `Bahnschrift SemiCondensed`，正文使用 `Segoe UI Variable`，路径、ID 和状态使用 `Cascadia Mono`。界面以直角、细边、档案脊线和真实编号组织信息；圆角只用于输入、状态与必要浮层。不使用渐变、玻璃拟态、发光边框或圆角卡片网格。

唯一签名元素是“项目档案卡组”：项目不是普通列表项，而是带档案脊、会话标签、Context Health 轨道和真实更新时间的可滚动项目封面。选择卡片后，它扩展为项目概览，再进入同一空间中的项目工作桌。

## 最终页面流程

```text
启动首页（无常驻侧栏）
  ├─ 全屏 Overlay Menu
  ├─ 项目档案卡组
  ├─ 最近会话标签
  └─ 扫描 / 创建项目
      → 项目概览档案页
      → 项目工作桌
          ├─ Chat
          ├─ Sessions
          ├─ Graph
          ├─ Context
          ├─ Activity
          └─ Files
              ├─ 右侧：Context Inspector / Session Detail
              └─ 底部：Branches / Git / Skills / MCP / Diagnostics
          → Fresh Continuation 真实执行清单
```

旧深层路由继续保留，保证外部链接、Tauri bridge 与测试入口稳定；路由的视觉表现改为同一档案桌中的页、检查器或抽屉。

## 页面与操作模型

### 全局外壳

- 删除常驻全局侧栏。
- 顶部只保留品牌、当前路径、扫描状态和 Menu 按钮。
- Menu 使用 `audemarspiguet-menu` 的双面板结构和同一 timeline 反向关闭。快速重复点击只改变 timeline 方向，不创建新动画。
- Ctrl/Cmd+K 保留为搜索入口；Escape 关闭 Menu、检查器或抽屉。

### 首页

- 首页首先建立 Continuum 的独立身份，然后进入项目卡组，不展示营销文案。
- 项目卡组使用 ScrollTrigger pinned timeline；每一段滚动只负责一张卡的退场与下一张卡的前移。
- 卡片显示真实项目、当前任务、分支、会话数、最近会话、Context Health 和路径状态。
- reduced motion 下取消 pin、rotationX 和 scrub，退化为正常文档流。

### 项目概览

- 由所选档案封面进入全屏概览页；背景项目卡组保持空间连续性。
- 概览只展示进入工作区前需要判断的信息：目标、当前任务、分支、会话、上下文健康、路径状态。
- 打开工作区属于重大空间切换，使用 block grid；重命名、归档和 Context 是次要操作。

### 项目工作桌

- 不再使用项目内常驻左侧栏。
- 项目名称、当前分支、来源会话数量和 Context Health 位于顶部索引带。
- Chat、Sessions、Graph、Context、Activity、Files 是同一工作桌的不同排版。切换时使用 GSAP Flip 解释已有区域如何移动和缩放，不做廉价整页淡入。
- 分支与来源会话入口进入底部索引抽屉；Context Inspector 与 Session Detail 从右侧进入。
- Git、Skills、MCP、Diagnostics 从底部升起，像工作桌抽屉，不长期占用宽度。

### Fresh Continuation 与重大操作

- Fresh 页面显示真实持久状态：context_compiled、writing_context、launching、detecting、binding、listening、failed 等。
- 状态变化只动画真实发生的步骤；没有百分比，不使用模拟计时器。
- 切换整个项目、完整扫描和重建索引使用 block grid；全覆盖后执行操作，随后揭开原视图，以非阻塞状态条继续显示长任务。

## 动效规则

- Menu：约 1.05 秒面板合拢，文字在 0.58 秒后以 70ms stagger 揭开；关闭使用同一 timeline reverse。
- 首页卡组：ScrollTrigger `scrub: 0.8`；前卡 `yPercent` 退场并 `rotationX: 35`，后卡同步前移和放大。
- 项目概览：520–680ms，垂直位移、rotationX 与 clip-path 同步恢复。
- 工作桌 Flip：520–640ms，`power3.inOut`；结束后只给新内容一次短 reveal。
- 底部抽屉 / 右侧检查器：420–560ms，内容分组 40–60ms stagger。
- 控件反馈：120–180ms，不为每个小按钮添加弹跳或装饰动画。
- 所有 React 动画使用 `useGSAP` scope；事件回调使用 `contextSafe`；卸载时自动 revert；新动画使用 `overwrite` 或复用单一 timeline；所有场景支持 `prefers-reduced-motion`。
