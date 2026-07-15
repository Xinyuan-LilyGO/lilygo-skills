# Source Recovery

English version: [SOURCE_RECOVERY.md](SOURCE_RECOVERY.md).

Source recovery 是用户让 Agent 为 LilyGO 板子实现、调试、迁移固件功能时使用的
资料恢复路径。Skill 默认不把所有资料都塞进 prompt，但会在需要写代码前暴露最近的
官方 demo、源码 header、关键事实和扩展命令。

## 会暴露什么

比如用户说：

```text
我在用 LilyGO T-Display-S3，PlatformIO Arduino 项目。
帮我接一个 I2C 传感器，并把读数显示到屏幕上。
```

runtime 应该暴露：

- 最近的官方 demo，例如 `examples/tft/tft.ino`。
- 板子自己的源码 header，例如 TFT_eSPI Setup206 和
  `examples/factory/pin_config.h`。
- 关键事实，例如 `PIN_IIC_SDA=GPIO18` 和 `PIN_IIC_SCL=GPIO17`。
- 恢复查询命令，例如 `source query --board board-t-display-s3 --topic io`。
- 重新证明命令，例如 `verify sources --board board-t-display-s3`。

thin `context` capsule 只拿到紧凑版本；更完整的信息仍然通过 `source query`、
`verify sources` 和生成出来的 board skill 展开。

## 如何使用

用户不需要先说 CLI 命令，直接自然语言描述即可：

```text
我用 T-Display-S3 和 PlatformIO Arduino。帮我做一个 TFT_eSPI 屏幕 demo，
同时读取一个 I2C 传感器。
```

Agent 可以读取已注入的紧凑上下文；需要更多细节时，再展开：

```bash
lilygo-skills context --json "T-Display-S3 PlatformIO Arduino TFT_eSPI I2C sensor screen"
lilygo-skills source query --board board-t-display-s3 --topic io --json
lilygo-skills verify sources --board board-t-display-s3 --topic io --json
```

这样精确引脚和 demo 路径来自官方源码，而不是模型记忆。

## 生成的板级 Skill

生成的 board skill 会包含紧凑的 `Source-Backed Board Facts` 小节。
以 T-Display-S3 为例，这里会包含官方 I2C 引脚、触摸引脚、显示事实和 demo
引用。源码仓库仍然不提交生成快照；它们由安装器和 official-source pipeline 物化。
直接查看事实：

```bash
lilygo-skills source query --board board-t-display-s3 --topic i2c --json
lilygo-skills source query --board board-t-display-s3 --topic display --json
```

## 验证

修改板子检测、fact pack、生成 skill 或 hook 渲染时，运行 gates：

```bash
npx tsc --noEmit
npm test
bash scripts/ci-gate.sh
```

测试套件覆盖 `context`、`hook`、`source query` 和 `verify sources` surface，
以及生成的 `board-t-display-s3` 事实。

Source recovery 属于 V3 source/context 证据。它能指导实现和调试，但不能等同于
硬件成功；真正硬件成功仍然需要 build、flash、serial、display、OTA 或其他设备证据。
