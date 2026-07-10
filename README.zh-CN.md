# lilygo-skills

面向 AI 辅助 LilyGO 开发板开发的 **skill 优先(skill-first)** 上下文 runtime。

装到 Claude Code、Codex 或任意 agent 一次即可。之后用户用自然语言描述固件需求,
agent 就能加载正确的板子、框架、带官方来源背书的引脚、官方示例、setup hint 和安全
调试步骤——无需手翻数据手册,**也绝不编造引脚**。

设计目标很窄:**在硬件事实上保持正确,在“到底验证到哪一步”上保持诚实。** runtime
说出的每个引脚、总线、电源轨都能追溯到官方上游的某一行(URL + 行号 + sha256);数据
本地随 CLI 发布,断网可用;机器可校验的诚实标记(`hardware_verified`、
`evidence_boundary`)确保“有官方来源”不会被误当成“板子真的跑通了”。

## 一段话看懂架构

产品是一个很小的 **meta Skill**(`skills/lilygo-router/SKILL.md`)加一个薄 Rust CLI。
`SKILL.md` 是*操作系统*:查询协议、调试循环、诚实规则,全部以散文形式承载。CLI 是
确定性的 **Context Kernel**:判断 prompt 是否是 LilyGO 工作、涉及哪块板子/框架、哪些
带来源的事实可以安全注入,然后把一个紧凑胶囊和精确的 `source query` 命令交给 AI,让
它按需拉取更深的内容。“选哪些子 skill / playbook 相关”由数据驱动的 trigger 决定;
“这个引脚我能不能说”由诚实规则和 source model 强制。

## 任意 agent 可用——hook 是可选的

- **纯 skill(任意宿主)。** `SKILL.md` + `lilygo-skills` CLI 在 Claude Code、Codex
  或任意 agent 上就足够。跑 `context` 拿胶囊、跑 `source query` 拉精确引脚,价值不依赖
  任何 hook。
- **可选的 Claude Code hook。** 一行 `UserPromptSubmit` hook 会自动注入胶囊,省掉手动
  `context`。它只是*便利*:没有 hook 的宿主多跑一条命令,什么都不损失——数据和质量门禁
  跨平台完全一致。

## CLI——五个日常命令

日常使用面很小、很稳定:

```bash
lilygo-skills context [--project <dir>] --json "<prompt>"   # Context Kernel:CWD → 板 → 胶囊(≤~1KB)
lilygo-skills source query --board <board-id> --topic <topic> --json   # 拉带来源的精确引脚/总线
lilygo-skills route --json "<prompt>"                        # 相关性闸门 + 匹配到的 skill/playbook id
lilygo-skills index list|query <id> --json                  # 列出已注册 skill/playbook,展开某一个
lilygo-skills doctor --json                                  # 体检注入链路
```

`context` 是一击入口:它从项目自动认板(先读 `.lilygo-skills/project.json`,否则嗅探
`platformio.ini`、`sdkconfig`、`*.ino`),返回匹配的 skill id、top 事实、验证等级和后续
查询命令。

另有少量支撑命令覆盖安装/健康、setup 和项目记忆:`verify`、
`setup plan --framework <arduino|platformio|esp-idf|rust>`、`preference show`、
`reference list`、`project init|show|clear`、`source completeness`,以及做来源补全的
`update board-facts`。**没有** `goal`、`benchmark`、`generate` 命令——那些层已折叠进
`SKILL.md` 散文与数据模型。

## 数据驱动的选择

recipe、playbook、reference hint、preference hint 都是 **JSON 数据**,不是手写的散文分支。
每类都有一张 `*-triggers.json` 表(`data/recipes/recipe-triggers.json`、
`data/playbooks/playbook-triggers.json`、`data/references/reference-triggers.json`、
`data/preferences/...`),由单一的选择引擎(`selection.rs`)把 prompt 对着这些 trigger
匹配,决定要浮现哪些紧凑 id。增删或调优一条 recipe/playbook/reference 是改数据,不是写新
Rust;reader 代码保持很薄。

- **Recipes**(`data/recipes/recipes.json`)—— OTA、LVGL、LoRa 是带官方来源背书的
  recipe pack(引用 Espressif OTA、LVGL、RadioLib + LilyGO 示例),不是提交的外设 skill。
- **Playbooks**(`data/playbooks/playbooks.json`)—— source discovery、setup、
  build/flash/serial、LVGL、BSP driver、radio/GNSS 的精简操作指南。agent 先看到紧凑 id,
  再用 `lilygo-skills index query <playbook-id> --json` 展开。

## 引导文档——薄代码 + 厚引导

行为深度以散文引导文档的形式放在 `skills/lilygo-router/guides/`,遵循 `dev-flow` 风格
(薄代码 + 厚引导)。`SKILL.md` 是入口,`context`/`source query` 的 expand 指针指向它们:

