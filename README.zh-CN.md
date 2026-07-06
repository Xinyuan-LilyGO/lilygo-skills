# lilygo-skills

面向 AI 辅助 LilyGO 开发板开发的 Skill runtime。

把它安装到 Codex、Claude Code 或其他 AI Agent 后，用户直接用自然语言描述固件
需求即可。Agent 会按需加载对应的 LilyGO 板子、框架、source facts、官方示例、
setup hint 和安全调试步骤，不需要用户自己手动搜索文档。

已提交的 meta Skill 是上下文注入的运行入口：识别板子/框架/领域，读取官方来源，
规划有界调试闭环，返回完成状态，申请权限，只在授权后运行本地命令，分类失败，并记录
证据。自动生成的 skills 是这个运行流的补充上下文，本身不是产品边界。

文档入口：

| 主题 | English | 中文 |
|------|---------|------|
| 总览 | [README.md](README.md) | [README.zh-CN.md](README.zh-CN.md) |
| 架构 | [ARCHITECTURE.md](ARCHITECTURE.md) | [ARCHITECTURE.zh-CN.md](ARCHITECTURE.zh-CN.md) |
| 上下文层 | [docs/CONTEXT_LAYER.md](docs/CONTEXT_LAYER.md) | [docs/CONTEXT_LAYER.zh-CN.md](docs/CONTEXT_LAYER.zh-CN.md) |
| Skill 生成 | [docs/SKILL_GENERATION.md](docs/SKILL_GENERATION.md) | [docs/SKILL_GENERATION.zh-CN.md](docs/SKILL_GENERATION.zh-CN.md) |
| 板级事实 | [docs/BOARD_FACTS.md](docs/BOARD_FACTS.md) | [docs/BOARD_FACTS.zh-CN.md](docs/BOARD_FACTS.zh-CN.md) |
| Source recovery | [docs/SOURCE_RECOVERY.md](docs/SOURCE_RECOVERY.md) | [docs/SOURCE_RECOVERY.zh-CN.md](docs/SOURCE_RECOVERY.zh-CN.md) |
| 验证等级 | [docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md) | [docs/VERIFICATION_LEVELS.zh-CN.md](docs/VERIFICATION_LEVELS.zh-CN.md) |
| 新增板子 | [docs/CONTRIBUTING_BOARDS.md](docs/CONTRIBUTING_BOARDS.md) | [docs/CONTRIBUTING_BOARDS.zh-CN.md](docs/CONTRIBUTING_BOARDS.zh-CN.md) |

公开仓库只保存 runtime source：CLI、安装器、router Skill、source model、
templates、references、schemas 和发布门禁。

项目目标是逐步覆盖 LilyGO boards。当前已验证 runtime 覆盖从 LilyGO ESP32 系列产品
开始：ESP32、ESP32-S2、ESP32-S3、ESP32-C3、ESP32-P4。其他 LilyGO 产品可以记录为
未来资料候选，但在完成设计和验证前，runtime 必须对 build、flash、OTA、硬件调试
返回 unsupported。

## 安装到 AI Agent

推荐方式是直接让 AI 安装这个 Skill：

```text
帮我从 https://github.com/Xinyuan-LilyGO/lilygo-skills 安装这个 LilyGO Skill，
并在当前固件仓库里使用它。如果缺 Node.js，先告诉我；如果需要构建完整
Rust runtime，安装 Rust/Cargo 前先问我。
```

推荐环境：

- Git：用于 clone 仓库和 source reference。
- Node.js：运行 `install.js` 和挂载 Skill 必需。
- Rust/Cargo：完整动态 runtime 推荐具备。如果暂时缺失，也可以先挂载 Skill；
  Agent 会根据 setup guidance 后续帮助配置 Rust/Cargo 或安装预编译 runtime。

Agent 应该先检查：

```bash
git --version
node --version
rustup --version   # 只有本机构建 runtime 前才需要
cargo --version    # 只有本机构建 runtime 前才需要
```

