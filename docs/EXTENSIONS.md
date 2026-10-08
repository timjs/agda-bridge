# Extensions: literate Agda in Typst

This document plans extensions beyond the phases of [`PLAN.md`](PLAN.md). The
first is literate Agda in Typst, files such as `Nat.lagda.typ`, with Typst
highlighting for the prose, Agda highlighting for the code blocks, and every
feature of the bridge inside those blocks. It records what Agda and Zed do
with such files, what was tested and how, the plan, and the decisions that are
still open. The sources, and how each was used, are listed at the end.

Paths of the extension's files are in zed-agda, and are marked as such, as in
"zed-agda's `extension.toml`"; all other paths are in this repository.

---

## 1. Summary

- Zed can run several language servers on one file, but it chooses them by the
  file's own language. A code block injected into another language gets
  highlighting, never a server of its own. That is no loss here, because Agda
  reads literate files itself: the bridge only has to be attached to the whole
  file.
- The plan is a new language in zed-agda, "Literate Agda (Typst)", for
  `.lagda.typ`. It parses the file with the Typst grammar, injects Agda into the
  blocks Agda checks, as one combined syntax layer, and attaches the bridge. A
  language setting, `scope_opt_in_language_servers`, keeps the bridge's
  completions, hover and code actions out of the prose.
- The bridge needs one change for these files: the features that read lines
  themselves, such as making a helper function, must not see the prose.
- Tinymist, the Typst language server, could serve the prose as a second
  server, but only through an entry of its own in zed-agda. This is optional.
- Without any code, a `.lagda.typ` file can already be opened as Agda through
  Zed's `file_types` setting, at the cost of the Typst highlighting.
- `PLAN.md` listed literate Agda as out of scope, needing "grammar injections
  and different offsets". The injections are needed, the different offsets are
  not: Agda reports positions in the literate file itself.

---

## 2. What Agda does with `.lagda.typ`

Tested with Agda 2.8.0.2 through `--interaction-json`, on a sample
`Nat.lagda.typ` with a heading, prose, two `agda` blocks, a goal `{! !}` in the
second, and a `haskell` block.

- Agda loads the file and skips the `haskell` block. It reports the goal at
  line 26, columns 12 to 17, which are the coordinates of the file itself, so
  positions need no translation between Zed and Agda.