| 引导文档 | 驱动什么 |
|----------|----------|
| `query-protocol.md` | 拿 `context` → 自动认板 → 说任何引脚前先 `source query` 拉取 |
| `board-bringup-checklist.md` | 从零上手:认板 → 找官方源 → 跑官方 demo → 采证据 |
| `debug-flash-serial.md` | 有界 build → upload → monitor,以及失败归类 |
| `debug-display-bringup.md` | ST7789 / TFT_eSPI Setup vs ESP-IDF i80、背光与电源轨 |
| `debug-lvgl-loop.md` | LVGL tick / flush / draw-buffer / 触摸循环排错 |
| `debug-lora-gnss.md` | SX126x / RadioLib + GNSS 上手与排错 |
| `debug-power-battery.md` | 电源轨、充电、电量计检查 |
| `toolchain-setup.md` | Arduino / PlatformIO / ESP-IDF / Rust esp-rs 工具链(报告 + 提示) |
| `honesty-evidence.md` | 证据等级、`hardware_verified=false`、不乱编规则 |

## 相对通用 LilyGO skill 的特质

- **引脚有来源。** 每条板级事实都带官方 URL、行号和 sha256,说出的引脚能追溯到上游确切
  一行——不是从训练数据里回忆的值。
- **离线本地。** 数据随 CLI 发布,毫秒级作答;没有托管服务依赖,断网可用。
- **自动认板。** `context` 从项目文件推断板子,用户不必每次报板名。
- **质量门禁。** 棘轮化 coverage 基线、source-authority 与 auto-mapping 校验,加一个
  确定性 CI gate,守护数据。
- **诚实可机检。** `hardware_verified` 和 `evidence_boundary` 是可被 gate 打分的注入
  标记,不只是散文承诺。

## 效果(小样本,如实陈述)

早期效果 pilot(2026-07-07,记录在 `eval/fixtures/smoke-scorecard.json`)用两种方式跑
一组冷门 LilyGO 板级引脚/接线/调试问题:**完整系统**(注入胶囊 + 模型自己跑
`source query` 拉取)对比**裸模型**(无 skill)。

- **完整系统:6/6 全对,0 幻觉**,且全部附官方来源引用。
- **裸模型:命中口径 4/6,人评 2/6**,并有三处*自信的错误*——例如把 SDA/SCL 接反、把
  8-bit 并口屏说成 SPI 总线、开出错误的 TFT_eSPI Setup 文件。

pilot 展示的价值是**正确 + 可验证 + 零编造**,尤其在裸模型不确定却照样硬答的板子上。
两点诚实提醒:

- 这是**小样本 pilot**,不是大规模 benchmark,只作方向性参考。
- 完整系统的强项来自*完整*闭环(胶囊**推送**关键子集,模型再用 `source query` **拉取**
  其余)。对*不完整*胶囊做纯推送式阅读,可能比裸模型更糟——模型会锚定在部分“已验证事实”
  上,把其余靠推断补齐。修法是在 `SKILL.md` 和 guides 里写死一条硬规则:**胶囊是指针,
  不是完整引脚图——引脚不在胶囊里就是“去拉取”,绝不是“猜”。**
- **最终判据是 P4 真机 A/B**,它需要用户凭据,这里既不模拟也不伪造。

## 板子覆盖

source model 提供 **26 个 board fact pack**(`data/facts/board-fact-packs.json`),在
official-source pipeline 中全部 `fields_missing_source=0`。资料不完整的 topic 如实返回
`unknown_with_sources` 或 `needs_source_ingestion`,绝不编造引脚。近期加深的板子:

- **T-Display-S3** —— 8-bit 并口(i8080)屏总线已按引脚粒度建模
  (`display.d0`–`display.d7`、`wr`、`rd`、`cs`、`dc`、`reset`、`backlight`),runtime
  知道这些引脚已被屏占用,之后才建议接别的线。
- **T-Deck** —— LoRa(`lora.cs`/`busy`/`rst`/`dio1`)、键盘中断、轨迹球中断、共享 SPI
  总线、SD 卡、显示引脚,均来自官方 `utilities.h`。
- **T-CameraPlus-S3** —— 外设矩阵已存在,但对仍需查官方产品源才能定的话题(如存储)
  如实返回 `unknown_with_sources`。

被识别但超出范围的 LilyGO 产品(如 RP2040 等非 ESP32 板)会给出显式的支持边界一行,而
不是注入空胶囊。普通非 LilyGO prompt 什么都不注入。

## 安装

把仓库给 agent 让它安装,或在 Git 与 Node.js 就绪后手动装一次:

```bash
git clone https://github.com/Xinyuan-LilyGO/lilygo-skills.git
cd lilygo-skills
node install.js --all --dry-run     # 预览写入
node install.js --all               # 安装 + 自测
lilygo-skills doctor --json         # 确认注入链路
```

- **Node.js** 是运行 `install.js`、挂载 Skill 的必需项。
- **Rust/Cargo** 推荐用于完整动态 runtime。缺失时安装器仍以 **mount-only** 挂载;用
  `--build` 在同一步编译 CLI,或用 `--prebuilt-only` 安装预编译 runtime(无需 Rust):

  ```bash
  node install.js --all --prebuilt-only && lilygo-skills doctor --json
  ```

