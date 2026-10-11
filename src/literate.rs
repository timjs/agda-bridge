//! Literate Agda: which parts of a file Agda reads as code. Features that
//! read the text themselves, such as adding a clause or placing a helper
//! function, read [`code`] instead, the text with everything else blanked,
//! as Agda's own `illiterate` does before it parses. Every character keeps
//! its offset, so positions in it are positions in the file.
//!
//! Ported from Agda 2.8.0's `src/full/Agda/Syntax/Parser/Literate.hs`, for
//! `.lagda.md` and `.lagda.typ`, which Agda both reads with its Markdown rules
//! (`literateMd`). Other files are returned as they are, which is right for
//! `.agda`; the other literate formats (`.lagda`, `.lagda.tex`, `.lagda.rst`,
//! `.lagda.org`, `.lagda.tree`) are not ported yet.

use std::borrow::Cow;
use std::path::Path;

/// The extensions Agda reads with its Markdown rules.
const MARKDOWN_LIKE: &[&str] = &[".lagda.md", ".lagda.typ"];

/// Where a Markdown-like file is, at the start of a line.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Prose,
    /// In a block Agda reads as code: ```` ```agda ```` or a bare ```` ``` ````.
    Agda,
    /// In a block in another language, which Agda skips.
    Other,
}

/// The text of the file at `path` as Agda reads it: code as it is, every
/// other character a space, except line breaks and other white space.
pub fn code<'a>(path: &Path, text: &'a str) -> Cow<'a, str> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if !MARKDOWN_LIKE
        .iter()
        .any(|extension| name.ends_with(extension))
    {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut state = State::Prose;
    // Lines with their line break, as Agda's `caseLine` takes them.
    for line in text.split_inclusive('\n') {
        let is_code = match state {
            State::Prose => {
                if begins_agda(line) {
                    state = State::Agda;
                } else if begins_other(line) {
                    state = State::Other;
                }
                false
            }
            State::Agda if ends(line) => {
                state = State::Prose;
                false
            }
            State::Agda => true,
            State::Other => {
                if ends(line) {
                    state = State::Prose;
                }
                false
            }
        };
        match is_code {
            true => out.push_str(line),
            false => out.extend(line.chars().map(bleach)),
        }
    }
    Cow::Owned(out)
}

/// White space in Agda's regular expressions: POSIX's `[[:space:]]`, which
/// regex-tdfa defines as these six characters.
fn is_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0B' | '\x0C' | '\r' | ' ')
}

/// Agda's `md_begin`, `(.*)([[:space:]]*```(agda)?[[:space:]]*)`: a line
/// that ends, before white space, in ```` ``` ```` or ```` ```agda ````,
/// whatever comes before it.
fn begins_agda(line: &str) -> bool {
    let line = line.trim_end_matches(is_space);
    line.ends_with("```") || line.ends_with("```agda")
}

/// Agda's `md_begin_other`, `[[:space:]]*```[a-zA-Z0-9-]*[[:space:]]*`: a
/// fence with another language.
fn begins_other(line: &str) -> bool {
    line.trim_matches(is_space)
        .strip_prefix("```")
        .is_some_and(|language| {
            language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// Agda's `md_end`, `([[:space:]]*```[[:space:]]*)`: a bare fence.
fn ends(line: &str) -> bool {
    line.trim_matches(is_space) == "```"
}

/// Agda's `bleach`: white space stays, except a tab, and every other
/// character becomes a space.
fn bleach(c: char) -> char {
    match c.is_whitespace() && c != '\t' {
        true => c,
        false => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typ(text: &str) -> String {
        code(Path::new("A.lagda.typ"), text).into_owned()
    }

    /// The lines of `text` that are code, by their numbers.
    fn code_lines(text: &str) -> Vec<usize> {
        typ(text)
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(number, _)| number)
            .collect()
    }

    #[test]
    fn keeps_agda_blocks_and_blanks_the_rest() {
        let text = "= Title\n\n```agda\nmodule A where\n```\n\nProse: x.\n";
        assert_eq!(
            typ(text),
            "       \n\n       \nmodule A where\n   \n\n         \n"
        );
    }

    #[test]
    fn keeps_every_offset() {
        let text = "Prosa → ℕ\t𝔹\r\n```agda\r\nx : ℕ\r\n```\r\nend";
        let code = typ(text);
        assert_eq!(code.chars().count(), text.chars().count());
        let breaks = |s: &str| -> Vec<usize> {
            s.chars()
                .enumerate()
                .filter(|(_, c)| *c == '\n' || *c == '\r')
                .map(|(i, _)| i)
                .collect()
        };
        assert_eq!(breaks(&code), breaks(text));
        assert!(code.contains("\nx : ℕ\r\n"), "{code:?}");
    }

    #[test]
    fn reads_bare_blocks_as_agda_and_skips_other_languages() {
        let text = "```\nx : Set\n```\n```haskell\nfoo = 1\n```\n```agda-like\ny\n```\n";
        assert_eq!(code_lines(text), [1]);
    }

    #[test]
    fn follows_agdas_rules_at_the_edges() {
        // A fence at the end of a line of prose opens a block.
        assert_eq!(code_lines("Code: ```\nx\n```\n"), [1]);
        // Fences may be indented and followed by white space.
        assert_eq!(code_lines("  ```agda  \nx\n  ```  \n"), [1]);
        // `agda` is case sensitive, and a space before it makes prose.
        assert_eq!(code_lines("```Agda\nx\n```\n``` agda\ny\n"), [0usize; 0]);
        // Only a bare fence closes a block, also inside another language.
        assert_eq!(code_lines("```agda\nx\n```agda\n```\ny\n"), [1, 2]);
        assert_eq!(code_lines("```haskell\n```agda\nx\n```\n"), [0usize; 0]);
        // An unclosed block runs to the end, also without a final break.
        assert_eq!(code_lines("```agda\nx\ny"), [1, 2]);
    }

    #[test]
    fn leaves_other_files_alone() {
        let text = "Prose\n```agda\nx\n```\n";
        for name in ["A.agda", "A.lagda.tex", "A.typ", "A.md"] {
            assert!(
                matches!(code(Path::new(name), text), Cow::Borrowed(_)),
                "{name}"
            );
        }
        assert_eq!(
            code(Path::new("A.lagda.md"), text),
            "     \n       \nx\n   \n"
        );
    }
}