如果缺 Node.js，Agent 应该说明 installer 暂时不能运行。如果缺 Rust/Cargo，
Agent 仍然可以先挂载 Skill，然后根据已挂载的 setup guidance 帮助配置 Rust/Cargo
或使用预编译 runtime。Skill installer 本身不会静默安装宿主依赖。
Arduino CLI、PlatformIO、ESP-IDF、esp-rs、board core、串口工具、LoRa/GNSS
依赖等框架工具，也会由 Agent 根据 `setup plan` 和当前固件任务继续配置。

Git 和 Node.js 具备后的手动挂载：

```bash
git clone https://github.com/Xinyuan-LilyGO/lilygo-skills.git
cd lilygo-skills
node install.js --all --dry-run
node install.js --all
```

安装位置：

```text
~/.codex/lilygo-skills/
~/.claude/lilygo-skills/
```

安装器同时会接好两个宿主的注入链路：

- **Claude Code**：把带 YAML frontmatter 的 router skill 装到
  `~/.claude/skills/lilygo-skills/SKILL.md`，并向 `~/.claude/settings.json`
  幂等合并一个 `UserPromptSubmit` hook：

  ```json
  {
    "hooks": {
      "UserPromptSubmit": [
        {
          "hooks": [
            {
              "type": "command",
              "command": "\"$HOME/.claude/lilygo-skills/bin/lilygo-skills\" hook claude"
            }
          ]
        }
      ]
    }
  }
  ```

  LilyGO 相关 prompt 会通过 `hookSpecificOutput.additionalContext` 信封注入
  上下文；无关 prompt 保持静默（fail-open，退出码 0）。如果 `settings.json`
  不是合法 JSON，安装器会明确报错并打印上面的手工接线片段，不会碰这个文件。
  重复安装不会产生重复条目。

- **Codex**：向 `~/.codex/AGENTS.md` 追加一段带标记的 `lilygo-skills` 小节
  （只追加一次，重装时原位替换），指向 runtime 根目录和 `route --json`
  发现协议。

卸载请按顺序：先删除 `~/.claude/settings.json` 里的 `UserPromptSubmit` 条目
（否则每条 prompt 都会报 hook 命令失败），再删除
`~/.claude/skills/lilygo-skills/`、`~/.claude/lilygo-skills/`、
`~/.codex/AGENTS.md` 里的标记小节和 `~/.codex/lilygo-skills/`。

公开源码树是 meta-only：唯一提交的 Skill 是 `skills/lilygo-router/SKILL.md`，
即 meta router。板子、系列、框架、工具、外设、芯片、功能、debug、应用等 skill
不再提交到 `skills/` 下，而是按需从 `data/` 里的 source model 生成。
`skills/references/` 下的静态文档和 `templates/skills/` 下的生成契约会提交，
方便用户查看上下文如何选择、生成的 Skill 文件如何成形。

当已有编译好的 runtime 时，`install.js` 会通过调用 CLI 的 `generate skills`
把 runtime skills 生成到安装目录，而不是拷贝一份已提交的快照。完整安装内容包括
`lilygo-skills` 二进制、刚生成的 runtime skills、source/fact 数据，以及 Agent 会
加载的 meta router Skill。真实 source truth 在 `data/`、`index/` 和官方资料里；
完整安装每次都从这个 source model 重新生成 skills。
安装目录也包含 `skills/references/` 和 `templates/skills/`，因此安装态 Agent
不需要浏览源码 checkout 也能查看同一套契约。

如果没有编译好的 runtime，且没有显式传 `--build`，`install.js` 仍会以
**mount-only** 模式成功挂载。它会接好 Codex/Claude 入口，复制 meta router、
data、templates 和 references，并安装一个很小的 setup-only launcher。这个
launcher 不会伪装成完整板级事实注入；它会提示 Agent 先运行 `setup plan`，再构建
或安装 runtime 后继续深入固件开发。

需要在同一步升级为完整动态上下文时，使用 `--build`。
`install.js --build` 会先运行 `cargo build --release -p lilygo-skills-cli`。
不带 `--build` 时，安装器默认优先安装 `target/release/lilygo-skills`，缺失时回退到
`target/debug/lilygo-skills`，再缺失则回退到 mount-only；也支持显式指定二进制：

```bash
node install.js --all --dry-run
node install.js --all
node install.js --all --dry-run --build
node install.js --all --build
node install.js --all --profile release
node install.js --all --bin /path/to/lilygo-skills
```

