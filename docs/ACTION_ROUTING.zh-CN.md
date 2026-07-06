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
- 来自 `.lilygo-skills/skills/index.json` 的项目本地 custom skill hint；
- 通过 `doctor --json` 自检安装链路。

纯事实查询仍然保持紧凑。用户只问“哪些引脚或总线被占用”时，runtime 返回 fact table
和 source-query 命令，不注入 build、flash、serial、OTA 或 demo 动作。

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

`doctor` 会验证 runtime data、生成 skills、route 样例、no-op 样例，以及已存在的
宿主安装文件。它不证明硬件行为成功。
