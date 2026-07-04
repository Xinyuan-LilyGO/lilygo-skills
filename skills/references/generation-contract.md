# Generation Contract

The source tree commits the meta Skill, source data, references, templates, and
runtime code. Dynamic board, chip, peripheral, framework, playbook, debug, app,
and recipe Skill files are generated into install/cache/output roots.

## Source Inputs

- `index/routes.json`
- `data/boards.json`
- `data/facts/**`
- `data/peripherals/**`
- `data/recipes/**`
- `data/playbooks/**`
- `data/skills/reference/**`
- `skills/lilygo-router/SKILL.md`
- `skills/references/**`
- `templates/skills/**`

## Generated Outputs

```text
<out>/skills/<skill-id>/SKILL.md
<out>/skills/references/*.md
<out>/templates/skills/*.md
<out>/index/routes.json
```

Directories under `skills/` count as generated skills only when they contain a
`SKILL.md` file. Support docs under `skills/references/` are not routed skills.

## Required Marker

Template-rendered runtime skills include:

```text
Generation Contract: templates/skills/<kind>.md
```

Smoke tests use this marker to verify that public templates are the actual
generation path.