正常安装后直接调用 `lilygo-skills`。`cargo run` 只用于源码 checkout 内的开发
和测试。如果 Agent 已经有编译好的二进制，可以用 `--bin` 直接安装这个产物，不需要
再次构建 CLI。

安装器也不会静默安装 Arduino CLI、PlatformIO、ESP-IDF、esp-rs、board core、
固件库或 LoRa/GNSS 依赖。

Setup 会先通过 Skill 路由，而不是直接运行安装器。机器 readiness 由只读 setup
planner 处理：

```bash
lilygo-skills setup plan --framework arduino --json
lilygo-skills setup plan --framework platformio --json
lilygo-skills setup plan --framework esp-idf --json
lilygo-skills setup plan --framework rust --json
```

`setup plan` 只返回检查项和安装提示，`writes=[]`；它不会安装包、修改固件文件、
打开串口或烧录硬件。

## 用自然语言开始

安装后，用户不需要先研究 CLI，直接对 AI 说需求即可：

```text
我在用 LilyGO T-Display-S3，PlatformIO Arduino 项目。
帮我接一个 I2C 温湿度传感器，并把读数显示到 LVGL 屏幕上。
```

```text
这个仓库目标是 LilyGO T-Beam。
帮我搭 LoRa + GNSS 遥测，并给出串口调试路径。
```

```text
我有一个 LilyGO T-Deck 显示项目。
帮我找到显示和输入相关资料，做一个小 UI，并说明如何验证。
```

Agent 会用这个 Skill 判断应该注入哪些紧凑上下文、读哪些官方示例或源码、以及
哪些 setup/debug 命令可以安全执行。

常见任务可以直接这样说：

| 用户可以说 | Agent 背后应该触发 |
|------------|--------------------|
| “帮我在这个仓库初始化 LilyGO Skill，我用 T-Display-S3 和 PlatformIO。” | `project init`，写 `.lilygo-skills/project.json`，生成 ignored 的项目 cache |
| “重新生成这个项目的 LilyGO skills，并检查是否完整。” | `generate skills --out .lilygo-skills/generated-skills` + `verify --generated-root` |
| “我准备用 Arduino/ESP-IDF/PlatformIO/Rust 开发，帮我检查环境。” | `setup plan --framework ...` |
| “这个板子的屏幕/LoRa/GNSS/某个传感器怎么接、用哪个 demo？” | `source query` 和必要的 generated board/peripheral layer |
| “帮我实现这个功能，先告诉我还缺什么信息。” | `goal complete --dry-run` 或 `goal plan` |
| “跑 benchmark，确认这个 Skill 注入没有退化。” | `benchmark --generated-root ...` 或默认 registry benchmark |
| “验证到 V3/V4/V5，并说明证据是什么。” | 按验证等级选择 route/source/build/flash/serial/OTA/display 证据 |

这些自然语言请求会触发明确的运行路径。普通问答不会隐式写文件；只有用户要求安装、
初始化、生成、更新、实现或验证时，Agent 才会进入对应的写入或执行命令。

对于实现、setup、demo 和调试类任务，Agent 通常先运行：

```bash
lilygo-skills goal complete --dry-run --json "<prompt>"
```

这个 capsule 会直接说明当前 completion state：是否已经 ready、是否需要询问板子或
框架、是否需要 source ingestion、是否缺 generated skills、是否需要 setup、是否需要
显式权限，或者是否可以交给已有安全 goal runner 执行。

对于实现或调试类请求，Skill 还会路由自动生成的 playbook。它们是精简的操作指南，
覆盖 source discovery、setup、build/flash/serial、LVGL、OTA、BSP driver、
radio/GNSS 等工作。Agent 起初只看到紧凑的 playbook id 和摘要；只有任务需要完整
检查清单时，才会用 `lilygo-skills index query playbook-lvgl-debug --json` 或对应
`playbook-*` id 展开。

如果用户明确说了框架，Agent 会加载对应框架 layer。如果某个构建/实现任务需要
框架，但 prompt 和项目上下文都没有提供，runtime 会返回 `needs_clarification`，
让 AI 在 Arduino、PlatformIO、ESP-IDF、Rust esp-rs 等选项中询问用户。轻量资料
查询可以保持 framework unspecified，不会过早替用户选择。

