# Skills Directory — meta-only source tree

This directory is **meta-only**. The only committed Skill is the meta router
`lilygo-router/SKILL.md`, which tells an agent how to ask the `lilygo-skills`
CLI for compact context, generation, setup, update, and evidence actions.
Static reference docs under `references/` are support material, not routed
Skills.

Generated board, series, framework, tool, peripheral, chip, feature, debug,
app/recipe, and playbook skills are **not committed here**. They are runtime
artifacts produced from the source model (`data/**`) on demand:

```bash
# Materialize every runtime skill into a generated cache (never the source tree):
lilygo-skills generate skills --out .tmp/generated-skills --json

# Verify the generated cache is complete and honest about evidence levels:
lilygo-skills verify --generated-root .tmp/generated-skills --json
```

The installer (`node install.js`) generates skills into the selected Codex/Claude
install root; `project init` writes a project-local cache. Route and hook calls
stay no-write: they may report that a generated skill is missing and include a
compact generation/update command, but never write skills implicitly.

Source-backed facts live under `data/`, route triggers live in
`index/routes.json`, reference practice skills live under
`data/skills/reference/`, playbooks live under `data/playbooks/`, and public
references live in official repos, docs, examples, datasheets, or project-local
`.lilygo-skills/references.json`.

Generated Skill file shapes live under `../templates/skills/`. The CLI uses
those templates for board, peripheral/chip/feature, and playbook output, and
copies both templates and `references/` into generated/install roots.

Why meta-only: committing generated snapshots turns the repository into an opaque
dump of hand-written skill text. Keeping only the source model plus a
deterministic generator keeps the release auditable and the skills reproducible.
