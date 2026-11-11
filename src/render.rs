//! Markdown for hovers and for the output file, in the layout of Emacs's
//! `*Agda information*` buffer. Agda code goes in ```agda fences, so Zed
//! highlights it with the Agda grammar in hovers and in Markdown previews.

use crate::metas;
use crate::protocol::{DisplayInfo, GoalInfo, OutputConstraint, TypeAux, message_text};
use serde_json::Value;

const RULE: &str = "────────────────────────────────────────";

/// Goal type and context, as `Cmd_goal_type_context` returns them, with the
/// type of the goal's text as `Have:` when `Cmd_goal_type_context_infer`
/// asked for it, as in Emacs.
pub fn goal(id: u32, info: &GoalInfo) -> String {
    match info {
        GoalInfo::GoalType {
            ty,
            type_aux,
            entries,
            boundary,
            output_forms,
        } => {
            let mut out = format!("**Goal ?{id}**\n\n```agda\nGoal: {ty}\n");
            if let TypeAux::GoalAndHave { expr } = type_aux {
                out.push_str(&format!("Have: {expr}\n"));
            }
            for value in boundary {
                out.push_str(&format!("Boundary: {}\n", message_text(value)));
            }
            out.push_str(RULE);
            out.push('\n');
            for entry in entries {
                let scope = if entry.in_scope {
                    ""
                } else {
                    "  (not in scope)"
                };
                out.push_str(&format!(
                    "{} : {}{scope}\n",
                    entry.reified_name, entry.binding
                ));
            }
            for form in output_forms {
                out.push_str(&format!("{}\n", message_text(form)));
            }
            out.push_str("```\n");
            out
        }
        GoalInfo::NormalForm { expr } => normal_form(id, None, expr),
        GoalInfo::HelperFunction { signature } => {
            format!("**Helper function for ?{id}**\n\n```agda\n{signature}\n```\n")
        }
        GoalInfo::Other => format!(
            "**Goal ?{id}**\n\n(Agda sent goal information this version cannot show yet.)\n"
        ),
    }
}

/// Agda's answer about a goal: its type and context, and, for text in the
/// goal, the type of that text or why there is none.
#[derive(Debug, Clone)]
pub struct GoalAnswer {
    /// The goal's text when Agda was asked, which `Have:` is about.
    pub text: String,
    pub info: GoalInfo,
    /// Why Agda cannot type that text.
    pub untyped: Option<String>,
}

impl GoalAnswer {
    pub fn markdown(&self, id: u32) -> String {
        let mut out = goal(id, &self.info);
        if let Some(message) = &self.untyped {
            out.push_str(&untyped(message));
        }
        out
    }
}

/// What hover says when Agda is busy and nothing is known about the goal.
pub const BUSY: &str = "Agda is busy. Hover again in a moment.";

/// What hover shows about goal `id`, with `text` in it, while Agda stays
/// busy: Agda's last answer, without `Have:` when that was about other text,
/// or else the goal's type `ty` from the last load; then what is missing, or,
/// while Agda loads the file again (`reloading`), that it is from before.
pub fn goal_while_busy(
    id: u32,
    answer: Option<&GoalAnswer>,
    ty: Option<&str>,
    text: &str,
    reloading: bool,
) -> String {
    let (shown, missing) = match (answer, ty) {
        (Some(answer), _) if answer.text == text => (answer.markdown(id), None),
        (Some(answer), _) => {
            let mut info = answer.info.clone();
            if let GoalInfo::GoalType { type_aux, .. } = &mut info {
                *type_aux = TypeAux::GoalOnly;
            }
            let missing = (!text.is_empty()).then_some("the type of the goal's text");
            (goal(id, &info), missing)
        }
        (None, Some(ty)) => {
            let missing = match text.is_empty() {
                true => "the context",
                false => "the context and the type of the goal's text",
            };
            (
                format!("**Goal ?{id}**\n\n```agda\nGoal: {ty}\n```\n"),
                Some(missing),
            )
        }
        (None, None) => return BUSY.to_string(),
    };
    let note = match (reloading, missing) {
        (true, _) => "Agda is loading the file again: this is from the last load.".to_string(),
        (false, Some(missing)) => format!("Agda is busy: {missing} follows when it is free."),
        (false, None) => return shown,
    };
    format!("{shown}\n*{note}*\n")
}

/// Why the text in a goal has no type, to follow the goal and its context.
pub fn untyped(message: &str) -> String {
    format!("\nAgda cannot infer a type for the text in the goal:\n\n```text\n{message}\n```\n")
}