## Agent 会做什么

以 T-Display-S3 传感器示例为例，Skill 会帮助 Agent：

1. 从 prompt 或项目上下文识别精确板子和框架。
2. 只加载当前任务需要的板子、框架、显示、传感器相关 layer。
3. 需要实现细节时查询 source-backed facts：引脚、总线、连接器、外设、demo。
4. 如果已知板子/主题资料还不完整，返回 `needs_source_ingestion`、官方资料和
   update 命令，而不是直接猜。
5. 加入 source-first 的 playbook hint，并只在需要详细清单时展开对应 playbook。
6. 先用 `goal complete` 判断下一步 completion state。
7. 为 Arduino、PlatformIO、ESP-IDF、Rust esp-rs、LVGL、串口、OTA、simulator、
   build、flash 生成 setup、source 或 debug plan。
8. 在接触硬件、串口、网络、OTA 或 simulator artifact 前先请求权限。

这样没有经验的用户也能从“板子名 + 目标”开始，同时避免 AI 编造 GPIO、bus、
屏幕芯片或不支持的工作流。

外设首先是 board facts：引脚、总线、expander、connector、电源、屏幕、无线电、
传感器、存储、输入设备和 demo 都应该来自对应板子的 source-backed fact pack。
peripheral/chip layer 是跨板复用的索引，帮助路由相似器件，但不能替代板子自己的
事实。以 LoRa/GNSS 为例，runtime 可以路由到 T-Beam、LoRa、GNSS、Arduino 和
serial-debug 上下文，但具体芯片、总线、天线、区域和 demo 指导仍取决于这块板子的
source completeness。

## 渐进式披露

Runtime 采用分层设计，避免一次性把所有文档塞进上下文。

| Layer | 何时加载 | 作用 |
|-------|----------|------|
| L0 | 始终 | Router、hook envelope、verify、benchmark |
| L1 | 识别到板子/项目上下文 | LilyGO 板子、MCU 系列、source pointer |
| L2 | 识别到外设/芯片/功能 | Display、sensor、GNSS、LoRa、power、storage、input |
| L3 | 识别到框架 | Arduino、PlatformIO、ESP-IDF、Rust esp-rs、LVGL |
| L4 | 要实现或调试 | Build、flash、serial、OTA、simulator、app recipe |
| L5 | 固件仓库上下文 | `.lilygo-skills/project.json` 默认值和澄清问题 |
| L6 | 用户要求实现/调试 | Goal plan、权限、artifact、证据边界 |
| L7 | 需要细节 | Source facts、preferences、reference read hints |
| L8 | 资料不完整 | Completeness 状态和 enrichment 下一步 |
| L9 | 需要可复用实现/调试模式 | 生成的 playbook hint 和展开命令 |
| L10 | Agent 需要完成任务 | `goal complete` 状态、计划、权限和证据摘要 |

默认注入很小：id、摘要、top facts、readiness status 和查询命令。完整 fact pack、
官方源码和长参考文档只在任务需要时再读取。Playbook 也遵循同样规则：route 和 hook
只注入类似 `playbook-lvgl-debug` 的紧凑 id；只有用户要求实现、调试、setup、烧录、
验证或诊断时，Agent 才展开完整生成 playbook。

第一次使用某块板子时，安装态 runtime 会从安装目录里选择已经生成好的相关 layer。
route 和 hook 保持 no-write：如果某个被路由到的生成 skill 缺失，它们可以报告这一点
并附上一条紧凑的 generate/update 命令，但绝不会隐式写入 skill，也不会联网抓取资料。
只有显式的 install、update、project-init 和 generate 命令才会写入生成 skill，且只写
到安装目录、项目 cache 或测试输出目录。新增或过期资料通过显式 `update boards`、
`update skills`、`update source-packs`、`update board-facts` 命令刷新。

## 项目上下文

每个固件仓库可以保存公开默认值：

```bash
lilygo-skills project init \
  --project /path/to/firmware \
  --board board-t-display-s3 \
  --framework fw-platformio \
  --json
```

