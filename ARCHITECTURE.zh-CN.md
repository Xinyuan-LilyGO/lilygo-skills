# 架构

English version: [ARCHITECTURE.md](ARCHITECTURE.md)。相关文档：
[Context layers](docs/CONTEXT_LAYER.md) /
[中文](docs/CONTEXT_LAYER.zh-CN.md)，[Skill generation](docs/SKILL_GENERATION.md)
/ [中文](docs/SKILL_GENERATION.zh-CN.md)，[Board facts](docs/BOARD_FACTS.md) /
[中文](docs/BOARD_FACTS.zh-CN.md)，[Source recovery](docs/SOURCE_RECOVERY.md) /
[中文](docs/SOURCE_RECOVERY.zh-CN.md)，[Action routing](docs/ACTION_ROUTING.md) /
[中文](docs/ACTION_ROUTING.zh-CN.md)，[Verification levels](docs/VERIFICATION_LEVELS.md)
/ [中文](docs/VERIFICATION_LEVELS.zh-CN.md)。

lilygo-skills 是面向 LilyGO 开发 Agent 的 Rust CLI 加 Skill runtime。它把自然语言的
板级任务转换成紧凑上下文包，并提供确定性的源码查询、runtime skill 生成、setup 规划
和显式授权的证据采集命令。

Runtime 把“上下文准备好”和“硬件已验证”分开处理。Route、source、generation、install
和 benchmark 检查证明 Agent 拿到了正确上下文，并且 runtime 可以被复现；build、
simulator、flash、serial、OTA、display、RF 和外设行为只由各自的 V4/V5 证据产物验证。

架构按 board family 扩展。当前已验证 runtime 覆盖从 LilyGO ESP32 系列产品开始：
ESP32、ESP32-S2、ESP32-S3、ESP32-C3、ESP32-P4。

## Runtime Surface

```text
用户 prompt
  -> route/project/profile resolver
  -> 匹配 skill id 和紧凑摘要
  -> 可选 goal complete / goal plan
  -> 可选 source query / source completeness / enrichment dry-run
  -> 只有显式授权后才执行带权限的 goal start evidence
```

主要入口：

- `route --json <prompt>`：选择板子、框架、外设、功能、应用和工具 Skill。
- `hook codex|claude`：给 AI 客户端使用的安装态上下文 envelope。
- `project init/show/clear`：每个固件仓库的板子/框架默认值。
- `goal complete/plan/start/status/evidence/cancel`：完成状态、实现和调试目标、
  权限和证据执行。
- `source query`：查询 IO、pinout、bus、expander、connector、peripheral facts。
- `source completeness`：查询某个板子/主题是否足够 quick-start。
- `update board-facts`：显式 enrichment 某个板子/主题。
- `setup plan`：空白机器的只读工具链 setup 计划。
- `verify` 和 `benchmark`：完整性和路由质量门。
- `doctor --json`：安装态 runtime 和注入链路健康检查。

- `generate skills --out <dir>`：把每个 runtime skill 生成到 generated cache；
  报告 skill 数量、source-pack id、source hash、warnings 和 verification hints。

安装和 setup 是分开的。`install.js --build` 可以从当前 checkout 编译 Rust CLI，
再用这个二进制和 source model 安装 Skill runtime。它不是拷贝一份已提交的 skill
快照，而是通过调用 CLI 的 `generate skills` 把 runtime skills 生成到安装目录。
它不安装宿主工具链或固件依赖。`setup plan` 只报告 readiness check 和安装提示，
不做 mutation；Agent 只有在用户明确同意后，才应该在 setup-plan 命令之外执行真实
工具安装。

## Layer Model

