# 架构

English version: [ARCHITECTURE.md](ARCHITECTURE.md)。相关文档:
[上下文层](docs/CONTEXT_LAYER.zh-CN.md) /
[EN](docs/CONTEXT_LAYER.md),[Skill 生成](docs/SKILL_GENERATION.zh-CN.md)
/ [EN](docs/SKILL_GENERATION.md),[板级事实](docs/BOARD_FACTS.zh-CN.md) /
[EN](docs/BOARD_FACTS.md),[Source recovery](docs/SOURCE_RECOVERY.zh-CN.md) /
[EN](docs/SOURCE_RECOVERY.md),[Action routing](docs/ACTION_ROUTING.zh-CN.md) /
[EN](docs/ACTION_ROUTING.md),以及
[验证等级](docs/VERIFICATION_LEVELS.zh-CN.md) /
[EN](docs/VERIFICATION_LEVELS.md)。

lilygo-skills 是面向 LilyGO 开发 Agent 的一个 meta Skill 加一个薄 **Node context
kernel**。它把自然语言的板级任务变成一个紧凑、带来源的上下文胶囊,再暴露确定性命令去
拉取精确引脚并对上游复证。

runtime 把**上下文就绪**与**硬件验证**分开。内核证明 Agent 收到了正确的板级事实,每条
都能追溯到官方上游某一行;至于用户固件是否真的能 build、烧录、出像素或拿到 RF/GNSS
定位,那是内核从不声称的另一条证据轨。

## 两个部分

- **meta Skill** —— `skills/lilygo-router/SKILL.md`,唯一提交的 Skill。它是操作文档:
  查询协议、调试循环、诚实规则,全部以散文承载,配 `skills/lilygo-router/guides/` 下的
  聚焦 how-to 引导。它拥有 Agent 遵循的行为。
- **Node context kernel** —— `bin/` 下的 JS 薄核。它读提交的数据模型(`data/**`),回答
  一小组稳定命令。它以 `.mjs` 发布,跑在每个宿主本就有的 Node 上;安装器把 `bin/**` 与
  `data/**` 作为一个自包含 runtime 一起复制,数据永远随 reader 同行。

内核绝不内联硬件值。它返回的每个引脚、总线、电源轨都来自带官方 URL、行范围与 sha256 的
提交 fact pack。

## 命令面

```text
lilygo-skills context [--project <dir>] --json "<prompt>"              CWD -> 板 -> 胶囊
lilygo-skills source query --board <id> --topic <topic> --json         某 topic 的带来源事实
lilygo-skills verify sources --board <id> [--topic <t>] --json         在线复证(OK / DRIFT / UNREACHABLE)
lilygo-skills doctor --json                                            数据完整性自测
lilygo-skills hook <claude|codex>                                      推送厚板级胶囊
```

- **`context`** 判断 prompt 是否是 LilyGO 工作、涉及哪块板子(先读
  `.lilygo-skills/project.json`,否则嗅探 `platformio.ini`、`sdkconfig`、`*.ino`,再否则
  用 prompt 关键词),返回一个小胶囊:`board`、`board_source`、`skills`、
  `verification_level`,以及带后续 `source query` 命令的紧凑 `context` 串。被识别但超范围
  的产品(非 ESP32 的 LilyGO 板)给出支持边界一行;非 LilyGO prompt 则 no-op。
- **`source query`** 直接从 fact pack 返回按 topic 圈定的事实——每条带其 source ref
  (URL + 行范围 + sha256)——外加 completeness 信号(`complete` / `partial` /
  `needs_source_ingestion` / `unsupported`);当某值尚未 ingest 时给出发现提示而非猜测。
- **`verify sources`** 重新抓取每条 line-anchored 事实的原始来源,重算 sha256,归类
  `OK` / `DRIFT` / `UNREACHABLE`。断网或被限流是优雅降级(`UNREACHABLE`,仍 exit 0);
  只有真实内容漂移才失败。
- **`doctor`** 检查数据模型在场且内部一致(板子注册表、fact pack、V3 证据覆盖、嗅探
  规则),并跑一次样本注入。
- **`hook`** 是推送边界。Claude Code 上安装的 `UserPromptSubmit` hook
  (`node <root>/bin/hook.mjs claude`)从 stdin 读 prompt JSON,输出
  `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"..."}}`
  (信封或空;诊断走 stderr;fail-open exit 0)。Codex 通过 `AGENTS.md` 标记小节里的
  `hook codex` 消费诊断信封。

## Push / Pull 边界

内核先播种上下文,再指向其余:

- **Push(`hook`)。** 厚胶囊内联板子的关键 pin/bus/driver 事实——放得进字节预算的那个
  子集——外加祈使的 pull-before-claim 引导。它是指针,不是完整引脚图。
- **Pull(`source query`)。** 任何未内联的具体 pin/bus/setting **必须**用 `source query`
  拉取并按返回的 source ref 作答。引脚不在胶囊里就是“去拉取”,绝不是“从已显示的推断”。