这会写入可提交的 `.lilygo-skills/project.json`，并生成 ignored 的项目 cache
`.lilygo-skills/generated-skills/`。机器本地证据应该放在
`.lilygo-skills/local.json` 或 `.lilygo-skills/evidence/`，并保持 ignored。
OTA 执行也走这个私有层。用户提出 OTA 需求时，agent 会先从项目 manifest、
脚本、reference 和 ignored 本地 runner 配置里找真实执行方式；如果需要，它可以
把从项目推导出的 `ota_manifest_argv` 和 `ota_observe_argv` 写进
`.lilygo-skills/local.json`，只在缺少无法推断的私有端点、凭证或传输信息时再问。
这些私有值不会进入公共 prompt context 或命令输出。

路由优先级：

```text
明确 prompt > project context > global profile > needs_clarification > no-op
```

缺少板子或框架时，Agent 会收到结构化问题，而不是静默猜测。

## Preferences 和 References

Preferences 用来告诉 Agent 你希望 LilyGO 开发怎么做，例如框架优先级、调试工具、
代码大小限制和安全默认值。它是行为策略，不是资料来源：

```bash
lilygo-skills preference show --json
lilygo-skills preference show --project /path/to/firmware --json
```

项目偏好写在 `.lilygo-skills/preferences.json`，只要里面都是公开行为偏好，就可以
提交：

用户不需要一开始手写这个文件，可以直接对 AI 说：

```text
这个固件仓库优先用 PlatformIO，串口调试用 serial-mcp-server，单个固件函数控制在 60 行以内。
```

Agent 应该先确认这些是公开行为偏好，然后写入或更新
`.lilygo-skills/preferences.json`。CLI 会负责解析、校验，并且只在实现或调试类
prompt 需要时注入紧凑结果。

```json
{
  "framework_order": ["platformio", "arduino", "esp-idf", "rust"],
  "debug_tools": ["serial-mcp-server", "espflash", "binflow"],
  "code_limits": {
    "max_function_lines": 60,
    "max_file_lines": 500,
    "max_nesting": 3
  },
  "hardware_safety": {
    "prefer_dry_run": true,
    "require_explicit_flash": true
  }
}
```

不要把串口、Wi-Fi、OTA 主机、凭证、原始日志、本地 evidence path 写进 preferences。
这些属于 ignored local state，例如 `.lilygo-skills/local.json`。

References 用来告诉 Agent 在需要更多上下文时应该读哪些资料：

```bash
lilygo-skills reference list --json
lilygo-skills reference list --project /path/to/firmware --json
```

References 通常应该是官方示例、源码、数据手册、硬件说明或项目本地设计文档。
比如用户可以说：

```text
把 LilyGoLib factory example 作为这个仓库的显示和外设 bring-up 参考。
```

确认后，Agent 写入 `.lilygo-skills/references.json`。这里不能只存一个裸链接，AI
应该补上说明字段：`title`、`kind`、`applies_to`、`authority`、`summary`、
`read_when`、`inject_triggers`，让后续 Agent 知道这份资料是什么、什么时候读、
为什么读。

```json
{
  "schema_version": 1,
  "entries": [
    {
      "id": "project-lilygo-factory-example",
      "title": "LilyGoLib factory example",
      "kind": "official-example",
      "applies_to": ["display", "peripheral", "bring-up"],
      "path_or_url": "https://github.com/Xinyuan-LilyGO/LilyGoLib/blob/master/examples/factory/factory.ino",
      "authority": "source-navigation",
      "summary": "Read as an official example before changing board display or peripheral bring-up code.",
      "read_when": "User asks to implement or debug display, sensor, radio, or board bring-up behavior.",
      "inject_triggers": ["display", "sensor", "peripheral", "bring-up", "factory"]
    }
  ]
}
```

`serial-mcp-server` 更适合作为 preference 例子，因为它是调试工具偏好。它也可以在
内置 tool reference catalog 里出现，但项目 references 通常应该指向代码、官方示例、
板级文档、数据手册或项目设计说明。Preference 不会强制 AI 先读 reference；prompt、
项目上下文、route 结果和 goal 类型会一起解析，`goal plan` 只注入相关的紧凑
`preferences` 和 `reference_hints`。Source-completeness 和 board facts 优先级更高：
如果板子/主题缺关键事实，capsule 应该先返回 `needs_source_ingestion`，而不是把一个
reference 链接当作已经足够。