| Layer | 作用 | 默认注入 |
|-------|------|----------|
| L0 | Router、hook、verify、benchmark | decision、matched ids、reasons |
| L1 | Board/product/MCU-series skills | 板子摘要、source pointer |
| L2 | Peripheral/chip/feature skills | 相关芯片、外设或功能上下文 |
| L3 | Framework skills | Arduino、ESP-IDF、Rust esp-rs、PlatformIO、LVGL |
| L4 | Recipe/evidence context | OTA、UI、flash、serial、simulator |
| L5 | Project-local context | `.lilygo-skills/project.json` 默认值和澄清问题 |
| L6 | Goal planner | recipes、permissions、artifacts、evidence boundary |
| L7 | Source facts and preferences | 紧凑 fact tables、lookup commands、read hints |
| L8 | Source completeness | topic status 和 enrichment 下一步 |
| L9 | Embedded playbooks | source-first 操作模式和证据清单 |
| L10 | Completion coordinator | route、generated-root、source、setup、权限和证据状态 |
| L11 | Action routing | 按意图选择 demo、通用 bus lookup、project custom skills 和 doctor |

已提交的 router Skill 是嵌入式开发控制面。它告诉 Agent 如何分类 LilyGO 任务、
读取官方来源、规划有界调试闭环、申请权限、只在授权后运行本地命令、分类失败，并记录
V3/V4/V5 证据。自动生成的 skill 是数据面：短小的板子、外设、框架、芯片、应用和
recipe 上下文，用来帮助 meta Skill 选择下一步 source lookup 或命令。

核心规则是渐进式披露：route/hook 输出保持小而稳定；完整源码、fact pack 和参考
文档通过命令按需读取。Embedded playbook 也遵循同样规则：route 和 hook 只注入 id
和短 hint；只有实现、setup、调试或证据采集任务需要时，才通过
`index query playbook-* --json` 展开完整生成 playbook。

Source recovery 是跨 layer 的输出，而不是另一套命令体系。对实现和调试 prompt，
`goal plan`、hook context、`source query` 和生成的 board skill 会收敛到同一组
official-demo-first 上下文：demo 路径、板子自己的 header、关键事实和恢复命令。

Action routing 是下一层跨 layer 输出。它不扩大默认上下文，而是在实现/调试 capsule
里加入紧凑的 `next_actions`：最小官方 demo、IO/bus source-query 命令、
project-local custom skill hint，以及带权限标记的 build/flash/serial/network/OTA
路径。纯事实查询保持紧凑和只读。

公开源码树是 meta-only。唯一提交的 Skill 是 `skills/lilygo-router/SKILL.md`，即
meta router。板子、系列、框架、工具、外设、芯片、功能、debug、应用等 skill 都不再
提交，而是按需从 source model 生成。真实来源在 `data/`、`index/` 和官方资料中；
runtime skills 是生成产物，绝不是手改产物。生成与安装的关系见下面的 meta-only
发布边界章节。

## 架构边界

这些边界保证 runtime 有用，但不会让生成上下文变得臃肿或过度宣称：

- **Completion coordinator**：`goal complete` 把 route、project、generated root、
  source facts、setup、permissions、goal execution 和 evidence 组合成一个状态机。
  它可以汇总没有 formal completeness topic 的 source-backed facts 和 demo refs，
  但不能把上下文变成硬件成功声明。
- **Route token model**：路由使用明确 token 和数据驱动触发词。`sx1262` 这类具体
  芯片型号是有效触发词；不恢复不安全的前缀或子串匹配，因为那会重新引入
  `pio` 命中 `GPIO` 这样的误触发。
- **Generated chip taxonomy**：chip skill 只代表真实芯片标识。复合标签、内存容量、
  存储介质和可选项字符串保留为板子/外设 source facts，不铸造成 chip route。
- **Runtime materialization**：`install.js` 拥有 runtime root，并把生成数据/source
  data 镜像到那里。Claude `settings.json` 和 Codex `AGENTS.md` 这类用户文件只做
  marker 范围内的 merge。
- **Gate inventory**：每个保护发布边界的确定性 smoke 都必须直接出现在
  `scripts/ci-gate.sh`，即使另一个 smoke 已经间接运行它。

## Meta-Only 发布边界与生成流水线

公开源码树是 meta-only。唯一提交的 Skill 是 `skills/lilygo-router/SKILL.md`，即
Agent 加载的 meta router。其余每个 runtime skill（板子、系列、框架、工具、外设、
芯片、功能、debug、应用）都是从 source model 生成的产物，而不是提交的文件。

