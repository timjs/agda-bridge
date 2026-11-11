//! Unsolved metas: what Agda could not solve and did not make a goal, such as
//! an implicit argument nothing determines, or a `_` in the source. After a
//! load Agda lists them as `invisibleGoals`, with a name, a type and a range,
//! as it lists goals; a meta that is itself a type or a sort has no type. The
//! bridge keeps them like goals and names, moving them along with edits, so
//! that they can be diagnostics and be explained in hover, with what Agda
//! knows about them: the constraints stuck on them, and how they look with
//! implicit arguments shown.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::protocol::OutputConstraint;
use crate::text::Change;

/// An unsolved meta, at a half-open range of 0-based char offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// Agda's name for it: `_12`, or `_m_12` for an implicit argument `m`.
    pub name: String,
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    /// The variable this meta is the type of, when Agda could not infer the
    /// type of a binder, as `m` in `∀ {m} → …`.
    pub binder: Option<String>,
}

/// What Agda says a meta is: a term of a type, or itself a type or a sort,
/// as `OfType`, `JustType` and `JustSort` in its answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    OfType(String),
    Type,
    Sort,
}

impl Kind {
    fn of(goal: &OutputConstraint) -> Kind {
        match (goal.kind.as_str(), &goal.ty) {
            ("JustType", _) => Kind::Type,
            ("JustSort", _) => Kind::Sort,
            (_, ty) => Kind::OfType(ty.clone().unwrap_or_else(|| "_".into())),
        }
    }
}

impl Meta {
    /// Its number, by which constraints say what they are blocked on: 12
    /// for `_12` and `_m_12`.
    pub fn number(&self) -> Option<u64> {
        self.name.rsplit('_').next()?.parse().ok()
    }

    /// The implicit argument this meta stands for, which Agda names it
    /// after: `m` for `_m_12`, none for `_12`.
    pub fn argument(&self) -> Option<&str> {
        let (argument, number) = self.name.strip_prefix('_')?.rsplit_once('_')?;
        (!argument.is_empty() && number.parse::<u64>().is_ok()).then_some(argument)
    }

    /// The meta as Agda prints it in its list of goals: `_15 : ℕ`, or
    /// `Type _3` and `Sort _4` for a meta that is a type or a sort.
    pub fn label(&self) -> String {
        label(&self.name, &self.kind)
    }

    /// The meta as a diagnostic says it.
    pub fn message(&self) -> String {
        let meta = match &self.kind {
            Kind::OfType(ty) => format!("Unsolved meta {} : {ty}", self.name),
            Kind::Type => format!("Unsolved meta {}, a type", self.name),
            Kind::Sort => format!("Unsolved meta {}, a sort", self.name),
        };
        match (self.argument(), &self.binder) {
            (Some(argument), _) => format!("{meta}, the implicit argument {argument}"),
            (None, Some(binder)) => format!("{meta}, the type of {binder}"),
            (None, None) => meta,
        }
    }
}

/// A meta as Agda prints it in its list of goals.
fn label(name: &str, kind: &Kind) -> String {
    match kind {
        Kind::OfType(ty) => format!("{name} : {ty}"),
        Kind::Type => format!("Type {name}"),
        Kind::Sort => format!("Sort {name}"),
    }
}

/// The metas Agda listed, by name, as Agda printed them: for the answer
/// with implicit arguments shown.
pub fn labels(listed: &[OutputConstraint]) -> HashMap<String, String> {
    listed
        .iter()
        .filter_map(|goal| {
            let name = goal.constraint_obj.get("name")?.as_str()?;
            Some((name.to_string(), label(name, &Kind::of(goal))))
        })
        .collect()
}

/// Whether `text` has `name` as a word of its own, so that `_21` is not found
/// in `_210`.
fn mentions(text: &str, name: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || "(){}\";.@,[]".contains(c))
        .any(|word| word == name)
}