/// The normal form of the text in goal `id`, with that text when known, on
/// one line.
pub fn normal_form(id: u32, text: Option<&str>, normal: &str) -> String {
    let of = text
        .map(|text| {
            format!(
                " of `{}`",
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .unwrap_or_default();
    format!("**Normal form{of} in ?{id}**\n\n```agda\n{normal}\n```\n")
}

/// Agda's answer to why a name is in scope, tidied: in a file without goals,
/// Agda 2.8 leaves out where an `open` is (`the opening of Nat at` and
/// nothing after it), so such a line says so instead.
pub fn why_in_scope(message: &str) -> String {
    message
        .lines()
        .map(|line| match line.trim_end().strip_suffix(" at") {
            Some(start) => format!("{start} (Agda gives no location in a file without goals)"),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The file Agda answered about, for the locations in the output file: its
/// name, and a link to it from the output file, which takes `:line:column`.
pub struct Source {
    pub name: String,
    pub link: Box<dyn Fn(u64, u64) -> String + Send + Sync>,
}

/// Any `DisplayInfo`, for the output file, with locations in `source`.
pub fn display_info(raw: &Value, source: Option<&Source>) -> String {
    match DisplayInfo::parse(raw) {
        DisplayInfo::AllGoalsWarnings {
            visible_goals,
            invisible_goals,
            warnings,
            errors,
        } => {
            let mut out = String::new();
            section(&mut out, "Errors", errors.iter().map(message_text));
            let goals = visible_goals
                .iter()
                .map(|goal| format!("{} : {}", goal.label(), goal.ty.as_deref().unwrap_or("_")));
            section(&mut out, "Goals", goals);
            unsolved_metas(&mut out, &visible_goals, &invisible_goals, source);
            section(&mut out, "Warnings", warnings.iter().map(message_text));
            if out.is_empty() {
                out.push_str("All done: no goals, errors or warnings.\n");
            }
            out
        }
        DisplayInfo::Error {
            error,
            message,
            warnings,
        } => {
            let text = message
                .or_else(|| error.as_ref().map(message_text))
                .unwrap_or_default();
            let mut out = String::new();
            section(&mut out, "Error", std::iter::once(text));
            section(&mut out, "Warnings", warnings.iter().map(message_text));
            out
        }
        DisplayInfo::GoalSpecific {
            interaction_point,
            goal_info,
        } => goal(interaction_point.id, &goal_info),
        DisplayInfo::Auto { info } => format!("## Auto\n\n{info}\n"),
        DisplayInfo::InferredType { expr } => format!("```agda\n{expr}\n```\n"),
        DisplayInfo::WhyInScope { message } => format!("```text\n{message}\n```\n"),
        DisplayInfo::Constraints { constraints } => {
            let mut out = String::new();
            let none = std::collections::HashMap::new();
            section(
                &mut out,
                "Constraints",
                constraints
                    .iter()
                    .map(|form| crate::metas::constraint_text(form, &none)),
            );
            match out.is_empty() {
                true => "No constraints.\n".to_string(),
                false => out,
            }
        }
        DisplayInfo::Other => format!(
            "```json\n{}\n```\n",
            serde_json::to_string_pretty(raw).unwrap_or_default()
        ),
    }
}

/// The unsolved metas, each with its location, a link when the output file
/// knows where the file is, as in Emacs's `*All Goals*`:
/// `` `_13 : ℕ` at [Metas.agda:15.8-9](../Metas.agda:15:8) ``. A meta on a
/// goal is left out, as in the diagnostics: it is the goal's own type, or
/// that type's sort, which the goal shows.
fn unsolved_metas(
    out: &mut String,
    goals: &[OutputConstraint],
    metas: &[OutputConstraint],
    source: Option<&Source>,
) {
    let goal_ranges: Vec<(usize, usize)> = goals.iter().filter_map(metas::range_of).collect();
    let items: Vec<String> = metas
        .iter()
        .filter(|meta| {
            !metas::range_of(meta).is_some_and(|(start, end)| {
                goal_ranges
                    .iter()
                    .any(|&(from, to)| from <= start && end <= to)
            })
        })
        .filter_map(|meta| {
            let label = metas::label_of(meta)?;
            let at = match (metas::location_of(meta), source) {
                (Some(location), Some(source)) => format!(
                    " at [{}:{}]({})",
                    source.name,
                    location.agda(),
                    (source.link)(location.line, location.column)
                ),
                (Some(location), None) => format!(" at line {}", location.agda()),
                (None, _) => String::new(),
            };
            Some(format!("- `{label}`{at}\n"))
        })
        .collect();
    if items.is_empty() {
        return;
    }
    out.push_str("## Unsolved metas\n\n");
    for item in items {
        out.push_str(&item);
    }
    out.push('\n');
}

fn section(out: &mut String, title: &str, items: impl Iterator<Item = String>) {
    let items: Vec<String> = items.collect();
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n```agda\n"));
    for item in items {
        out.push_str(item.trim_end());
        out.push('\n');
    }
    out.push_str("```\n\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ContextEntry;

    #[test]
    fn shows_the_type_of_a_goals_text_and_its_normal_form() {
        let info = GoalInfo::GoalType {
            ty: "ℕ".into(),
            type_aux: TypeAux::GoalAndHave {
                expr: "ℕ → ℕ".into(),
            },
            entries: vec![ContextEntry {
                reified_name: "n".into(),
                binding: "ℕ".into(),
                in_scope: true,
            }],
            boundary: vec![],
            output_forms: vec![],
        };
        assert_eq!(
            goal(3, &info),
            format!("**Goal ?3**\n\n```agda\nGoal: ℕ\nHave: ℕ → ℕ\n{RULE}\nn : ℕ\n```\n")
        );
        assert_eq!(
            normal_form(1, Some("two\n  + two"), "suc (suc zero)"),
            "**Normal form of `two + two` in ?1**\n\n```agda\nsuc (suc zero)\n```\n"
        );
        assert!(
            goal(
                1,
                &GoalInfo::NormalForm {
                    expr: "zero".into()
                }
            )
            .starts_with("**Normal form in ?1**")
        );
    }

    #[test]
    fn shows_what_is_known_while_agda_is_busy() {
        let info = GoalInfo::GoalType {
            ty: "ℕ".into(),
            type_aux: TypeAux::GoalAndHave {
                expr: "ℕ → ℕ".into(),
            },
            entries: vec![ContextEntry {
                reified_name: "n".into(),
                binding: "ℕ".into(),
                in_scope: true,
            }],
            boundary: vec![],
            output_forms: vec![],
        };
        let answer = GoalAnswer {
            text: "suc".into(),
            info,
            untyped: None,
        };
        let full = answer.markdown(3);
        assert!(full.contains("Have: ℕ → ℕ\n"), "{full}");
        // The answer for this text, whole; only while loading with a note.
        assert_eq!(goal_while_busy(3, Some(&answer), None, "suc", false), full);
        assert_eq!(
            goal_while_busy(3, Some(&answer), None, "suc", true),
            format!("{full}\n*Agda is loading the file again: this is from the last load.*\n")
        );
        // For other text, without `Have:`; for no text that is all of it.
        let other = goal_while_busy(3, Some(&answer), Some("ℕ"), "suc n", false);
        assert!(
            !other.contains("Have:")
                && other.contains("n : ℕ")
                && other.ends_with(
                    "\n*Agda is busy: the type of the goal's text follows when it is free.*\n"
                ),
            "{other}"
        );
        let empty = goal_while_busy(3, Some(&answer), Some("ℕ"), "", false);
        assert!(
            !empty.contains("Have:") && !empty.contains("*Agda"),
            "{empty}"
        );
        // Without an answer, the type from the load.
        assert_eq!(
            goal_while_busy(3, None, Some("ℕ"), "", false),
            "**Goal ?3**\n\n```agda\nGoal: ℕ\n```\n\n*Agda is busy: the context follows when it is free.*\n"
        );
        assert!(
            goal_while_busy(3, None, Some("ℕ"), "suc", false)
                .contains("the context and the type of the goal's text follows")
        );
        assert_eq!(goal_while_busy(3, None, None, "", false), BUSY);
    }

    #[test]
    fn says_when_agda_leaves_out_a_location() {
        let message = "suc is in scope as\n  * a constructor Nat.ℕ.suc brought into scope by\n    - the opening of Nat at\n    - its definition at Nat.agda:5.3-6";
        assert_eq!(
            why_in_scope(message),
            "suc is in scope as\n  * a constructor Nat.ℕ.suc brought into scope by\n    - the opening of Nat (Agda gives no location in a file without goals)\n    - its definition at Nat.agda:5.3-6"
        );
    }

    #[test]
    fn lists_unsolved_metas_with_locations() {
        // Shaped as Agda 2.8.0.2 sends them: a goal, the type of a `with`
        // goal and its sort on that goal, an implicit argument and a sort.
        let at = |pos: u64, line: u64, col: u64| serde_json::json!({ "pos": pos, "line": line, "col": col });
        let raw = serde_json::json!({
            "kind": "AllGoalsWarnings",
            "visibleGoals": [
                { "constraintObj": { "id": 2, "range": [{ "start": at(200, 13, 16), "end": at(206, 13, 22) }] },
                  "kind": "OfType", "type": "_15" },
            ],
            "invisibleGoals": [
                { "constraintObj": { "name": "_15", "range": [{ "start": at(200, 13, 16), "end": at(206, 13, 22) }] },
                  "kind": "OfType", "type": "_14" },
                { "constraintObj": { "name": "_m_19", "range": [{ "start": at(300, 22, 7), "end": at(305, 22, 12) }] },
                  "kind": "OfType", "type": "ℕ" },
                { "constraintObj": { "name": "_10", "range": [{ "start": at(400, 30, 9), "end": at(401, 31, 1) }] },
                  "kind": "JustSort" },
            ],
            "warnings": [],
            "errors": [],
        });
        let source = Source {
            name: "Metas.agda".into(),
            link: Box::new(|line, column| format!("../Metas.agda:{line}:{column}")),
        };
        let markdown = display_info(&raw, Some(&source));
        assert_eq!(
            markdown,
            "## Goals\n\n```agda\n?2 : _15\n```\n\n\
             ## Unsolved metas\n\n\
             - `_m_19 : ℕ` at [Metas.agda:22.7-12](../Metas.agda:22:7)\n\
             - `Sort _10` at [Metas.agda:30.9-31.1](../Metas.agda:30:9)\n\n"
        );
        // Without a link, the location alone.
        assert!(display_info(&raw, None).contains("- `_m_19 : ℕ` at line 22.7-12\n"));
    }
}