生成流水线从 source 输入组合出 runtime skill 集合：

```text
source packs (data/boards.json, data/peripherals/**)
  + fact packs (data/facts/**)
  + route rules (index/routes.json, data/router/derived-context.json)
  + recipe packs (data/recipes/recipes.json)
  + playbook packs (data/playbooks/playbooks.json)
  + reference practice skills (data/skills/reference/**)
  + static references (skills/references/**)
  + generation templates (templates/skills/**)
  -> generate skills --out <dir>
  -> generated cache（安装目录、项目 cache 或测试输出目录）
  -> installer 通过镜像 runtime-owned data 物化 runtime root
```

`generate skills --out <dir> --json` 写入 generated cache，并报告 `skill_count`、
`source_pack_ids`、`source_hashes`、`warnings`、`verification_hints`，绝不写入源码
树。`install.js` 安装时通过调用该命令生成到 `~/.codex/lilygo-skills/` 和
`~/.claude/lilygo-skills/`；`--all --dry-run` 报告 `generate_plans` 计划和计划写入，
但不实际生成 skill。

生成输出可以直接校验，与安装解耦：

- `verify --generated-root <dir> --json` 检查 registry/index 一致性、每个被路由到的
  skill 都存在、必需的 reference skill 都存在，以及 evidence-boundary 措辞诚实。
- `benchmark --generated-root <dir> --json --iterations <n>` 在该生成 skill 集合上
  benchmark 路由。

生成根还会带上支持文件：

- `skills/references/**`：context injection、source discovery、build/flash/serial、
  LVGL、OTA、BSP、radio/GNSS、preferences 和 generation 的静态展开文档。
- `templates/skills/**`：CLI 用来渲染生成 board、peripheral/chip/feature 和
  playbook Skill 文件的公开 Markdown 模板。

route 和 hook 保持 no-write。如果某个被路由到的生成 skill 缺失，它们可以报告并附上
一条紧凑的 generate/update 命令，但绝不隐式写入 skill，也不联网抓取 source。只有显式
的 install、update、project-init 和 generate 命令才会写入生成 skill，且只写到安装
目录、项目 cache 或测试输出目录。

Installer 把 runtime-owned 目录（`data/`、generated skills、static references、
templates、source-intake 产品数据）视为当前 checkout 的镜像，避免旧版本 stale 文件
残留在安装态 runtime 里。宿主集成文件仍属于用户，只通过有界 merge 逻辑更新。

Domain catalog 尽量放到 `data/`：route triggers（`data/router/derived-context.json`）、
recipes（`data/recipes/recipes.json`）、reference entries
（`data/references/built-in.json`）、fact/topic 关键词规则（`data/facts/*.json`）、
reference 实践 skill（`data/skills/reference/*.md`）、CLI 帮助文本
（`data/help/*.txt`）。Rust CLI 保留 parsing、routing、generation、install、privacy
和 goal policy。

在 Claude Code 宿主上,注入通过 `UserPromptSubmit` hook 完成:安装的二进制从
stdin 读取 prompt JSON,在 stdout 输出
`{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"..."}}`
(要么是信封要么为空;诊断只走 stderr;fail-open 退出码 0)。Codex 宿主通过
`AGENTS.md` 标记小节的 `hook codex` 使用传统诊断信封。

OTA、LVGL、LoRa 是 `data/recipes/recipes.json` 里的 source-backed recipe pack，不是
提交的板级外设 skill。每个 recipe source pack 引用官方上游文档（Espressif OTA 文档、
LVGL 文档和示例、RadioLib 加 LilyGO LoRa 示例）。内置 reference catalog 只包含
公开 URL，公网克隆开箱即可全部解析；`reference list --json` 报告每个条目的
source health。

## Embedded Playbooks

Playbook 是由 `data/playbooks/playbooks.json` 支撑的生成 runtime skill。它们是操作
模式，不是板级事实。一个 playbook 可以提醒 Agent 读取官方示例、对照 header、检查
setup readiness、运行有界 build/flash/serial 闭环、诊断 LVGL、检查 OTA manifest、
封装 BSP driver，或分类 RF/GNSS 证据。它不能制造缺失的 pin、bus、chip、电源轨、
demo 或 framework 事实。

