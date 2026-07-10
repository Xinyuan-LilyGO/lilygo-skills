//! Data-driven trigger-rule engine shared by the recipe and playbook selectors.
//!
//! The recipe/playbook *definitions* already live in JSON (`data/recipes`,
//! `data/playbooks`). This module moves the remaining hardcoded piece — the
//! per-id trigger rules that decide which ids fire for a prompt — into JSON as
//! well, so the Rust side is a thin generic evaluator with no per-id arms.
//!
//! A rule fires when its boolean `when` condition holds; conditions are a small
//! DSL over the normalized prompt, precomputed named flags, and named string
//! lists (route peripherals/chips/features/skills). Selection order is
//! preserved, so a later rule may reference an earlier rule's outcome via
//! `selected`.

use crate::text_match::contains_word;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// One boolean predicate in the trigger DSL. Deserialized from JSON with an
/// internal tag so each variant reads as a compact object, e.g.
/// `{ "keyword": ["lora", "gnss"] }` or `{ "flag": "has_board" }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cond {
    /// All sub-conditions hold (AND). Empty vec is vacuously true.
    All(Vec<Cond>),
    /// Any sub-condition holds (OR). Empty vec is false.
    Any(Vec<Cond>),
    /// Negates the inner condition.
    Not(Box<Cond>),
    /// Any needle is a word-boundary match in the normalized prompt.
    Keyword(Vec<String>),
    /// A precomputed named boolean flag is true.
    Flag(String),
    /// A named string list is non-empty.
    ListNonEmpty(String),
    /// A named string list contains an exact value.
    ListContains { list: String, value: String },
    /// A named string list contains any of the values.
    ListContainsAny { list: String, values: Vec<String> },
    /// A named string list has any element starting with the prefix.
    ListAnyPrefix { list: String, prefix: String },
    /// An id was already selected by an earlier rule.
    Selected(String),
}

/// A single trigger rule: when `when` holds, every id in `insert` is selected.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    /// Ids to select when the rule fires.
    pub insert: Vec<String>,
    /// The condition that fires the rule.
    pub when: Cond,
}

/// A named flag whose value is computed from a condition before rules run.
#[derive(Debug, Clone, Deserialize)]
pub struct FlagDef {
    pub name: String,
    pub when: Cond,
}

/// The full trigger table for one module, distilled from the old Rust arms.
#[derive(Debug, Clone, Deserialize)]
pub struct SelectionConfig {
    pub schema_version: u32,
    /// Replace `_` with `-` during prompt normalization (playbooks do; recipes
    /// do not).
    #[serde(default)]
    pub replace_underscore: bool,
    /// Flags defined in data, evaluated top-to-bottom before the rules. A later
    /// flag may reference an earlier one.
    #[serde(default)]
    pub flags: Vec<FlagDef>,
    /// Optional gate: when present and false, selection yields nothing.
    #[serde(default)]
    pub gate: Option<Cond>,
    /// Trigger rules, evaluated in order.
    pub rules: Vec<Rule>,
    /// Optional fixed output order for the selected ids.
    #[serde(default)]
    pub order: Vec<String>,
}

/// Inputs the engine evaluates a config against.
pub struct SelectionInput<'a> {
    /// Normalized prompt (already lowercased by the caller).
    pub prompt: String,
    /// Named booleans injected by the caller (e.g. `has_board`).
    pub flags: BTreeMap<&'a str, bool>,
    /// Named string lists referenced by list conditions.
    pub lists: BTreeMap<&'a str, Vec<String>>,
}

struct EvalCtx {
    normalized: String,
    flags: BTreeMap<String, bool>,
    lists: BTreeMap<String, Vec<String>>,
    selected: BTreeSet<String>,
}

impl EvalCtx {
    fn eval(&self, cond: &Cond) -> bool {
        match cond {
            Cond::All(conds) => conds.iter().all(|c| self.eval(c)),
            Cond::Any(conds) => conds.iter().any(|c| self.eval(c)),
            Cond::Not(inner) => !self.eval(inner),
            Cond::Keyword(needles) => needles
                .iter()
                .any(|needle| contains_word(&self.normalized, needle)),
            Cond::Flag(name) => self.flags.get(name).copied().unwrap_or(false),
            Cond::ListNonEmpty(list) => self.lists.get(list).is_some_and(|l| !l.is_empty()),
            Cond::ListContains { list, value } => self
                .lists
                .get(list)
                .is_some_and(|l| l.iter().any(|item| item == value)),
            Cond::ListContainsAny { list, values } => self
                .lists
                .get(list)
                .is_some_and(|l| l.iter().any(|item| values.contains(item))),
            Cond::ListAnyPrefix { list, prefix } => self
                .lists
                .get(list)
                .is_some_and(|l| l.iter().any(|item| item.starts_with(prefix))),
            Cond::Selected(id) => self.selected.contains(id),
        }
    }
}

/// Evaluate a trigger config against an input, returning the selected ids in the
/// config's `order` when set, otherwise sorted (BTreeSet iteration).
pub fn evaluate(config: &SelectionConfig, input: SelectionInput) -> Vec<String> {
    assert_eq!(
        config.schema_version, 1,
        "unsupported selection schema_version {}",
        config.schema_version
    );
    let normalized = if config.replace_underscore {
        input.prompt.replace('_', "-")
    } else {
        input.prompt
    };
    let mut ctx = EvalCtx {
        normalized,
        flags: input
            .flags
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        lists: input
            .lists
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        selected: BTreeSet::new(),
    };

    for flag in &config.flags {
        let value = ctx.eval(&flag.when);
        ctx.flags.insert(flag.name.clone(), value);
    }

    if let Some(gate) = &config.gate
        && !ctx.eval(gate)
    {
        return Vec::new();
    }

    for rule in &config.rules {
        if ctx.eval(&rule.when) {
            for id in &rule.insert {
                ctx.selected.insert(id.clone());
            }
        }
    }

    if config.order.is_empty() {
        ctx.selected.into_iter().collect()
    } else {
        config
            .order
            .iter()
            .filter(|id| ctx.selected.contains(*id))
            .cloned()
            .collect()
    }
}
