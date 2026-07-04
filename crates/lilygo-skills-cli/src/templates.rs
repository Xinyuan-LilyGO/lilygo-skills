//! Minimal template rendering for generated Skill contracts.
//!
//! Templates are checked into `templates/skills/` so generated runtime markdown
//! has a public source contract instead of living only in Rust format strings.

pub(crate) fn render_template(template: &str, values: &[(&str, String)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_replaces_named_placeholders() {
        let rendered = render_template(
            "A {{one}} {{two}}",
            &[("one", "board".to_string()), ("two", "context".to_string())],
        );
        assert_eq!(rendered, "A board context");
    }
}