每个 playbook source entry 包含：

- 带官方或项目权威的 source refs；
- required board facts；
- diagnostic axes；
- ordered steps；
- failure classes；
- evidence targets；
- 明确上下文不能证明什么的 anti-claims；
- resource hints 和 benchmark prompts。

Runtime 选择保持紧凑。用户请求实现、setup、调试、烧录、串口、LVGL、OTA、BSP、
radio、GNSS 或 source discovery 时，`route` 和安装态 hook 可以包含 `playbook-*`
id。`goal plan` 会加入小的 `playbook_hints`，其中包含 evidence targets 和展开命令。
完整 playbook 通过 `index query <playbook-id> --json` 或生成 runtime Skill 文件读取。

Playbook 的优先级低于 source facts 和 source-completeness 状态。如果某个板子/主题
缺事实，正确结果是带官方 refs 和 update dry-run 的 `needs_source_ingestion`，而不是
通用 playbook 答案。

### Release QA 与验证边界

Release QA 覆盖 source model、generation pipeline、install/runtime parity、
route/benchmark 覆盖、privacy check 和 evidence level enforcement。这些检查足以证明
context 层可复现，因此可以作为发布 Skill runtime 的依据。

硬件 workflow 使用单独的证据轨道。OTA transport、LVGL pixels、LoRa RF、flash success、
serial application logs、simulator output 和真实外设行为，只有在对应 V4/V5 产物存在时，
才从“context 可用”进入“已验证”。

## Source Authority

资料权威顺序不是扁平的：

1. 官方代码、headers、examples、manifests、board repositories。
2. 官方 LilyGO 硬件文档。
3. `https://github.com/Xinyuan-LilyGO/documentation`，也就是 wiki 内容背后的
   versioned documentation source。
4. `wiki.lilygo.cc` fallback 页面。
5. 项目 reference skills，用作实现和调试模式参考。
6. 社区或辅助工具资料，只作为 hint。

Reference hint 告诉 AI 应该读哪里，但不能覆盖 source facts。Runtime 不能证明的
精确值必须返回 `unknown_with_sources` 或 `needs_source_ingestion`，不能猜。

## Board And Project Identity

板子身份来源优先级：

1. prompt 明确写出的板子/框架。
2. 项目本地 `.lilygo-skills/project.json`。
3. 全局 active profile。
4. 结构化澄清问题。
5. 无关 prompt 返回 no-op。

prompt 中的事实永远优先。Project context 的作用是让不同固件目录拥有不同默认
板子/框架，而不修改全局 skill registry。

框架身份遵循同样规则。prompt 或 project context 明确写出时，会选择 Arduino、
PlatformIO、ESP-IDF 或 Rust esp-rs。如果实现/构建任务需要框架但当前未知，route
会返回 `needs_clarification`。轻量资料查询可以保持 framework unspecified，直到
任务真的需要工具链。

可提交的项目文件：

```text
.lilygo-skills/project.json
```

机器本地或未来证据文件：

```text
.lilygo-skills/local.json
.lilygo-skills/evidence/
```

私有状态不能提交，也不能注入公共 prompt context。
项目 OTA 执行也属于私有本地状态。agent 会从项目 manifest、脚本、reference 和
ignored 本地设置中解析 OTA runner。需要具体私有命令时，写成
`.lilygo-skills/local.json` 里的 `ota_manifest_argv` 和 `ota_observe_argv` 数组；
goal runner 会从公共 JSON 中省略私有 OTA 输出。

## Preferences And References

Preferences 是公开行为策略，不是硬件事实，也不是机器私有状态。解析顺序是内置
默认值加项目本地 `.lilygo-skills/preferences.json`。解析后的结构包括：

- `framework_order`：不明确 setup 或 planning 时的框架优先级。
- `debug_tools`：公开工具偏好，例如 `serial-mcp-server`、`espflash`、`binflow`。
- `code_limits`：生成固件改动时的代码大小和嵌套限制。
- `hardware_safety`：dry-run 和显式 flash 权限默认值。

