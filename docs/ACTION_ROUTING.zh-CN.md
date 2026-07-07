# Action Routing

Action routing 的目标是让 LilyGO 上下文保持紧凑，同时把“下一步该看什么、该跑什么”
放到 Agent 面前。

当 prompt 是实现或调试任务时，`goal plan` 和安装态 hook context 可以暴露：

- 最小相关官方 demo，例如 T-Display-S3 第一次点亮屏幕优先给
  `examples/tft/tft.ino`，而不是完整 factory test；
- 精确 IO 和 bus facts 的 source query，包括 `io`、`i2c`、`spi`、`uart`、
  `i2s`、`gpio`；
- 带权限的 `next_actions`，区分只读 source lookup 和 build、flash、serial、
  network、OTA；
- `goal-plan-bridge`，一个只读 next action，让 Agent 在改代码或接触硬件前先查看
  `goal plan`；
- 来自 `.lilygo-skills/skills/index.json` 的项目本地 custom skill hint；
- 来自 `.lilygo-skills/ledger.json` 和 `.lilygo-skills/context-digest.json` 的项目
  ledger hint，用于这个仓库已经接收或验证过相同上下文的情况；
- 通过 `doctor --json` 自检安装链路。

纯事实查询仍然保持紧凑。用户只问“哪些引脚或总线被占用”时，runtime 返回 fact table
和 source-query 命令，不注入 build、flash、serial、OTA 或 demo 动作。

分类规则是有方向性的：

| Prompt 形态 | 路由行为 |
|-------------|----------|
| 纯查询：“which pins are used by the screen?”、“哪些引脚被屏幕占用了?” | 只读 capsule：只保留 facts 和 source-query 命令 |
| 实现/调试：“bring up the display”、“让屏幕先亮起来”、“debug the sensor” | goal bridge、精选 demo、playbook 和带权限标记的 next actions |
| 混合：“先查一下引脚，然后帮我点亮屏幕” | 实现/调试优先，但保留查询展开命令 |

“first”“minimal”“先”“最小”这类短词不会单独触发 demo。它们只有和显示/运行或
factory-test 意图一起出现时，才影响 demo 排名。

这也是 token budget 规则：默认 capsule 要告诉 Agent “从哪里继续展开”，而不是把
全部 source 或生成 Skill 正文直接塞进 prompt。需要更多细节时，再用 `source query`、
`index query`、项目生成 skills 或 `goal plan` 展开。

Project ledger hit 也遵循这个规则。命中 ledger 可以把重复 prompt 变短，说明哪些内容
曾经验证过或已经注入过，但不会让实现请求直接停止。用户明确说“重新运行”“重新验证”或
`re-verify` 时，会绕过紧凑 hit，并继续暴露完整 goal 路径。

## 自然语言使用

用户可以直接说：

```text
我在用 LilyGO T-Display-S3，PlatformIO Arduino。
先点亮第一个 TFT 画面，然后接一个 I2C 传感器。
```

Agent 应该先检查：

```bash
lilygo-skills goal plan --json "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen with I2C sensor"
```

返回的 capsule 应该把最小 TFT demo 排在第一，给出 IO/I2C 的 `source query`，
并把 build 或设备操作标成需要权限的下一步。

如果只是查询，同一块板子的输出应该更小：

```bash
lilygo-skills route --json "T-Display-S3 的 I2C 引脚和外设地址有哪些?"
```

这个输出应该保留 fact/source-query 上下文，并省略 demo、recipe 和偏 mutation 的动作。

如果用户明确要完整 factory bring-up，较大的 factory example 仍然可达：

```bash
lilygo-skills goal plan --json "T-Display-S3 run the full factory test"
```

预期行为不是“永远用最小 demo”，而是“第一次可见输出用最小 demo，全板诊断保留
factory example”。

对于重复项目工作，用户也可以说：

```text
这个项目的显示 bring-up 已经验证过了，之后除非我要求重新验证，否则上下文保持短一点。
```

Agent 应该只保存脱敏后的公开摘要和 evidence hash。之后的显示类 prompt 可以收到紧凑的
`previously_verified` hint，以及 `project ledger show`、`source query`、
`goal evidence` 等展开命令。

## Project Custom Skills

固件仓库可以添加本地操作模式，而不修改公开 LilyGO runtime：

```text
.lilygo-skills/
  skills/
    index.json
    project-lvgl-loop/
      SKILL.md
```

每个 custom skill id 必须以 `project-` 开头，路径必须是
`.lilygo-skills/skills/` 下的相对路径，并且不能包含私有路径、凭据、原始日志、
串口设备或本地网络值。Project skill 只是补充的项目实践；官方板级 facts、headers
和 examples 仍然是最高权威。

## 健康检查

安装后，或者发现上下文注入没有触发时，运行：

```bash
lilygo-skills doctor --json
```

检查某个安装 HOME：

```bash
lilygo-skills doctor --json --home "$HOME"
```

`doctor` 会验证 runtime data、生成 skills、route 样例、no-op 样例，以及当前检查的
HOME 下 Codex/Claude 的接线状态。缺少集成是 warning；LilyGO hook 存在但命令畸形是
failure。当两端 host runtime 都存在时，`doctor` 还会检查二进制或数据镜像是否不同；
漂移会报告 warning，并打印重跑安装命令。它不证明硬件行为成功。
