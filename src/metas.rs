//! Unsolved metas: what Agda could not solve and did not make a goal, such as
//! an implicit argument nothing determines, or a `_` in the source. After a
//! load Agda lists them as `invisibleGoals`, with a name, a type and a range,
//! as it lists goals. The bridge keeps them like goals and names, moving them
//! along with edits, so that they can be diagnostics and be explained in
//! hover.

use std::collections::HashSet;

use crate::protocol::OutputConstraint;
use crate::text::Change;

/// An unsolved meta, at a half-open range of 0-based char offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// Agda's name for it: `_12`, or `_m_12` for an implicit argument `m`.
    pub name: String,
    pub ty: String,
    pub start: usize,
    pub end: usize,
}

impl Meta {
    /// The implicit argument this meta stands for, which Agda names it
    /// after: `m` for `_m_12`, none for `_12`.
    pub fn argument(&self) -> Option<&str> {
        let (argument, number) = self.name.strip_prefix('_')?.rsplit_once('_')?;
        (!argument.is_empty() && number.parse::<u64>().is_ok()).then_some(argument)
    }

    /// The meta as a diagnostic says it.
    pub fn message(&self) -> String {
        let meta = format!("Unsolved meta {} : {}", self.name, self.ty);
        match self.argument() {
            Some(argument) => format!("{meta}, the implicit argument {argument}"),
            None => meta,
        }
    }
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
                ty: goal.ty.clone().unwrap_or_else(|| "_".into()),
                start: position("start")?,
                end: position("end")?,
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

/// Keep only the metas that Agda still lists as unsolved, with their types
/// as Agda now gives them. After a goal command Agda lists the metas again,
/// but their ranges are those of the last load, which edits have moved
/// since: so the bridge keeps its own ranges, and only drops the metas the
/// command solved. Metas the command made appear at the next load.
pub fn keep_listed(metas: &mut Vec<Meta>, listed: &[OutputConstraint]) {
    let types: Vec<(String, Option<String>)> = listed
        .iter()
        .filter_map(|goal| {
            let name = goal.constraint_obj.get("name")?.as_str()?;
            Some((name.to_string(), goal.ty.clone()))
        })
        .collect();
    let names: HashSet<&str> = types.iter().map(|(name, _)| name.as_str()).collect();
    metas.retain(|meta| names.contains(meta.name.as_str()));
    for meta in metas.iter_mut() {
        if let Some((_, Some(ty))) = types.iter().find(|(name, _)| *name == meta.name) {
            meta.ty = ty.clone();
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
            ty: "ℕ".into(),
            start,
            end,
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
                Meta {
                    ty: "ℕ".into(),
                    ..meta("_m_15", 215, 220)
                },
                Meta {
                    ty: "Set".into(),
                    ..meta("_18", 299, 300)
                },
            ]
        );
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
        assert_eq!(meta("_m_15", 0, 1).argument(), Some("m"));
        assert_eq!(meta("_A_7", 0, 1).argument(), Some("A"));
        assert_eq!(meta("_x₁_7", 0, 1).argument(), Some("x₁"));
        assert_eq!(meta("_18", 0, 1).argument(), None);
        assert_eq!(
            meta("_m_15", 0, 1).message(),
            "Unsolved meta _m_15 : ℕ, the implicit argument m"
        );
        assert_eq!(meta("_18", 0, 1).message(), "Unsolved meta _18 : ℕ");
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
    fn keeps_the_metas_agda_still_lists() {
        let mut metas = vec![meta("_1", 10, 15), meta("_B_2", 20, 21)];
        keep_listed(&mut metas, &[invisible("_B_2", "Set", 99, 100)]);
        assert_eq!(
            metas,
            [Meta {
                ty: "Set".into(),
                ..meta("_B_2", 20, 21)
            }]
        );
    }
}
