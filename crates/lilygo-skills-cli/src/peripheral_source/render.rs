//! Renders peripheral, chip, and feature skills as navigation layers over
//! structured source packs rather than hand-maintained board facts.
use super::*;
use crate::templates::render_template;

const PERIPHERAL_TEMPLATE: &str = include_str!("../../../../templates/skills/peripheral.md");

pub(crate) fn render_generated_skill(skill: &Skill, index: &PeripheralSourcePackIndex) -> String {
    let packs = source_packs_for_skill(skill, index);
    let pack_lines = packs.iter().map(render_pack_line).collect::<String>();
    let sources = packs
        .iter()
        .flat_map(|pack| pack.sources.iter())
        .map(|source| {
            format!(
                "- `{}` rank {} [{} stale={}]: {}\n",
                source.kind, source.authority_rank, source.evidence_level, source.stale, source.url
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let frameworks = packs
        .iter()
        .flat_map(|pack| pack.framework_refs.iter())
        .map(|reference| {
            format!(
                "- `{}` `{}`: `{}` [{} stale={}] {}\n",
                reference.framework,
                reference.target,
                reference.path,
                reference.evidence_level,
                reference.stale,
                reference.source_url
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    render_template(
        PERIPHERAL_TEMPLATE,
        &[
            ("id", skill.id.clone()),
            ("summary", skill.summary.clone()),
            ("kind", format!("{:?}", skill.kind)),
            ("source_packs", pack_lines),
            ("ranked_sources", sources),
            (
                "framework_refs",
                if frameworks.is_empty() {
                    "- No framework demo reference is currently source-packed.\n".to_string()
                } else {
                    frameworks
                },
            ),
        ],
    )
}

pub(crate) fn render_pack_line(pack: &&PeripheralSourcePack) -> String {
    format!(
        "- `{}`: board `{}`, peripheral `{}`, chip `{}`, features [{}].\n",
        pack.id,
        pack.board_id,
        pack.peripheral,
        pack.chip,
        pack.feature_refs
            .iter()
            .map(|feature| feature.feature.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn source_packs_for_skill<'a>(
    skill: &Skill,
    index: &'a PeripheralSourcePackIndex,
) -> Vec<&'a PeripheralSourcePack> {
    index
        .packs
        .iter()
        .filter(|pack| skill_matches_pack(skill, pack))
        .collect()
}

pub(crate) fn skill_matches_pack(skill: &Skill, pack: &PeripheralSourcePack) -> bool {
    skill.id == peripheral_skill_id(pack)
        || skill.id == chip_skill_id(pack)
        || pack
            .feature_refs
            .iter()
            .any(|feature| skill.id == feature_skill_id(feature))
}

pub(crate) fn ensure_source_route_fixtures(registry: &mut Registry) {
    add_fixture(
        registry,
        "t-watch-ultra-raise-to-wake",
        "T-Watch Ultra Arduino IMU 抬腕检测怎么做",
        &[
            "board-t-watch-ultra",
            "periph-imu",
            "chip-bhi260ap",
            "fw-arduino",
            "feature-raise-to-wake",
        ],
    );
    add_fixture(
        registry,
        "t-watch-ultra-bhi260ap-source",
        "T-Watch Ultra BHI260AP driver source Arduino example",
        &[
            "board-t-watch-ultra",
            "periph-imu",
            "chip-bhi260ap",
            "fw-arduino",
        ],
    );
}

pub(crate) fn add_fixture(registry: &mut Registry, id: &str, prompt: &str, skills: &[&str]) {
    if registry
        .route_fixtures
        .iter()
        .any(|fixture| fixture.id == id)
    {
        return;
    }
    registry.route_fixtures.push(RouteFixture {
        id: id.to_string(),
        prompt: prompt.to_string(),
        expect_decision: "inject".to_string(),
        expect_skills: skills.iter().map(|skill| skill.to_string()).collect(),
    });
}