写入路径：

```text
用户提出偏好
  -> Agent 确认这是公开行为偏好
  -> Agent 写入 .lilygo-skills/preferences.json
  -> CLI 校验并解析 preferences
  -> goal capsule 在相关场景下注入紧凑 preferences
```

Preference value 注入前会校验。串口、本地路径、LAN host、凭证、OTA 主机、原始
日志、evidence path 等私有形态值会被拒绝，或不会进入公共上下文。Preferences 只
影响工具、风格和安全行为；source facts 仍然是权威来源。

References 是 source material 的读取提示：官方示例、源码、硬件说明、数据手册、
项目设计文档或操作模式。某个 reference 本地缺失只是 source-health 信息，不代表
可以编造事实。References 只在 route、goal 或 prompt 需要时加载，并且永远不能高于
官方代码、headers、examples 或 source-backed board facts。

项目 references 使用 `.lilygo-skills/references.json`，结构是 `schema_version`
加 `entries`。用户可以要求 Agent 添加公开参考源，例如官方 LilyGO example、
datasheet 或项目设计说明。Agent 确认它是公开资料后写入项目 reference entry；CLI
会把它和内置 catalog 合并、按 id 去重、校验 privacy 和 authority，再在实现/调试类
prompt 里暴露紧凑 `reference_hints`。这个 reference entry 需要 AI 补充说明字段：
`title`、`kind`、`applies_to`、`authority`、`summary`、`read_when`、
`inject_triggers`；只存裸 URL 不足以让后续 Agent 判断怎么用。像“串口调试用
serial-mcp-server”这种工具选择首先属于 preference；只有当任务需要阅读该工具文档或
操作模式时才作为 reference。

注入是有边界的。Route 和 hook 只回答选中了哪些 skills、readiness 状态和需要澄清
什么。`goal plan` 才会额外加入紧凑 `preferences` 和 `reference_hints`；
preference hints 和 reference hints 都有预算上限，完整 reference 内容仍然留在文件
或 URL 中，由 Agent 在确实需要时再读。Preference 不会强制 reference 先加载；两者
都由 prompt、项目上下文、route 和 goal 类型共同选择。纯事实查询不会加载偏好或参考
提示，除非它们会影响用户要求的动作。Source-completeness 状态高于 reference hints：
如果板子/主题缺关键事实，应先返回 `needs_source_ingestion`，不能把 reference 当作
实现已 ready 的证明。

## Board Facts And Completeness

Board facts 存在 `data/facts/board-fact-packs.json`。外设首先属于板级事实模型；
可复用 peripheral/chip layer 帮助路由，但不能替代板子的 source-backed facts。
一个 pack 可以包含：

- MCU family 和支持框架。
- Pin、bus、expander、connector、电源、显示、无线电、传感器、存储、输入和
  peripheral tables。
- 带 authority rank 和 hash 的 source refs。
- conflicts 和 `unknown_with_sources` 条目。

Completeness 是按 topic 判断的。同一块板子可以在传感器主题上完整，在显示主题上
仍然不完整。`source completeness` 返回：

- `complete`
- `partial`
- `needs_source_ingestion`
- `unsupported`

对支持范围内但资料不足的 topic，route/hook/goal 可以暴露紧凑 readiness 和
update 命令，但只有 `update board-facts` 可以写入 enrichment 后的 fact-pack 数据。

第一次使用某块板子时，只会从已安装 registry 中选择已有生成 layer。Route 和 hook
不会联网抓取 source、不会生成新 skill、也不会修改 fact pack。更新和生成必须通过
下面的显式 update flow 完成。

## Goal Planning And Evidence

`route` 回答“需要加载什么上下文”。`goal complete` 回答“当前是什么完成状态、
哪里阻塞或是否可以继续”。当请求已经适合规划时，`goal plan` 回答“AI 接下来应该
怎样做”。Setup 也是被路由出来的计划：空白机器或缺少框架工具链时，Agent 应该先用
`setup plan`，再考虑运行真实安装器。

