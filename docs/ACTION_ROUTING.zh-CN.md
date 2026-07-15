# Action Routing

English version: [ACTION_ROUTING.md](ACTION_ROUTING.md)。

Action routing 的目标是让 LilyGO 上下文保持紧凑，同时把“下一步该看什么、该跑什么”
放到 Agent 面前。在 JS thin core 里，这由两个作用在同一份 board capsule 上的命令完成：

- `context` 解析板子并返回一份 thin capsule(≤~1KB)，写明板子、验证边界，以及拉取
  精确事实用的 `source query` 命令。
- `hook` 把 thick board capsule 注入 `UserPromptSubmit` 上下文，内联 top source-backed
  facts，并附带强制的 pull-before-claim 规则。

两个 surface 都把 Agent 指向精确事实，而不是把整段 source 或生成 skill 正文塞进 prompt。

## Capsule 暴露什么

检测到板子后，capsule 会暴露：

- 解析出的 `board` id 以及检测方式(`keyword` 或项目文件)；
- 验证边界(`context-injection`、`hardware_verified=false`、`evidence_boundary=V3`)；
- `source query` 命令,以及带事实的 topic,例如 `pinout`、`display`、`i2c`、`spi`、
  `power`、`lora`、`gnss`、`touch`；
- 对 thick capsule,还会内联 top-ranked facts(chip、bus、driver 和 power 引脚),
  这样常见问题无需第二次调用即可回答。

纯事实查询保持紧凑。用户只问“哪些引脚或总线被占用”时，capsule 返回板级 facts 和
`source query` 命令，不注入 build、flash、serial 或 OTA 动作。Thin core 不会产出偏
mutation 的动作；执行交给 Agent 和用户自己的工具链。

## Pull Before Claim

Thick capsule 带一条硬规则：对任何还没内联在 capsule 里的具体引脚、总线、地址或设置，
Agent 必须先对相关 topic 跑 `source query`，并引用返回的官方 `url + line_range + sha256`。
既没内联、也无法通过 `source query` 恢复的值，必须报告为 unknown，而不是猜测。

`verify sources` 会把这些事实对记录的 source 重新证明，返回 `OK`、`DRIFT` 或
`UNREACHABLE`，这样过期的 capsule 会在 Agent 依赖它之前被发现。

## 自然语言使用

用户可以直接说：

```text
我在用 LilyGO T-Display-S3，PlatformIO Arduino。
先点亮第一个 TFT 画面，然后接一个 I2C 传感器。
```

Agent 应该先检查 capsule：

```bash
lilygo-skills context --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"
```

在写引脚前，再拉取精确的 IO/I2C 事实：

```bash
lilygo-skills source query --board board-t-display-s3 --topic i2c --json
lilygo-skills source query --board board-t-display-s3 --topic pinout --json
```

对固件仓库，传入项目目录，让板子检测用 build config 和源码,而不是只看 prompt：

```bash
lilygo-skills context --project . --json "bring up the display"
```

## Project Detection

`context --project <dir>` 会在固件仓库中嗅探 board-identifying token，来源是
`platformio.ini`、build config 和有限的一组源码文件。项目文件证据被视为比 prompt
keyword 更具体，因此两者都存在时,仓库内的板子胜出。检测是只读的，不写任何项目本地状态。

## 健康检查

安装后，或者发现上下文注入没有触发时，运行：

```bash
lilygo-skills doctor --json
```

`doctor` 会验证当前安装的 runtime 数据模型：检查 runtime 数据文件是否齐全、board
registry 和 fact packs 是否匹配、每条 fact 是否带 V3 evidence，以及 sniff matchers
是否可加载。它还会返回一份 `sample_injection` capsule，方便端到端确认注入链路。
data-integrity 问题会 fail closed；之后 Agent 在硬件上运行的内容，通过该任务自己的
build/flash/serial evidence 验证。
