# {{title}}: {{display_name}}

{{display_name}} is a generated context snapshot for a currently supported LilyGO target.

- Generation Contract: templates/skills/board.md
- MCU: `{{mcu}}`.
- Frameworks: {{frameworks}}.
- Peripherals: {{peripherals}}.
{{family}}- Source pointer: `{{repo_url}}`.
- Wiki pointer: `{{wiki_url}}`.
- Source status: `{{source_status}}`.
- Pair framework-specific prompts with `fw-arduino`, `fw-esp-idf`, `fw-rust`, or `fw-platformio`.
- Pair LVGL prompts with `fw-lvgl` and `debug-lvgl-loop`; pair OTA prompts with `app-ota` and `debug-flash-serial` as recipe/evidence context only.
- Keep claims at context injection unless `verify-hardware` or an evidence smoke reaches V4/V5.
{{source_urls}}{{peripheral_matrix}}{{demo_refs}}{{warnings}}