内置 reference catalog 只包含公开 URL（官方文档、工具参考），公网克隆开箱即可
全部解析；`reference list --json` 会报告每个条目的 source health。

OTA、LVGL、LoRa 不是提交的板级外设 skill，而是 `data/recipes/recipes.json` 里的
source-backed recipe pack。每个 recipe source pack 引用官方上游文档（Espressif OTA
文档、LVGL 文档和示例、RadioLib 加 LilyGO LoRa 示例）。`goal plan` 会暴露
recipe id、source-pack id 和官方 refs，让 Agent 先读权威来源。

生成 playbook 是 recipes 和 source facts 之上的操作模式层。它们来自
`data/playbooks/playbooks.json`，和其他 deep skills 一样生成到 runtime。Playbook
不会制造板级事实，也不会宣称硬件成功；它告诉 Agent 应该读哪些 source、检查哪些
失败维度、需要什么证据，以及哪些结论不能只靠上下文来声称。

## 更新和资料刷新

用户可以直接说：

```text
更新这个板子的 LilyGO Skill 资料，并检查 display facts 是否完整。
```

后台通常先跑 dry-run：

```bash
lilygo-skills update sources --dry-run --json
lilygo-skills update boards --dry-run --json
lilygo-skills update skills --dry-run --json
lilygo-skills update source-packs --dry-run --json
lilygo-skills update peripheral-skills --dry-run --json
lilygo-skills update fact-packs --dry-run --json
lilygo-skills update board-facts --board <board-id> --topic <topic> --dry-run --json
lilygo-skills verify --json
lilygo-skills benchmark --json --iterations 5000
```

由于 skill 是生成而非提交的，可以直接生成一份 generated cache 并检查：

```bash
lilygo-skills generate skills --out <dir> --json
lilygo-skills verify --generated-root <dir> --json
lilygo-skills benchmark --generated-root <dir> --json --iterations 5000
```

`generate skills` 把每个 runtime skill 写入 generated cache（绝不写入源码树），
并报告 `skill_count`、`source_pack_ids`、`source_hashes`、`warnings` 和
`verification_hints`。`update skills` 和 `update peripheral-skills` 是同一条
generated cache 路径的兼容入口；默认写 `.lilygo-skills/generated-skills/`，
也可以用 `--out` 指向其他 generated root。`verify --generated-root` 双向检查该
generated cache：registry/index 一致性、每个被路由到的 skill 都存在、没有未注册
generated skill、必需的 reference skill 都存在，以及 evidence-boundary 措辞诚实。
`benchmark --generated-root` 在该生成 skill 集合上 benchmark 路由。

`benchmark` 是这个工程和安装后二进制 `lilygo-skills` 自带的质量门。它验证的是
路由和上下文注入质量：route fixtures、避免过度注入的 negative cases、已注册
skill 覆盖率、goal capsules 和 goal complete 状态覆盖都必须通过。它不是硬件性能 benchmark。普通用户不需要
每次提问都跑；Agent 或维护者在更新 source、skill、router 或 goal 后，以及发布
新版 Skill 前运行。

只有在 planned reads/writes 正确时才移除 `--dry-run`。route、hook 和 goal plan
本身不会修改 source data。

## 给 Agent 的直接命令

CLI 是 Skill 背后的实现层。常用命令：

```bash
lilygo-skills route --json "<prompt>"
lilygo-skills goal complete --dry-run --json "<prompt>"
lilygo-skills goal plan --json "<prompt>"
lilygo-skills setup plan --framework platformio --json
lilygo-skills source query --board <board-id> --topic io --json
lilygo-skills source completeness --board <board-id> --topic display --json
lilygo-skills reference list --json
lilygo-skills preference show --json
lilygo-skills index query playbook-lvgl-debug --json
lilygo-skills generate skills --out <dir> --json
```

源码 checkout 内开发时等价形式是：

```bash
cargo run -p lilygo-skills-cli -- <command>
```

本 README 是主要使用文档。