/// The metas Agda listed after a load, with Agda's 1-based code point offsets
/// turned into 0-based char offsets. A meta without a range is left out:
/// there is nothing to show it on.
pub fn from_invisible_goals(goals: &[OutputConstraint]) -> Vec<Meta> {
    goals
        .iter()
        .filter_map(|goal| {
            let object = goal.constraint_obj.as_object()?;
            let name = object.get("name")?.as_str()?.to_string();
            let interval = object.get("range")?.as_array()?.first()?;
            let position = |end: &str| -> Option<usize> {
                Some(interval.get(end)?.get("pos")?.as_u64()? as usize - 1)
            };
            Some(Meta {
                name,
                kind: Kind::of(goal),
                start: position("start")?,
                end: position("end")?,
                binder: None,
            })
        })
        .collect()
}

/// Move the metas along with an edit; an edit that touches a meta removes
/// it, since what Agda could not solve there has changed.
pub fn adjust(metas: &mut Vec<Meta>, change: &Change) {
    metas.retain_mut(|meta| match change.follow(meta.start, meta.end) {
        Some((start, end)) => {
            (meta.start, meta.end) = (start, end);
            true
        }
        None => false,
    });
}

/// The metas whose range covers the char at `offset`, innermost first.
pub fn metas_under(metas: &[Meta], offset: usize) -> Vec<&Meta> {
    let mut found: Vec<&Meta> = metas
        .iter()
        .filter(|meta| meta.start <= offset && offset < meta.end)
        .collect();
    found.sort_by_key(|meta| meta.end - meta.start);
    found
}

/// What hover says about a meta besides the meta itself.
#[derive(Debug, Default)]
pub struct Details {
    /// The text of the meta's range.
    pub source: String,
    /// The meta as Agda prints it with implicit arguments shown, when that
    /// differs from [`Meta::label`].
    pub implicit: Option<String>,
    /// The constraints on it, as [`constraints_on`] gives them.
    pub constraints: Vec<String>,
    /// The same with implicit arguments shown, when that differs.
    pub implicit_constraints: Vec<String>,
    /// Where else it occurs, as [`uses`] gives it.
    pub uses: Vec<String>,
}

/// Hover on a meta: the meta as Agda prints it, and with implicit arguments;
/// what it is and how to see its context; where else it occurs; and the
/// constraints on it, also with implicit arguments.
pub fn hover(meta: &Meta, details: &Details) -> String {
    let mut out = format!("**Unsolved meta `{}`**\n\n", meta.name);
    code(&mut out, None, std::slice::from_ref(&meta.label()));
    if let Some(implicit) = &details.implicit {
        code(
            &mut out,
            Some("With implicit arguments:"),
            std::slice::from_ref(implicit),
        );
    }
    out.push_str(
        &match (meta.argument(), &meta.binder, details.source.as_str()) {
            (Some(argument), _, _) => format!(
                "Agda could not infer the implicit argument `{argument}`. To see its \
             context, make it a goal: give the argument as `{{{argument} = ?}}`, \
             for an operator in prefix form.\n"
            ),
            (None, Some(binder), _) => format!(
                "Agda could not infer the type of `{binder}`. Write it, as in \
             `{{{binder} : …}}`.\n"
            ),
            (None, None, "_") => "Agda could not solve this `_`. To see its context, make \
                              it a goal: write `?` in its place.\n"
                .to_string(),
            (None, None, _) => "Agda could not solve this meta.\n".to_string(),
        },
    );
    if !details.uses.is_empty() {
        out.push_str("\nIt also occurs:\n\n");
        for place in &details.uses {
            out.push_str(&format!("- {place}\n"));
        }
    }
    if !details.constraints.is_empty() {
        out.push('\n');
        code(&mut out, Some("Constraints on it:"), &details.constraints);
        if !details.implicit_constraints.is_empty() {
            code(
                &mut out,
                Some("With implicit arguments:"),
                &details.implicit_constraints,
            );
        }
    }
    out
}

/// An ```agda block with `lines`, after `title` when there is one.
fn code(out: &mut String, title: Option<&str>, lines: &[String]) {
    if let Some(title) = title {
        out.push_str(title);
        out.push_str("\n\n");
    }
    out.push_str("```agda\n");
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("```\n\n");
}