这个完整闭环——推关键子集、带引用拉其余——正是让答案正确且可验证的原因。对不完整胶囊
做纯推送式阅读是不安全的,所以 `SKILL.md` 和 guides 把 pull 规则写死。

## 数据模型与供应链

板级事实一律从官方 LilyGO 与厂商来源 ingest,绝不手敲。内核只读提交的 JSON:

- `data/boards.json` —— 板子/产品注册表。
- `data/facts/board-fact-packs.json` —— 每板的 pin/bus/expander/connector/peripheral
  表,每条事实带其 source ref 与证据等级。
- `data/facts/prompt-keywords.json`、`data/facts/topic-fields.json` —— 驱动选择的
  关键词表与 topic 字段表。
- `data/sniff-rules.json` —— 用于自动认板的项目文件匹配器。

ingest 与复证管线是一组 Node 脚本,在作者时运行,不属于日常 CLI:

```text
官方源(pipeline/source-manifest.json:原始 URL + 行范围 + 宏->fact 映射)
  -> node pipeline/ingest-from-manifest.js --board <id> --write
  -> data/facts/board-fact-packs.json(值带 URL + 行范围 + sha256)
  -> node pipeline/verify-auto-mapping.js       抽取引脚 == 官方宏
  -> node pipeline/verify-source-authority.js   每条事实都保留有排序的官方来源
  -> node eval/verify-provenance.js             每条事实都带 url + hash provenance
```

## 来源权威

来源权威是有序的,不是拍平的:

1. 官方代码、头文件、示例、manifest 与板子仓库。
2. 官方 LilyGO 硬件文档。
3. `https://github.com/Xinyuan-LilyGO/documentation`,LilyGO wiki 内容背后的带版本源。
4. `wiki.lilygo.cc` 兜底页。
5. 项目参考模式,作实现/调试提示用。

reference hint 告诉 Agent 读什么;它们不覆盖 source fact。内核证不出确切值时返回
`unknown_with_sources` 或 `needs_source_ingestion`,而不是猜。

## 板子与项目身份

板子身份按优先级解析:显式 prompt 文本,然后项目本地 `.lilygo-skills/project.json`,
然后项目文件嗅探,再然后当 prompt 无关时 no-op。prompt 事实永远胜出。模棱两可的项目
证据(一个 `platformio.ini` 里两块板)返回无板而非猜测。`.lilygo-skills/` 下的项目文件
让不同固件目录带不同默认;私有或机器本地状态(`local.json`、`evidence/`)绝不提交、也
绝不注入公开 prompt 上下文。

## 验证边界

内核在 source/context 维度验证到 **V3**:routing、hook 输出、source fact、completeness
状态、在线复证与 provenance 都由 gate 覆盖。硬件执行是另一条 task-scoped 证据轨——build
产物、烧录成功、串口日志、OTA 传输、显示像素与外设行为,只有在对应 V4/V5 产物存在时才
从“上下文可用”变为“已验证”。见
[docs/VERIFICATION_LEVELS.zh-CN.md](docs/VERIFICATION_LEVELS.zh-CN.md)。

诚实标记是可机检的,不只是散文:`hardware_verified` 与 `evidence_boundary` 是 gate 能
打分的注入值。

## 质量门禁

两个门禁族守护 runtime 改动,二者都与固件语言无关,都在 CI 跑:

- **JS 核心** —— `npm test`(单测 + CLI 契约 + hook 值对齐 against 冻结参照 + CJK
  路由)、`npx tsc --noEmit`、`doctor --json`、在线 `verify sources`。
- **数据 / provenance** —— official-source pipeline(gold + all boards)、gold fact-pack
  diff、板级三问覆盖、provenance 复证、注入胶囊 coverage gate(棘轮,只上不下)、
  scorecard grading。

`scripts/ci-gate.sh` 聚合两族,外加 doc/surface 卫生 smoke 与 install -> hook 集成
smoke,让 HEAD 失败的检查绝不能搭上绿色管线。

## Runtime 物化

`install.js` 拥有 runtime 根,把 `bin/**` 与 `data/**` 镜像到那里,让上一版的过期文件
无法在安装后存活。用户拥有的宿主文件——Claude `settings.json` 与 Codex `AGENTS.md`——
只做合并、只在标记范围内:安装器只替换自己的 hook 条目,且拒绝碰非法 JSON 或不平衡的
标记。宿主工具链(Arduino CLI、PlatformIO、ESP-IDF、esp-rs、串口与 LoRa 工具)保持
显式,绝不隐式安装。

## 文件地图

```text
bin/                          Node context kernel(dispatcher + hook + data reader)
data/boards.json              板子/产品注册表
data/facts/                   板级 fact pack + 关键词/topic 表
data/sniff-rules.json         认板用的项目文件匹配器
data/references/source-intake 公开 source-intake 缓存与 manifest
skills/lilygo-router/         提交的 meta router SKILL.md + guides + references
pipeline/                     Node ingest + 复证管线
eval/                         契约/对齐测试、coverage + provenance gate、scorecard
scripts/                      确定性门禁(ci-gate.sh)+ 卫生/集成 smoke
docs/                         人类架构与贡献者文档
```
