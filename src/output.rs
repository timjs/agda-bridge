//! The output file: a Markdown stand-in for Emacs's `*Agda information*`
//! buffer, rewritten after every command so Zed reloads it in place.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Output {
    pub path: PathBuf,
    shown: AtomicBool,
}

impl Output {
    pub fn new(path: PathBuf) -> Output {
        Output {
            path,
            shown: AtomicBool::new(false),
        }
    }

    /// Replace the file's contents. Returns true the first time, so the caller
    /// can ask Zed to open the file once.
    pub async fn write(&self, title: &str, source: Option<&Path>, body: &str) -> io::Result<bool> {
        let mut text = String::from(
            "<!-- Written by agda-bridge after every Agda command. Edits here are overwritten. -->\n\n",
        );
        text.push_str(&format!("# {title}\n\n"));
        if let Some(source) = source {
            let name = source.file_name().unwrap_or_default().to_string_lossy();
            text.push_str(&format!("*{name}*\n\n"));
        }
        text.push_str(body);

        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // Write a sibling file and rename it over the old one, so Zed never
        // reads a half-written file.
        let temporary = self.path.with_extension("md.tmp");
        tokio::fs::write(&temporary, text).await?;
        tokio::fs::rename(&temporary, &self.path).await?;
        Ok(!self.shown.swap(true, Ordering::SeqCst))
    }

    /// A Markdown link target for `file`, relative to the output file's own
    /// directory, as Zed's Markdown preview resolves links, with `:line:column`
    /// after it, which the preview opens at that place. Characters that would
    /// end the link or start a fragment are percent-encoded; the preview
    /// decodes them.
    pub fn link_to(&self, file: &Path, line: u64, column: u64) -> String {
        let from = self.path.parent().unwrap_or(Path::new(""));
        format!("{}:{line}:{column}", encode(&relative(from, file)))
    }

    /// Create the file if no command has written it yet, so that opening it
    /// shows a file instead of an empty unsaved buffer. Counts as shown.
    pub async fn ensure_exists(&self) -> io::Result<()> {
        if !tokio::fs::try_exists(&self.path).await.unwrap_or(false) {
            self.write(
                "Agda",
                None,
                "No output yet. Save an Agda file to load it.\n",
            )
            .await?;
        }
        self.shown.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// `to` relative to the directory `from`, with `..` where needed; `to`
/// itself when the two share nothing, as on different volumes.
fn relative(from: &Path, to: &Path) -> String {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return to
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .into_owned();
    }
    let ups = std::iter::repeat_n("..".to_string(), from.len() - common);
    let downs = to[common..]
        .iter()
        .map(|part| part.as_os_str().to_string_lossy().into_owned());
    ups.chain(downs).collect::<Vec<_>>().join("/")
}

/// Percent-encode what would end a Markdown link or start a fragment.
fn encode(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '%' => "%25".to_string(),
            '(' => "%28".to_string(),
            ')' => "%29".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '#' => "%23".to_string(),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_relative_to_the_output_file() {
        let output = Output::new(PathBuf::from("/p/My Proofs/.zed/agda-output.md"));
        assert_eq!(
            output.link_to(
                Path::new("/p/My Proofs/Language/Judgement.lagda.typ"),
                119,
                21
            ),
            "../Language/Judgement.lagda.typ:119:21"
        );
        assert_eq!(
            output.link_to(Path::new("/p/My Proofs/A (old)/B#1.agda"), 3, 1),
            "../A%20%28old%29/B%231.agda:3:1"
        );
        assert_eq!(
            output.link_to(Path::new("/p/My Proofs/.zed/X.agda"), 1, 1),
            "X.agda:1:1"
        );
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/c/d.agda")),
            "../../c/d.agda"
        );
    }
}