/// Where else `meta` occurs: as or in the type of a goal, from
/// `goal_types`, or of one of the other `metas`.
pub fn uses(meta: &Meta, metas: &[Meta], goal_types: &HashMap<u32, String>) -> Vec<String> {
    let role = |ty: &str, what: String| {
        if ty.trim() == meta.name {
            Some(format!("as the type of {what}"))
        } else if mentions(ty, &meta.name) {
            Some(format!("in the type of {what}"))
        } else {
            None
        }
    };
    let mut goals: Vec<(&u32, &String)> = goal_types.iter().collect();
    goals.sort();
    let in_goals = goals
        .into_iter()
        .filter_map(|(id, ty)| role(ty, format!("goal `?{id}`")));
    let in_metas = metas
        .iter()
        .filter(|other| other.name != meta.name)
        .filter_map(|other| match &other.kind {
            Kind::OfType(ty) => role(ty, format!("`{}`", other.name)),
            _ => None,
        });
    let mut found: Vec<String> = in_goals.chain(in_metas).collect();
    found.dedup();
    found
}

/// The names of `metas` by number, so that a constraint blocked on meta 12
/// says `_m_12`, as the diagnostic and the hover do, where Agda says `_12`.
pub fn names(metas: &[Meta]) -> HashMap<u64, String> {
    metas
        .iter()
        .filter_map(|meta| Some((meta.number()?, meta.name.clone())))
        .collect()
}

/// The constraints of `forms`, from `Cmd_constraints`, that are on `meta`:
/// blocked on it, or mentioning it. Each as [`constraint_text`] prints it.
pub fn constraints_on(meta: &Meta, forms: &[Value], names: &HashMap<u64, String>) -> Vec<String> {
    let number = meta.number();
    forms
        .iter()
        .filter_map(|form| {
            let text = constraint_text(form, names);
            let blocked = number.is_some_and(|number| blocked_on(form.get("unblocker"), number));
            (blocked || mentions(&text, &meta.name)).then_some(text)
        })
        .collect()
}

/// Whether a blocker, as Agda sends it, waits for meta `number`.
fn blocked_on(blocker: Option<&Value>, number: u64) -> bool {
    let Some(blocker) = blocker else {
        return false;
    };
    match blocker.get("kind").and_then(Value::as_str) {
        Some("UnblockOnMeta") => {
            blocker
                .get("meta")
                .and_then(|meta| meta.get("id"))
                .and_then(Value::as_u64)
                == Some(number)
        }
        Some("UnblockOnAll" | "UnblockOnAny") => blocker
            .get("blockers")
            .and_then(Value::as_array)
            .is_some_and(|blockers| blockers.iter().any(|b| blocked_on(Some(b), number))),
        _ => false,
    }
}