安装器把 runtime 根写到 `~/.claude/lilygo-skills/` 和 `~/.codex/lilygo-skills/`,把 router
Skill 装到 `~/.claude/skills/lilygo-skills/SKILL.md`,向 `~/.claude/settings.json` 幂等合并
可选的 `UserPromptSubmit` hook,并向 `~/.codex/AGENTS.md` 追加一段带标记的小节。若
`settings.json` 不是合法 JSON,安装器会明确报错并打印手工片段,不碰这个文件。

宿主工具链(Arduino CLI、PlatformIO、ESP-IDF、esp-rs、board core、串口工具、
LoRa/GNSS 库)保持显式——都通过 `setup plan` 和用户授权步骤处理,绝不隐式安装。

## 质量门禁

发布 runtime 改动前,跑门禁:

```bash
cargo test --workspace                              # 154 个测试
node eval/coverage-gate.js                          # 注入胶囊覆盖率 >= 基线
node pipeline/verify-auto-mapping.js                # 抽取引脚 == 官方宏
node pipeline/verify-source-authority.js            # 每条事实都保留有排序的官方来源
bash scripts/ci-gate.sh                             # 34 道确定性门禁
cargo fmt --check ; cargo clippy --workspace --all-targets -- -D warnings
```

`coverage-gate.js` 针对每个 eval prompt 直接给模型*真实收到的注入胶囊*按期望事实打分
(当前 **62 条期望事实里覆盖 55 条,88.7%,20 个诚实标记全部在场**,基线只上不下)。因此
裁剪脚手架永远无法悄悄让模型实际看到的事实退化。`ci-gate.sh` 跑 34 道确定性检查
(byte-for-byte 胶囊 fixture、板级三问测试、scorecard grading、install/doctor smoke、
source pipeline)。

## 如何加一块板

板级事实一律从官方 LilyGO 来源 ingest,绝不手敲:

1. 在 `pipeline/source-manifest.json` 加一条官方源条目(板 id、官方头文件/示例原始 URL、
   line range、topic、`authority_rank`,以及宏→事实 key 映射)。若该板用了共享表还不认识
   的宏名,扩展 `pipeline/pin-naming-conventions.json`。
2. 先 dry-run 再写入:

   ```bash
   node pipeline/ingest-from-manifest.js --board <board-id> --json
   node pipeline/ingest-from-manifest.js --board <board-id> --write
   ```

3. 保持门禁全绿(auto-mapping、source-authority、all-boards pipeline、coverage gate、
   `scripts/ci-gate.sh`)。新的期望事实进 `eval/tasks.json`;真实注入覆盖更多时用
   `node eval/coverage-gate.js --update-baseline` 把基线**上调**(绝不下调)。

## 文档

| 主题 | English | 中文 |
|------|---------|------|
| 总览 | [README.md](README.md) | [README.zh-CN.md](README.zh-CN.md) |
| 架构 | [ARCHITECTURE.md](ARCHITECTURE.md) | [ARCHITECTURE.zh-CN.md](ARCHITECTURE.zh-CN.md) |
| 上下文层 | [docs/CONTEXT_LAYER.md](docs/CONTEXT_LAYER.md) | [docs/CONTEXT_LAYER.zh-CN.md](docs/CONTEXT_LAYER.zh-CN.md) |
| Skill 生成 | [docs/SKILL_GENERATION.md](docs/SKILL_GENERATION.md) | [docs/SKILL_GENERATION.zh-CN.md](docs/SKILL_GENERATION.zh-CN.md) |
| 板级事实 | [docs/BOARD_FACTS.md](docs/BOARD_FACTS.md) | [docs/BOARD_FACTS.zh-CN.md](docs/BOARD_FACTS.zh-CN.md) |
| Source recovery | [docs/SOURCE_RECOVERY.md](docs/SOURCE_RECOVERY.md) | [docs/SOURCE_RECOVERY.zh-CN.md](docs/SOURCE_RECOVERY.zh-CN.md) |
| Action routing | [docs/ACTION_ROUTING.md](docs/ACTION_ROUTING.md) | [docs/ACTION_ROUTING.zh-CN.md](docs/ACTION_ROUTING.zh-CN.md) |
| 验证等级 | [docs/VERIFICATION_LEVELS.md](docs/VERIFICATION_LEVELS.md) | [docs/VERIFICATION_LEVELS.zh-CN.md](docs/VERIFICATION_LEVELS.zh-CN.md) |
| 新增板子 | [docs/CONTRIBUTING_BOARDS.md](docs/CONTRIBUTING_BOARDS.md) | [docs/CONTRIBUTING_BOARDS.zh-CN.md](docs/CONTRIBUTING_BOARDS.zh-CN.md) |

公开仓库就是 runtime source:CLI、安装器、router Skill、source model、数据表、references、
schema 和发布门禁。本 README 是主要使用文档。