- Agda reads `.lagda.typ` with its Markdown rules. Agda 2.8.0's
  `src/full/Agda/Syntax/Parser/Literate.hs` maps `.lagda.typ` to `literateMd`,
  because Typst and Markdown "use the same syntax for code blocks". A block
  starts at a line that matches ```` (.*)([[:space:]]*```(agda)?[[:space:]]*) ````,
  so **a bare ```` ``` ```` block, without a language, is Agda code too**. A
  block with another language, a line matching
  ```` [[:space:]]*```[a-zA-Z0-9-]*[[:space:]]* ````, is skipped. A block ends
  at a line that holds only ```` ``` ```` and spaces. A second sample confirmed
  the bare block: a bare block with the text `this is not Agda` gave
  `error: [MissingTypeSignature.Function]` at line 10.
- Agda's highlighting marks the prose, and the skipped blocks, as `background`,
  and the fence lines of its own blocks as `markup`. `src/highlight.rs` has no
  token for `background`, so the prose gets no semantic tokens, and maps
  `markup` to `comment`, so the fence lines are coloured as comments.
- `typst compile` (Typst 0.15.1) compiles the same file without errors, so it
  is valid Typst as well, in which the Agda blocks are raw blocks.

---

## 3. What Zed allows

Read in Zed's source at commit `4240146` (`main` on 8 October 2026), and
checked to be present in tag `v1.23.2` (commit `c019950`), the version
installed here.

1. **Which language a file gets.** `find_for_file` in
   `crates/language/src/available_languages.rs` scores every language by the
   length of its longest matching suffix. For `Nat.lagda.typ`, the suffix
   `lagda.typ` matches the whole file name, 13 characters, and Typst's `typ`
   only the extension, 3, so a language with `path_suffixes = ["lagda.typ"]`
   wins. A user's `file_types` setting wins over both.
2. **Which servers a file gets.** `language_server_ids_for_buffer` in
   `crates/project/src/lsp_store.rs` uses `buffer.language()`, the file's own
   language; injections never start servers. A running server is keyed by
   worktree, server name, toolchain and settings (`LanguageServerSeed`), not by
   language, so `.agda` and `.lagda.typ` files in one project share one bridge,
   and so one Agda.
3. **Combined injections.** `crates/language/src/syntax_map.rs` puts all
   matches of an `injection.combined` pattern for one language into one layer
   (`InjectionGroupKey::Combined`). All Agda blocks then form one Agda syntax
   tree, as Agda reads them.
4. **Keeping a server to some scopes.** A language's
   `scope_opt_in_language_servers` lists servers that are only allowed where an
   override in `overrides.scm` opts them in (`LanguageScope::language_allowed`
   in `crates/language/src/language.rs`). The scope at a position comes from
   the innermost syntax layer there (`language_scope_at` in
   `crates/language/src/buffer.rs`), which inside an injected block is Agda,
   whose configuration lists no opt-in servers. `lsp_store.rs` applies this
   filter to requests at a position, among them completions, hover, go to
   definition, references, code actions and signature help. So with
   `agda-bridge` in the new language's list, the bridge gets those requests
   inside Agda blocks only. Diagnostics and semantic tokens are for the whole
   file and are not affected, and the server still starts. Zed's own TSX
   language uses the same mechanism to keep Tailwind to strings
   (`crates/grammars/src/tsx/config.toml`). Zed's documentation lists the key
   but does not explain it yet (`docs/src/extensions/languages.md`).
5. **Comments and brackets.** Comment toggling (`toggle_comments` in
   `crates/editor/src/input.rs`) and closing brackets (`handle_input`, same
   file) also take their settings from `language_scope_at`. In Agda blocks they
   follow the Agda language, with `-- ` and no closing `_` or `*`; in the prose
   they follow the new language, with `// ` and Typst's closing `_` and `*`.
6. **Outline.** `outline_items_containing_internal` in `buffer.rs` runs the
   outline queries through `syntax.matches`, which visits every layer, so Typst
   headings and Agda definitions both appear. In tree-sitter-typst a `section`
   spans its heading and everything up to the next heading of the same level,
   so the Agda definitions should nest under their headings.
7. **Semantic tokens.** The rules are looked up by language name
   (`get_or_create_token_stylizer` in
   `crates/project/src/lsp_store/semantic_tokens.rs`), and extensions register
   them per language directory (`crates/extension_host/src/extension_host.rs`).
   The new language needs its own copy of `semantic_token_rules.json`, the
   third identical copy, and its own `semantic_tokens` setting.
8. **Shared grammars.** Grammars are registered by name, and when two
   extensions declare the same name, the last one registered wins
   (`extension_host.rs`). The Typst extension declares `typst` at commit
   `abe60cb`; declaring it at the same commit in zed-agda makes the order
   irrelevant, and the new language also works without the Typst extension.
9. **No servers of other extensions through settings.** Extension servers are
   registered per language (`register_language_server` in
   `crates/language_extension/src/extension_lsp_adapter.rs`), and the
   `language_servers` setting only picks among a language's own servers and
   Zed's built-in optional ones (`crates/project/src/manifest_tree/server_tree.rs`).
   So `"language_servers": ["agda-bridge", "tinymist"]` on the new language
   would not start Tinymist.
10. **Keymap context.** The context `extension` is the file's last extension
    (`crates/editor/src/editor.rs`), `typ` for `Nat.lagda.typ`. The README's
    bindings for `[ g` and `] g`, with `extension == agda`, do not apply to these
    files.
11. **`languageId`.** Zed sends the language name in lower case, unless the
    extension maps it with `language_ids` (`extension_lsp_adapter.rs`). The
    bridge ignores `languageId`.

Today, with the Typst extension installed, `Nat.lagda.typ` opens as Typst:
Tinymist runs, and the Typst extension's own `injections.scm` highlights every
`agda` block with tree-sitter-agda, but block by block, and nothing checks the
Agda.

---

## 4. The grammars, tested

A small Rust program, using `tree-sitter` 0.25.10 with both grammars compiled
from source at the commits Zed uses (tree-sitter-typst `abe60cb`,
tree-sitter-agda `e8d47a6`), parsed a sample with two `agda` blocks, a
`haskell` block and a bare block. It ran the injection query below and parsed
the Agda ranges as one tree, through `set_included_ranges`, as Zed does for a
combined injection.

The query, for zed-agda's `languages/literate-agda-typst/injections.scm`:

```scheme
; Agda reads ```agda blocks and bare ``` blocks as code, and all of them as
; one module, so they form one combined Agda layer.
((raw_blck
  lang: (ident) @_lang
  (blob) @injection.content)
  (#eq? @_lang "agda")
  (#set! injection.language "agda")
  (#set! injection.combined))

((raw_blck
  !lang
  (blob) @injection.content)
  (#set! injection.language "agda")
  (#set! injection.combined))

; Other blocks in their own language, as the Typst extension does.
((raw_blck
  lang: (ident) @injection.language
  (blob) @injection.content)
  (#not-eq? @injection.language "agda"))

((comment) @injection.content
  (#set! injection.language "comment"))
```

The results:

- The Typst tree has no errors.
- The query gives the two `agda` blocks and the bare block to Agda, and the
  `haskell` block to its own language.
- The combined Agda tree has no errors.

Typst's `highlights.scm` gives the content of every raw block the capture
`@text.literal`. Zed orders the captures of all layers by start, longer first,
then by layer depth (`sort_key` in `syntax_map.rs`), so Agda's captures nest
inside the literal one, whose colour would show between them. The new language
therefore uses the Typst extension's `highlights.scm` with one rule replaced:

```scheme
(raw_blck
  "```" @punctuation.delimiter @embedded)

; Only blocks Agda skips are literal text; the literal colour would show
; between the tokens of the injected Agda layer.
((raw_blck
  lang: (ident) @_lang
  (blob) @text.literal)
  (#not-eq? @_lang "agda"))
```

All of these compile against the grammars: the Typst extension's six queries
(`brackets`, `highlights`, `indents`, `injections`, `outline`, `overrides`), the
injection query, the changed highlights, and zed-agda's three Agda queries
(`brackets`, `highlights`, `outline`, from branch `use-agda-bridge`).

---

## 5. What the bridge needs

Already right:

- `is_agda_source` in `src/server.rs` accepts `.lagda.typ`.
- Loading, goals, diagnostics, hover, go to definition, renaming and semantic
  tokens work with Agda's positions, which are the file's own.
- Expanding `?` to `{!  !}` after a load (`expand_question_marks` in
  `src/server.rs`) only touches goals that Agda reported, never the prose.

Wrong in literate files are the features that read lines themselves:

- `src/helper.rs` scans upward from the goal for its definition. When the
  signature is in an earlier block, it walks through the ```` ```agda ```` fence,
  a line at depth 0 that is no boundary, into the prose, and may put the helper
  function there.
- `src/clause.rs` (`signature_at`) reads a signature over several lines, and
  can run into a fence or prose at the edge of a block.
- `src/goals.rs` (`case_split`, `add_with`) reads the goal's line and its
  neighbours; the risk there is smaller.
- `src/input.rs` completes `\` and `#` anywhere. Zed's scope filter keeps this
  out of the prose in the new language, but not in the fallback of section 7,
  and not in other editors.

Agda itself solves this by turning the prose into spaces before it parses. The
bridge can do the same.

---

## 6. Plan

### Step 1, zed-agda: the language "Literate Agda (Typst)"

On top of branch `use-agda-bridge`, a new directory
`languages/literate-agda-typst/`:

| File | Content |
| --- | --- |
| `config.toml` | the Typst extension's settings (comments, brackets, lists, tab size), with the lines below |
| `injections.scm` | the query of section 4 |
| `highlights.scm` | the Typst extension's, with the change of section 4 |
| `brackets.scm`, `indents.scm`, `outline.scm`, `overrides.scm` | the Typst extension's, unchanged |
| `semantic_token_rules.json` | a copy of [`zed/semantic_token_rules.json`](../zed/semantic_token_rules.json) |

```toml
name = "Literate Agda (Typst)"
grammar = "typst"
path_suffixes = ["lagda.typ"]
line_comments = ["// "]
scope_opt_in_language_servers = ["agda-bridge"]
```

zed-agda's `extension.toml` gets the grammar and the language:

```toml
[grammars.typst]
repository = "https://github.com/uben0/tree-sitter-typst"
rev = "abe60cbed7986ee475d93f816c1be287f220c5d8"

[language_servers.agda-bridge]
name = "Agda Bridge"
languages = ["Agda", "Literate Agda (Typst)"]
```

zed-agda's `src/lib.rs` needs no change. Further:

- Credit the Typst extension, Apache-2.0 like zed-agda, for the copied queries.
- In the README, the setting for Agda's highlighting in the new language, and
  the keymap context `extension == typ`, which also covers plain Typst files:

  ```json
  "languages": {
    "Literate Agda (Typst)": { "semantic_tokens": "combined" }
  }
  ```

- A unit test in zed-agda that compares its two copies of
  `semantic_token_rules.json`; keeping them identical to this repository's copy
  stays a manual step, as now.

To check in Zed, with the dev extension:

- `Nat.lagda.typ` gets "Literate Agda (Typst)", a plain `.typ` file stays Typst.
- The prose is highlighted as Typst, `agda` and bare blocks as Agda, other
  blocks in their own language.
- Saving loads the file in Agda; goals, hover and code actions work in blocks.
- `\` and `#` open the symbol menu in blocks, not in prose; hover in prose
  shows nothing from the bridge.
- Comment toggling gives `-- ` in blocks and `// ` in prose.
- The outline shows the headings with the Agda definitions under them.
- A `.lagda.typ` module and an `.agda` module can import each other, through
  the one bridge.

### Step 2, the bridge: literate files

- A new module, `src/literate.rs`, ports Agda's `literateMd` for `.lagda.md`
  and `.lagda.typ`: it returns the text with every character outside code
  replaced by a space, keeping the line breaks, so every offset stays the same.
  Fences and prose become blank lines, so scanning upward goes from a block
  into the block before it, as Agda reads them.
- `clause.rs`, `helper.rs`, `goals.rs` (`case_split`, `add_with`) and
  `input.rs` read that text instead of the document; `input.rs` then offers no
  symbols outside code.
- Decide whether fences keep the `comment` token, which `markup` gets in
  `src/highlight.rs`.
- The other formats, `.lagda.tex`, `.lagda.rst`, `.lagda.org` and
  `.lagda.tree`, can follow later by porting their rules from the same file of
  Agda.
- Tests: a fixture `tests/fixtures/Literate.lagda.typ` with prose, an `agda`
  block with a signature, a later block with its clause and a goal, a `haskell`
  block and a bare block. End-to-end tests that a load gives the goals on the
  right lines, that a helper function lands in code when the signature is in
  an earlier block, that "Make clause" and a case split work in a block, and
  that prose gets no symbol completions. Unit tests for `literate.rs` with the
  cases of Agda's rules.

### Step 3, optional: Tinymist for the prose

- zed-agda's `extension.toml` gets a second server for "Literate Agda (Typst)",
  under a name of its own, such as `tinymist-literate`, since server names are
  global and the Typst extension already registers `tinymist`; with
  `language_ids = { "Literate Agda (Typst)" = "typst" }`.
- zed-agda's `src/lib.rs` starts `tinymist` from `PATH`, where Homebrew puts
  it, or from `lsp.tinymist.binary.path`, and passes on the settings under
  `lsp.tinymist`, so one Tinymist configuration serves both languages.
- zed-agda's `languages/agda/config.toml` gets
  `scope_opt_in_language_servers = ["tinymist-literate"]`, which keeps
  Tinymist out of the Agda blocks, as the bridge is kept out of the prose.
- To find out in Zed before relying on it: how Zed combines the semantic tokens
  of two servers on one file, and if they clash, switching Tinymist's off with
  its `semanticTokens` setting; and whether Tinymist's formatter leaves raw
  blocks alone.

---

## 7. Without code: open the file as Agda

```json
"file_types": { "Agda": ["*.lagda.typ"] },
"languages": { "Agda": { "semantic_tokens": "full" } }
```

`file_types` wins over the suffixes of every extension (section 3, point 1).
The bridge then works on the whole file, but tree-sitter-agda also parses the
prose, so `full` is needed to hide its highlighting there, which leaves the
prose without any, since Agda gives it no tokens; `full` also applies to
`.agda` files. `\` and `#` open the symbol menu in the prose too.

---

## 8. Open decisions

1. **Bare blocks.** The request was for ```` ```agda ```` blocks only, but
   Agda also checks bare blocks. Proposal: highlight them as Agda too, so the
   editor shows what Agda checks.
2. **The name.** "Literate Agda (Typst)" leaves room for "Literate Agda
   (Markdown)" with Zed's built-in Markdown grammar. The name is typed in
   settings.
3. **Tinymist**, step 3: yes or no.
4. **The branch.** Step 1 builds on zed-agda's `use-agda-bridge`, not `master`.

---

## 9. Checks and their results

| Check | Result |
| --- | --- |
| Agda 2.8.0.2 loads the sample `Nat.lagda.typ` | goal `?0 : ℕ` at line 26, columns 12 to 17; the `haskell` block is skipped |
| Agda's highlighting of that file | `background` for the prose and the `haskell` block, `markup` for the fences of the `agda` blocks |
| Agda on a bare block | `error: [MissingTypeSignature.Function]` at line 10 |
| `typst compile` on the sample | a PDF, no errors or warnings |
| The injection query on a sample with every kind of block | three Agda ranges (two `agda`, one bare), one `haskell`; the Typst tree and the combined Agda tree have no errors |
| The queries compile against the grammars | Typst extension 6 of 6, injections 1 of 1, changed highlights 1 of 1 (65 patterns, the original has 64), zed-agda's Agda queries 3 of 3 |
| Typst sections | a level-1 section spans its level-2 section and both Agda blocks under them |
| Zed `v1.23.2` has the code read on `main` | the suffix rule, the scope filter, combined injections and `language_scope_at` are all present |

Nothing was checked in a running Zed yet; that is the checklist of step 1.

---

## 10. Sources and how they were used

| Source | What I took from it |
| --- | --- |
| Agda 2.8.0.2 on this machine, through `--interaction-json` | how Agda treats `.lagda.typ`: positions, skipped blocks, bare blocks, highlighting |
| [agda/agda](https://github.com/agda/agda) at tag `v2.8.0` (`src/full/Agda/Syntax/Parser/Literate.hs`) | the exact rules for code blocks in `.lagda.md` and `.lagda.typ`, to port in step 2 |
| [zed-industries/zed](https://github.com/zed-industries/zed) at commit `4240146` and tag `v1.23.2` (the files named in section 3) | every statement about Zed, read in code rather than recalled |
| [zed-industries/extensions](https://github.com/zed-industries/extensions) (`extensions.toml`, `.gitmodules`) | which repositories hold the Typst and Agda extensions |
| [WeetHet/typst.zed](https://github.com/WeetHet/typst.zed) at commit `289ce59` (`extension.toml`, `languages/typst/*`, `src/typst.rs`, `LICENSE`) | the grammar commit, the queries to reuse, how it starts Tinymist, its licence |
| [uben0/tree-sitter-typst](https://github.com/uben0/tree-sitter-typst) at commit `abe60cb` (`grammar.js`, `src/`) | the `raw_blck` node, and the grammar for the tests |
| [tree-sitter/tree-sitter-agda](https://github.com/tree-sitter/tree-sitter-agda) at commit `e8d47a6` | the grammar for the tests, at zed-agda's commit |
| This repository (`src/server.rs`, `highlight.rs`, `helper.rs`, `clause.rs`, `goals.rs`, `input.rs`, `zed/semantic_token_rules.json`) | what already works in literate files, and what reads lines itself |
| zed-agda, branch `use-agda-bridge` (`extension.toml`, `src/lib.rs`, `languages/agda/*`) | where the new language goes, and the Agda queries tested in section 4 |
| Typst 0.15.1 and Tinymist from Homebrew on this machine | that the literate file is valid Typst, and that Tinymist is on `PATH` for step 3 |