/// A constraint from `Cmd_constraints`, which Agda sends as an object, as
/// Agda prints it for Emacs: a port of the `Pretty` instance of
/// `OutputConstraint` in Agda 2.8.0's `src/full/Agda/Interaction/BasicOps.hs`,
/// on one line, followed by what the constraint is blocked on, as in
/// `pred _21 = zero : ℕ (blocked on _21)`. Metas in `names` get their names.
/// A kind this port does not know is shown as Agda's JSON.
pub fn constraint_text(form: &Value, names: &HashMap<u64, String>) -> String {
    let constraint = form.get("constraint").unwrap_or(form);
    let field = |key: &str| show(constraint.get(key));
    let object = |index: usize| {
        show(
            constraint
                .get("constraintObjs")
                .and_then(Value::as_array)
                .and_then(|objects| objects.get(index)),
        )
    };
    let comparison = match constraint.get("comparison").and_then(Value::as_str) {
        Some("CmpLeq") => "=<",
        _ => "=",
    };
    let kind = constraint.get("kind").and_then(Value::as_str).unwrap_or("");
    let body = match kind {
        "OfType" => format!("{} : {}", field("constraintObj"), field("type")),
        "JustType" => format!("Type {}", field("constraintObj")),
        "JustSort" => format!("Sort {}", field("constraintObj")),
        "CmpInType" => format!(
            "{} {comparison} {} : {}",
            object(0),
            object(1),
            field("type")
        ),
        // Agda prints the polarities where other comparisons have `=`.
        "CmpElim" => format!(
            "{} {} {} : {}",
            object(0),
            polarities(constraint.get("polarities")),
            object(1),
            field("type")
        ),
        "CmpTypes" | "CmpLevels" | "CmpTeles" | "CmpSorts" => {
            format!("{} {comparison} {}", object(0), object(1))
        }
        "Assign" => format!("{} := {}", field("constraintObj"), field("value")),
        "TypedAssign" => format!(
            "{} := {} :? {}",
            field("constraintObj"),
            field("value"),
            field("type")
        ),
        "PostponedCheckArgs" => {
            let arguments: Vec<String> = constraint
                .get("arguments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|argument| {
                    let argument = show(Some(argument));
                    match argument.contains([' ', '\n']) {
                        true => format!("({argument})"),
                        false => argument,
                    }
                })
                .collect();
            format!(
                "{} := (_ : {}) {} : {}",
                field("constraintObj"),
                field("ofType"),
                arguments.join(" "),
                field("type")
            )
        }
        "IsEmptyType" => format!("Is empty: {}", field("type")),
        "SizeLtSat" => format!("Not empty type of sizes: {}", field("type")),
        "FindInstanceOF" => {
            let candidates: Vec<String> = constraint
                .get("candidates")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|candidate| {
                    format!(
                        "    {} : {}",
                        show(candidate.get("value")),
                        show(candidate.get("type"))
                    )
                })
                .collect();
            let mut text = format!(
                "Resolve instance argument {} : {}\n  Candidate:",
                field("constraintObj"),
                field("type")
            );
            for candidate in candidates {
                text.push('\n');
                text.push_str(&candidate);
            }
            text
        }
        "ResolveInstanceOF" => format!("Resolve output type of instance {}", field("name")),
        "PTSInstance" => format!("PTS instance for ({}, {})", object(0), object(1)),
        "PostponedCheckFunDef" => {
            format!("Check definition of {} : {}", field("name"), field("type"))
        }
        "DataSort" => format!("Sort {} allows data/record definitions", field("sort")),
        "CheckLock" => format!("Check lock {} allows {}", field("lock"), field("head")),
        "UsableAtMod" => format!("Is usable at {} modality: {}", field("mod"), field("term")),
        _ => constraint.to_string(),
    };
    match unblocker(form.get("unblocker"), names) {
        Some(blocked) => format!("{body} ({blocked})"),
        None => body,
    }
}

/// A part of a constraint: Agda sends terms as text, and a comparison of
/// eliminations as lists, which Agda prints as `[a, b]`.
fn show(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => {
            let items: Vec<String> = items.iter().map(|item| show(Some(item))).collect();
            format!("[{}]", items.join(", "))
        }
        Some(Value::Null) | None => "_".to_string(),
        Some(other) => other.to_string(),
    }
}

/// The polarities of a `CmpElim`, as Agda prints them: `[+, -]`.
fn polarities(value: Option<&Value>) -> String {
    let symbols: Vec<&str> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|polarity| match polarity.as_str() {
            Some("Covariant") => "+",
            Some("Contravariant") => "-",
            Some("Invariant") => "*",
            Some("Nonvariant") => "_",
            _ => "?",
        })
        .collect();
    format!("[{}]", symbols.join(", "))
}