`goal complete` 是现有 layer 之上的有界协调器。它可以返回 `no_op`、
`needs_clarification`、`needs_generation`、`needs_source_ingestion`、
`needs_setup`、`needs_permission`、`planned`、`complete`、`blocked` 或 `failed`。
它不会添加第二套命令执行器；当权限明确且 readiness gate 通过时，它委托给
`goal start`，否则只返回下一步动作并保持 no-write。

一个 goal plan 可以包含：

- 主板子和框架。
- Source-backed facts 和官方 demo refs。
- build、upload、monitor、LVGL simulator、OTA、serial 等 recipe steps。
- 需要的权限。
- 计划 artifact。
- evidence boundary。
- 资料缺失时的 discovery hints。

Goal 执行层不限制 Agent 的资料查找路径。只有当板子 profile 已经有 source-backed
或本地验证过的事实时，它才提供已知可用的执行骨架，例如 Arduino FQBN 和必须的
library roots。缺少这些 profile 时，命令会保留为待查询占位，Agent 应该先读取官方
板子资料、框架资料、项目文件和用户 reference，再补齐可运行命令。

`goal complete` 和 `goal start` 默认 no-write。真实动作必须显式加权限，例如
`--allow-build`、`--allow-flash --port <port>`、`--allow-serial --port <port>`、
`--allow-network --allow-ota` 或 `--allow-simulator`。

## Verification Boundary

当前 runtime 已验证到 V3 source/context/completeness：路由、hook 输出、
source facts、completeness status、enrichment dry-run、benchmark、安装态 runtime
parity 都有测试。

这不等于每块板都已经物理烧录或每个 demo 都跑过。build、simulator、flash、
serial log、OTA、显示像素和真实外设行为需要 V4/V5 证据。见
[docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md)。

## Benchmark Gate

Benchmark 是 Rust CLI 的一部分，不是外部项目。源码在
`crates/lilygo-skills-cli/` 下，安装后的 `lilygo-skills` 二进制也提供同一个
`benchmark --json` 命令。

Benchmark 会检查：

- 每个已注册 skill 至少有一个覆盖到的触发路径。
- 正向 route fixtures 仍然注入预期 skills。
- 负向 fixtures 能防止短触发词或无关 skill 过度注入。
- Goal capsules 仍然包含预期的紧凑上下文。
- Goal complete 覆盖 `no_op`、`needs_clarification`、
  `needs_source_ingestion` 和 `needs_permission`。
- Baseline comparison 能发现 case 数和 skill 覆盖率退化。

`scripts/full-evidence-smoke.sh --dry-run` 会把短 benchmark 纳入 evidence pack。
发布或推送前使用更长的版本：

```bash
lilygo-skills benchmark --json --iterations 5000
```

这仍然只是 V3 context quality evidence，不证明 build、flash、serial、OTA transport、
显示像素或真实外设行为。

## File Map

```text
crates/lilygo-skills-cli/     Rust CLI implementation
data/boards.json              Generated board/product source model
data/peripherals/             Peripheral/chip/feature source packs
data/facts/                   Board fact packs
data/recipes/                 Goal recipe source packs
data/playbooks/               Generated playbook source model
data/references/built-in.json Built-in reference catalog (public URLs)
data/references/source-intake Public source-intake cache and manifest
data/skills/reference/        Reference practice skills used in generation
index/routes.json             Skill registry and triggers
skills/lilygo-router/SKILL.md Committed meta router (only committed Skill)
generated runtime skills      Produced by `generate skills` into the install root
scripts/*smoke.sh             CLI verification smokes
docs/                         Human architecture and contributor docs
```

## Update Flow

```text
update sources
  -> update boards
  -> update skills（generated cache）
  -> update source-packs
  -> update peripheral-skills（generated cache）
  -> update fact-packs
  -> update board-facts --board <id> --topic <topic>
  -> verify
  -> benchmark
```

Dry-run 必须报告 planned reads/writes 但不修改文件。Apply 必须限制在支持路径内，
并保持当前已验证支持边界。生成 skill 更新只写 `.lilygo-skills/generated-skills/`
或显式 `--out <generated-root>`，绝不能把生成的 `SKILL.md` 写回已提交的源码
`skills/` 目录。