/// What a constraint waits for, as Agda prints it after the constraint:
/// `blocked on _21`, `blocked on problem 31`, or `stuck`; nothing for a
/// constraint that waits for nothing.
fn unblocker(blocker: Option<&Value>, names: &HashMap<u64, String>) -> Option<String> {
    let blocker = blocker?;
    let blockers = || {
        blocker
            .get("blockers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    match blocker.get("kind").and_then(Value::as_str)? {
        "UnblockOnAll" if blockers().is_empty() => None,
        "UnblockOnAny" if blockers().is_empty() => Some("stuck".to_string()),
        _ => Some(format!("blocked on {}", blocker_name(blocker, names))),
    }
}

/// A blocker as Agda's `Pretty` instance prints it.
fn blocker_name(blocker: &Value, names: &HashMap<u64, String>) -> String {
    let all = |word: &str| {
        let parts: Vec<String> = blocker
            .get("blockers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|inner| blocker_name(inner, names))
            .collect();
        format!("{word}({})", parts.join(", "))
    };
    match blocker.get("kind").and_then(Value::as_str) {
        Some("UnblockOnMeta") => {
            match blocker
                .get("meta")
                .and_then(|meta| meta.get("id"))
                .and_then(Value::as_u64)
            {
                Some(id) => names.get(&id).cloned().unwrap_or_else(|| format!("_{id}")),
                None => "_".to_string(),
            }
        }
        Some("UnblockOnProblem") => format!("problem {}", show(blocker.get("id"))),
        Some("UnblockOnDef") => format!("definition {}", show(blocker.get("name"))),
        Some("UnblockOnAll") => all("all"),
        Some("UnblockOnAny") => all("any"),
        _ => blocker.to_string(),
    }
}

/// Keep only the metas that Agda still lists as unsolved, with their types
/// as Agda now gives them. After a goal command Agda lists the metas again,
/// but their ranges are those of the last load, which edits have moved
/// since: so the bridge keeps its own ranges, and only drops the metas the
/// command solved. Metas the command made appear at the next load.
pub fn keep_listed(metas: &mut Vec<Meta>, listed: &[OutputConstraint]) {
    let kinds: Vec<(String, Kind)> = listed
        .iter()
        .filter_map(|goal| {
            let name = goal.constraint_obj.get("name")?.as_str()?;
            Some((name.to_string(), Kind::of(goal)))
        })
        .collect();
    let names: HashSet<&str> = kinds.iter().map(|(name, _)| name.as_str()).collect();
    metas.retain(|meta| names.contains(meta.name.as_str()));
    for meta in metas.iter_mut() {
        if let Some((_, kind)) = kinds.iter().find(|(name, _)| *name == meta.name) {
            meta.kind = kind.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unsolved meta as Agda lists it, at 1-based offsets.
    fn invisible(name: &str, ty: &str, start: usize, end: usize) -> OutputConstraint {
        let at = |pos: usize| serde_json::json!({ "pos": pos });
        serde_json::from_value(serde_json::json!({
            "constraintObj": {
                "name": name,
                "range": [{ "start": at(start), "end": at(end) }],
            },
            "kind": "OfType",
            "type": ty,
        }))
        .unwrap()
    }

    fn meta(name: &str, start: usize, end: usize) -> Meta {
        Meta {
            name: name.into(),
            kind: Kind::OfType("ℕ".into()),
            start,
            end,
            binder: None,
        }
    }

    #[test]
    fn reads_metas_with_their_ranges() {
        let listed = [
            invisible("_m_15", "ℕ", 216, 221),
            invisible("_18", "Set", 300, 301),
        ];
        assert_eq!(
            from_invisible_goals(&listed),
            [
                meta("_m_15", 215, 220),
                Meta {
                    kind: Kind::OfType("Set".into()),
                    ..meta("_18", 299, 300)
                },
            ]
        );
        // A meta that is a type or a sort has no type, as Agda 2.8.0.2
        // sends for the type of a `with` goal and its sort.
        let sort: OutputConstraint = serde_json::from_value(serde_json::json!({
            "constraintObj": { "name": "_14", "range": [{ "start": { "pos": 7 }, "end": { "pos": 13 } }] },
            "kind": "JustSort",
        }))
        .unwrap();
        let read = from_invisible_goals(&[sort]);
        assert_eq!(
            read,
            [Meta {
                kind: Kind::Sort,
                ..meta("_14", 6, 12)
            }]
        );
        assert_eq!(read[0].label(), "Sort _14");
        assert_eq!(read[0].message(), "Unsolved meta _14, a sort");
        // A meta without a range is left out.
        let rangeless: OutputConstraint = serde_json::from_value(serde_json::json!({
            "constraintObj": { "name": "_3", "range": [] },
            "kind": "OfType",
            "type": "ℕ",
        }))
        .unwrap();
        assert_eq!(from_invisible_goals(&[rangeless]), []);
    }

    #[test]
    fn names_the_implicit_argument() {
        assert_eq!(meta("_m_15", 0, 1).number(), Some(15));
        assert_eq!(meta("_18", 0, 1).number(), Some(18));
        assert_eq!(meta("_m_15", 0, 1).argument(), Some("m"));
        assert_eq!(meta("_A_7", 0, 1).argument(), Some("A"));
        assert_eq!(meta("_x₁_7", 0, 1).argument(), Some("x₁"));
        assert_eq!(meta("_18", 0, 1).argument(), None);
        assert_eq!(
            meta("_m_15", 0, 1).message(),
            "Unsolved meta _m_15 : ℕ, the implicit argument m"
        );
        assert_eq!(meta("_18", 0, 1).message(), "Unsolved meta _18 : ℕ");
        let binder = Meta {
            binder: Some("m".into()),
            kind: Kind::OfType("Set".into()),
            ..meta("_360", 0, 1)
        };
        assert_eq!(binder.message(), "Unsolved meta _360 : Set, the type of m");
    }

    #[test]
    fn follows_edits() {
        let mut metas = vec![meta("_1", 10, 15), meta("_2", 20, 21)];
        // Two chars inserted before both move them.
        adjust(
            &mut metas,
            &Change {
                start: 0,
                old_end: 0,
                new_len: 2,
            },
        );
        assert_eq!(metas, [meta("_1", 12, 17), meta("_2", 22, 23)]);
        // An edit inside the first removes it, and moves the second.
        adjust(
            &mut metas,
            &Change {
                start: 13,
                old_end: 14,
                new_len: 3,
            },
        );
        assert_eq!(metas, [meta("_2", 24, 25)]);
        // An edit after it leaves it.
        adjust(
            &mut metas,
            &Change {
                start: 25,
                old_end: 25,
                new_len: 4,
            },
        );
        assert_eq!(metas, [meta("_2", 24, 25)]);
    }

    #[test]
    fn finds_the_innermost_meta_first() {
        let metas = [
            meta("_outer", 0, 10),
            meta("_inner", 4, 6),
            meta("_after", 10, 12),
        ];
        let names = |offset| -> Vec<&str> {
            metas_under(&metas, offset)
                .iter()
                .map(|meta| meta.name.as_str())
                .collect()
        };
        assert_eq!(names(5), ["_inner", "_outer"]);
        assert_eq!(names(0), ["_outer"]);
        assert_eq!(names(10), ["_after"]);
        assert!(names(12).is_empty());
    }

    /// A constraint as `Cmd_constraints` sends it.
    fn form(constraint: Value, unblocker: Value) -> Value {
        serde_json::json!({
            "constraint": constraint,
            "problems": [31],
            "range": [],
            "unblocker": unblocker,
        })
    }

    fn on_meta(id: u64) -> Value {
        serde_json::json!({ "kind": "UnblockOnMeta", "meta": { "id": id, "module": 7 } })
    }

    #[test]
    fn prints_constraints_as_agda_does() {
        use serde_json::json;
        let none = HashMap::new();
        // As Agda 2.8.0.2 sends and prints the constraint of `stuck` in
        // tests/fixtures/Metas.agda.
        let stuck = form(
            json!({ "kind": "CmpInType", "comparison": "CmpEq",
                    "constraintObjs": ["pred _21", "zero"], "type": "ℕ" }),
            on_meta(21),
        );
        assert_eq!(
            constraint_text(&stuck, &none),
            "pred _21 = zero : ℕ (blocked on _21)"
        );
        let names = HashMap::from([(21, "_m_21".to_string())]);
        assert_eq!(
            constraint_text(&stuck, &names),
            "pred _21 = zero : ℕ (blocked on _m_21)"
        );
        let cases = [
            (
                json!({ "kind": "Assign", "constraintObj": "_22", "value": "refl" }),
                json!({ "kind": "UnblockOnProblem", "id": 31 }),
                "_22 := refl (blocked on problem 31)",
            ),
            (
                json!({ "kind": "CmpTypes", "comparison": "CmpLeq", "constraintObjs": ["A", "B"] }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                "A =< B",
            ),
            (
                json!({ "kind": "OfType", "constraintObj": "_3", "type": "Set" }),
                json!({ "kind": "UnblockOnAny", "blockers": [] }),
                "_3 : Set (stuck)",
            ),
            (
                json!({ "kind": "TypedAssign", "constraintObj": "_4", "value": "x", "type": "A" }),
                json!({ "kind": "UnblockOnAny", "blockers": [on_meta(1), { "kind": "UnblockOnDef", "name": "f" }] }),
                "_4 := x :? A (blocked on any(_1, definition f))",
            ),
            (
                json!({ "kind": "PostponedCheckArgs", "constraintObj": "_5", "ofType": "A → B",
                     "arguments": ["x", "f y"], "type": "B" }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                "_5 := (_ : A → B) x (f y) : B",
            ),
            (
                json!({ "kind": "CmpElim", "polarities": ["Covariant", "Invariant"], "type": "A",
                     "constraintObjs": [["a", "b"], ["c"]] }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                "[a, b] [+, *] [c] : A",
            ),
            (
                json!({ "kind": "IsEmptyType", "type": "⊥" }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                "Is empty: ⊥",
            ),
            (
                json!({ "kind": "FindInstanceOF", "constraintObj": "_6", "type": "Eq A",
                     "candidates": [{ "value": "eqℕ", "type": "Eq ℕ" }] }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                "Resolve instance argument _6 : Eq A\n  Candidate:\n    eqℕ : Eq ℕ",
            ),
            (
                json!({ "kind": "SomethingNew", "x": 1 }),
                json!({ "kind": "UnblockOnAll", "blockers": [] }),
                r#"{"kind":"SomethingNew","x":1}"#,
            ),
        ];
        for (constraint, unblocker, expected) in cases {
            assert_eq!(
                constraint_text(&form(constraint, unblocker), &none),
                expected
            );
        }
    }

    #[test]
    fn finds_the_constraints_on_a_meta() {
        use serde_json::json;
        let forms = [
            form(
                json!({ "kind": "CmpInType", "comparison": "CmpEq",
                        "constraintObjs": ["pred _21", "zero"], "type": "ℕ" }),
                on_meta(21),
            ),
            form(
                json!({ "kind": "Assign", "constraintObj": "_22", "value": "refl" }),
                json!({ "kind": "UnblockOnProblem", "id": 31 }),
            ),
            form(
                json!({ "kind": "CmpTypes", "comparison": "CmpEq",
                        "constraintObjs": ["_210", "ℕ"] }),
                json!({ "kind": "UnblockOnAny", "blockers": [on_meta(5), on_meta(210)] }),
            ),
        ];
        let metas = [meta("_21", 0, 1), meta("_22", 2, 3), meta("_x_5", 4, 5)];
        let names = names(&metas);
        let on = |meta: &Meta| constraints_on(meta, &forms, &names);
        // Blocked on it, not on `_210`, which only starts the same.
        assert_eq!(on(&metas[0]), ["pred _21 = zero : ℕ (blocked on _21)"]);
        // Mentioned, though blocked on a problem.
        assert_eq!(on(&metas[1]), ["_22 := refl (blocked on problem 31)"]);
        // Blocked on it among others.
        assert_eq!(on(&metas[2]), ["_210 = ℕ (blocked on any(_x_5, _210))"]);
    }

    #[test]
    fn explains_a_meta_in_hover() {
        let details = |source: &str| Details {
            source: source.into(),
            ..Details::default()
        };
        let text = hover(&meta("_m_15", 0, 5), &details("const"));
        assert!(text.starts_with("**Unsolved meta `_m_15`**\n\n```agda\n_m_15 : ℕ\n```\n"));
        assert!(text.contains("the implicit argument `m`") && text.contains("`{m = ?}`"));
        assert!(!text.contains("Constraints") && !text.contains("implicit arguments:"));
        let binder = Meta {
            binder: Some("m".into()),
            ..meta("_360", 0, 1)
        };
        assert!(
            hover(&binder, &details("m")).contains("the type of `m`. Write it, as in `{m : …}`.")
        );
        assert!(
            hover(&meta("_7", 0, 1), &details("m")).contains("Agda could not solve this meta.\n")
        );
        // Everything there is to say.
        let text = hover(
            &meta("_21", 0, 1),
            &Details {
                source: "_".into(),
                implicit: Some("_21 : _≡_ {ℕ} zero zero".into()),
                constraints: vec!["pred _21 = zero : ℕ".into()],
                implicit_constraints: vec!["pred {x} _21 = zero : ℕ".into()],
                uses: vec!["as the type of goal `?1`".into()],
            },
        );
        assert_eq!(
            text,
            "**Unsolved meta `_21`**\n\n```agda\n_21 : ℕ\n```\n\n\
             With implicit arguments:\n\n```agda\n_21 : _≡_ {ℕ} zero zero\n```\n\n\
             Agda could not solve this `_`. To see its context, make it a goal: write `?` \
             in its place.\n\
             \nIt also occurs:\n\n- as the type of goal `?1`\n\
             \nConstraints on it:\n\n```agda\npred _21 = zero : ℕ\n```\n\n\
             With implicit arguments:\n\n```agda\npred {x} _21 = zero : ℕ\n```\n\n"
        );
    }

    #[test]
    fn finds_where_else_a_meta_occurs() {
        let b = meta("_B_11", 0, 1);
        let others = [
            b.clone(),
            Meta {
                kind: Kind::OfType("_B_11 → ℕ".into()),
                ..meta("_15", 2, 3)
            },
            Meta {
                kind: Kind::OfType("_B_110".into()),
                ..meta("_16", 4, 5)
            },
            Meta {
                kind: Kind::Sort,
                ..meta("_17", 6, 7)
            },
        ];
        let goal_types = HashMap::from([
            (2, "Vec _B_11 n".to_string()),
            (1, "_B_11".to_string()),
            (0, "ℕ".to_string()),
        ]);
        assert_eq!(
            uses(&b, &others, &goal_types),
            [
                "as the type of goal `?1`",
                "in the type of goal `?2`",
                "in the type of `_15`"
            ]
        );
    }

    #[test]
    fn reads_metas_as_agda_prints_them() {
        let listed: Vec<OutputConstraint> = serde_json::from_value(serde_json::json!([
            { "constraintObj": { "name": "_17", "range": [] }, "kind": "OfType",
              "type": "_≡_ {ℕ} zero zero" },
            { "constraintObj": { "name": "_10", "range": [] }, "kind": "JustSort" },
        ]))
        .unwrap();
        assert_eq!(
            labels(&listed),
            HashMap::from([
                ("_17".to_string(), "_17 : _≡_ {ℕ} zero zero".to_string()),
                ("_10".to_string(), "Sort _10".to_string()),
            ])
        );
    }

    #[test]
    fn keeps_the_metas_agda_still_lists() {
        let mut metas = vec![meta("_1", 10, 15), meta("_B_2", 20, 21)];
        keep_listed(&mut metas, &[invisible("_B_2", "Set", 99, 100)]);
        assert_eq!(
            metas,
            [Meta {
                kind: Kind::OfType("Set".into()),
                ..meta("_B_2", 20, 21)
            }]
        );
    }
}
