# Extensions

This document plans extensions beyond the phases of [`PLAN.md`](PLAN.md),
each in a part of its own:

- [Literate Agda in Typst](#literate-agda-in-typst): files such as
  `Nat.lagda.typ`, with Typst highlighting for the prose, Agda highlighting
  for the code blocks, and every feature of the bridge inside those blocks.
- [Unsolved metas](#unsolved-metas): what Agda knows about the metas it could
  not solve, shown where they are, in hover and as diagnostics.

Each part records what Agda and Zed do, what was tested and how, the plan, and
the decisions that are still open, with the sources, and how each was used, at
its end.

Paths of the extension's files are in zed-agda, and are marked as such, as in
"zed-agda's `extension.toml`"; all other paths are in this repository.

---

## Literate Agda in Typst

This part is about literate Agda in Typst, files such as `Nat.lagda.typ`, with
Typst highlighting for the prose, Agda highlighting for the code blocks, and
every feature of the bridge inside those blocks.

---

### 1. Summary

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

### 2. What Agda does with `.lagda.typ`

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

### 3. What Zed allows

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

### 4. The grammars, tested

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

### 5. What the bridge needs

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
- `src/goals.rs` (`case_split`, `add_with`) reads the goal's line. Step 2
  found that this is no problem: Agda only puts goals in code, and these
  functions read nothing but the goal's own line.
- `src/input.rs` completes `\` and `#` anywhere. Zed's scope filter keeps this
  out of the prose in the new language, but not in the fallback of section 7,
  and not in other editors.
- Found in step 2: the files that a rename lists as possibly using the name,
  because they import its module, were found by the module's name taken from
  `file_stem()`, which is `Nat.lagda` for `Nat.lagda.typ`, so imports of a
  literate module were never found. That scan also read the prose.

Agda itself solves this by turning the prose into spaces before it parses. The
bridge can do the same.

---

### 6. Plan

#### Step 1, zed-agda: the language "Literate Agda (Typst)"

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

#### Step 2, the bridge: literate files (done)

Built on branch `literate-typst`:

- A new module, `src/literate.rs`, ports Agda's `literateMd` for `.lagda.md`
  and `.lagda.typ`. Its `code` returns the text with every character outside
  code replaced by a space, as Agda's `bleach` does, keeping line breaks, so
  every offset stays the same. Fences and prose become blank lines, so
  scanning upward goes from a block into the block before it, as Agda reads
  them. Agda's regular expressions are ported as plain string tests; their
  `[[:space:]]` is regex-tdfa's `"\t\n\v\f\r "`, which is not Rust's
  `is_ascii_whitespace`, since that leaves out `\v`. Other files, `.agda` and
  the literate formats not ported yet, are returned as they are.
- `src/server.rs` reads that text for "Make clause" (the code action and the
  command), for placing a helper function, and for symbol completion, which so
  offers nothing outside code. `case_split` and `add_with` keep reading the
  document, see section 5.
- A new `module_name` strips the whole extension, `Nat` for `Nat.lagda.typ`,
  and is used both by `is_agda_source` and by the rename's search for files
  that import the module, which now also reads only their code.
- Decided: fences keep the `comment` token that Agda's `markup` gets in
  `src/highlight.rs`.
- The other formats, `.lagda.tex`, `.lagda.rst`, `.lagda.org` and
  `.lagda.tree`, can follow later by porting their rules from the same file of
  Agda.

Tests:

- Unit tests in `src/literate.rs` for Agda's rules: `agda`, bare and other
  blocks; a fence at the end of a line of prose, indented fences, `Agda` and
  `` ``` agda `` as prose, `` ```agda `` inside a block, an unclosed block;
  offsets and line breaks kept, also with `\r\n`, tabs and characters outside
  the Basic Multilingual Plane; other files left alone.
- A unit test in `src/server.rs` for `module_name`.
- An end-to-end test, `works_in_literate_typst` in `tests/lsp.rs`, on the new
  fixture `tests/fixtures/Literate.lagda.typ`, whose prose has a line that
  reads like a signature of `double` and a `\to`. It checks that the goals are
  on the right lines, that the `agda` fences are comments and the prose and
  the `haskell` block have no tokens, that `\to` completes in code and not in
  prose, that "Make clause" is offered on a signature in a block and not on the
  prose, that the helper function goes above `double`'s signature in the block
  before, that Agda accepts the result, and that a case split and "Make
  clause" stay in their blocks.

#### Step 3, optional: Tinymist for the prose

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

### 7. Without code: open the file as Agda

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

### 8. Decisions

1. **Bare blocks.** The request was for ```` ```agda ```` blocks only, but
   Agda also checks bare blocks. Taken in step 1: they are highlighted as Agda
   too, so the editor shows what Agda checks.
2. **The name.** Taken in step 1: "Literate Agda (Typst)", which leaves room
   for "Literate Agda (Markdown)" with Zed's built-in Markdown grammar. The
   name is typed in settings.
3. **Tinymist**, step 3: yes or no. Still open.
4. **The branch.** Step 1 is on zed-agda's branch `literate-typst`, from
   `use-agda-bridge`; step 2 on this repository's branch `literate-typst`.
5. **Fences.** Taken in step 2: they keep the `comment` token.

---

### 9. Checks and their results

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

| Step 2: the end-to-end test against Agda 2.8.0.2 | passes |
| Step 2: the end-to-end test with each fix undone in turn | fails at its own check: without it, completion offers `→` for `\to` in prose; "Make clause" is offered on the prose line; the helper function goes into the prose, between two blocks, where Agda would not see it |
| Step 2: `module_name` with `file_stem()` instead | the test fails with `Nat.lagda` for `Nat.lagda.typ` |

Nothing was checked in a running Zed yet; that is the checklist of step 1.

---

### 10. Sources and how they were used

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

---

## Unsolved metas

Agda colours a meta it could not solve yellow, but the bridge said little more
about it: the output file listed its name and type, without saying where it
is, and hover showed nothing. This part plans what more the bridge can show,
from what Agda knows about these metas and what other editors do with it.

---

### 1. Summary

- After a load, Agda reports every unsolved meta with its name, its type and
  its range, the same as for a goal. A meta for an implicit argument is named
  after that argument, `_m_304` for an implicit `m`. On request,
  `Cmd_constraints` gives the constraints, each with the meta it is blocked
  on. Agda has no command that gives a meta's context: that needs a goal.
- Emacs lists the metas with clickable locations; banacorn's VS Code extension
  lists them with buttons that jump to them; agda2-vscode, the bridge's model,
  lists them without locations. None explains a meta where it is.
- The plan has six items. This branch builds the first three of the
  recommended order: hover on a meta (item 1), metas as diagnostics of
  severity warning (item 2), and the error about unsolved constraints on the
  code it is about instead of at the start of the file (item 6).

---

### 2. What Agda gives

Tested with Agda 2.8.0.2 through `--interaction-json`, on a file with an
implicit argument nothing determines (`const (suc zero)`, with
`const : {m : ℕ} → ℕ → ℕ`), an underscore nothing determines (`suc _`), and
a constraint stuck on an underscore (`stuck : pred _ ≡ zero`,
`stuck = refl`).

- `Cmd_load` answers with `AllGoalsWarnings`, whose `invisibleGoals` are the
  unsolved metas: `_m_15 : ℕ` on `const`, `_18 : ℕ` on the `_` of `two`, and
  `_21 : ℕ` on the `_` of `stuck`. Each has a name (`constraintObj.name`), a
  type and a range, in the same form as a goal's. The bridge kept the name and
  type for the output file, and dropped the range.
- A meta for an implicit argument is named `_<argument>_<number>`, and its
  range is the name of the function, `const`; for an operator, the operator
  part, `⊕` in `zero ⊕ suc zero`.
- The stuck constraint makes Agda add an error, `[UnsolvedConstraints]`, with
  the constraint as Agda prints it, `pred _21 = zero : ℕ (blocked on _21)`,
  but without a location: the bridge put it at line 1, column 1. Agda's
  highlighting marks the code it is about, `refl`, as `unsolvedconstraint`.
- `Cmd_constraints` answers with `Constraints`, a list of constraints as
  objects: the constraint by kind (`CmpInType` with `constraintObjs`
  `["pred _21", "zero"]`, `comparison` `CmpEq` and `type` `ℕ`), its range,
  here empty, and what it is blocked on (`UnblockOnMeta` with the meta's
  number, 21). Agda's own printer for these, the `Pretty` instance of
  `OutputConstraint` in `src/full/Agda/Interaction/BasicOps.hs`, can be
  ported as it is.
- A goal's context, from `Cmd_goal_type_context`, lists the constraints that
  mention the goal as text already (`outputForms`, such as
  `_22 := refl (blocked on problem 31)`), so goal hovers need no change.
- Every command that gives a context, such as `Cmd_goal_type_context`, takes a
  goal number (`InteractionId`); none takes a meta. To see a meta's context,
  it has to become a goal: `?` in place of a `_`, or `{m = ?}` for an implicit
  argument.
- Agda's highlighting for these ranges has an empty `note`, so it has no text
  to show.

---

### 3. What other editors do

| Editor | Unsolved metas | Constraints | Other |
| --- | --- | --- | --- |
| Emacs (`agda2-mode`) | `*All Goals*` lists `_13 : ℕ  [ at …/Metas.agda:15.8-9 ]`; that buffer is in `compilation-mode`, so the location is clickable and `next-error` steps through them | `C-c C-=` | `C-c C-x C-h` shows implicit arguments; a goal's context lists the constraints mentioning it; no hover |
| VS Code, banacorn's `agda-mode-vscode` | the panel lists the metas, each with a button that jumps to it | "Show constraints", at four levels of normalisation | toggles for implicit and irrelevant arguments |
| VS Code, `agda2-vscode` | "Unsolved Metas (not in scope)" with name and type only, no location | "Show constraints" | toggle for implicit arguments |

---

### 4. Plan

1. **Hover on a meta.** The bridge keeps the metas of each load, and moves them
   along with edits, as it does goals and names; an edit that touches a meta
   removes it until the next load. Hover anywhere in a meta's range shows its
   name and type, which implicit argument it is or how to make it a goal, and
   the constraints on it, from `Cmd_constraints`, which the bridge asks after
   a load that left metas. Below that comes the usual hover for the name under
   the cursor.
2. **Metas as diagnostics,** of severity warning. They appear in Zed's
   project diagnostics (`cmd-shift-m`), inline, and in the jumps between
   diagnostics of severity warning. Goals stay information, so jumps between
   goals still find goals only.
3. **Locations in the output file:** a list of metas of its own, with a
   location after each, as in Emacs.
4. **Code actions "Show constraints" and "Show implicit arguments",** as
   Emacs's `C-c C-=` and `C-c C-x C-h`. Dropped for metas, see decision 6:
   their hover shows both.
5. **A code action "Make goal" on a meta:** `?` in place of `_`, and
   `{m = ?}` after the name of a function applied in prefix form; not for
   operators.
6. **The error about unsolved constraints on its code.** An
   `[UnsolvedConstraints]` error without a location goes on the ranges Agda
   highlights as `unsolvedconstraint`, instead of at line 1, column 1.

Built on this branch: 1, 2 and 6, see section 6; item 1 then grew to
include what item 4 would have shown about a meta; then 3 and 5. For goals,
what item 4 could add is a to-do in `tests/fixtures/TODO.md`.

---

### 5. Decisions

1. **Severity of the meta diagnostics:** warning, like Agda's yellow.
2. **The branch:** `unsolved-metas`, from `literate-typst`, since the files
   in use are literate and this document is on that branch.
3. **Metas on a goal are left out.** `with {!  !}` leaves two metas on its
   goal, the goal's type and that type's sort, which Agda doesn't know yet.
   They are no separate problem: the goal shows its type, as `?2 : _15`, and
   filling it solves them. Agda never checks a goal's text, so a meta within a
   goal is always such a one.
4. **Metas are read at a load.** After a goal command Agda lists the metas
   again, but with the ranges of the last load, which edits have moved since.
   So the bridge keeps its own ranges, and only drops the metas the command
   solved; metas the command made appear at the next save.
5. **A constraint is on a meta** when it is blocked on it, or mentions it.
   Constraints name metas by number, `_304`; the bridge uses the names the
   diagnostics use, `_m_304`.
6. **Everything in the hover, no code actions.** Asked to show as much as
   possible in a meta's hover without extra code actions, if that doesn't
   take long, the bridge adds what Agda says with implicit arguments shown,
   what a binder's meta is, and where else a meta occurs. Asking Agda with
   implicit arguments takes four more commands after a load that left metas,
   each about a millisecond, also on `Judgement.lagda.typ`, which takes 5.9
   seconds to load. Normal forms are left out: Agda already gives a meta's
   type reduced (`_ : F zero` comes as `ℕ`), so `Cmd_metas Normalised` gave
   the same types, on the fixture and on `Judgement.lagda.typ`.

---

### 6. What was built

On branch `unsolved-metas`:

- A new module, `src/metas.rs`, keeps the metas of a load: name, kind and
  range. The kind is Agda's: a term of a type (`OfType`), or itself a type or
  a sort (`JustType`, `JustSort`), printed as Agda prints them: `_15 : _14`,
  `Type _3`, `Sort _14`. Metas move along with edits as goals and names do,
  and an edit that touches one removes it.
- `src/server.rs` publishes each meta as a diagnostic of severity warning,
  such as `Unsolved meta _m_304 : Nat.ℕ, the implicit argument m`.
- After a load or a goal command that leaves metas, the bridge asks
  `Cmd_constraints`. `metas::constraint_text` prints its objects as Agda
  prints them for Emacs, a port of the `Pretty` instance of
  `OutputConstraint`, for every kind in Agda 2.8.0's JSON encoding; a kind it
  doesn't know is shown as Agda's JSON.
- Hover on a meta shows, above the usual hover: the meta as Agda prints it,
  and with implicit arguments when that differs; how to see its context
  (`{m = ?}` for an implicit argument, `?` for a `_`), or, for the type of a
  variable bound without one, to write it; where else it occurs, as or in the
  type of a goal or another meta; and the constraints on it, also with
  implicit arguments when that differs.
- For the versions with implicit arguments, the bridge sends
  `ShowImplicitArgs True`, `Cmd_metas AsIs` and `Cmd_constraints`, and
  `ShowImplicitArgs False`, after a load or goal command that left metas. It
  does so only when Agda's `Status` says implicit arguments are off: a file
  with `--show-implicit` keeps them on, and needs no second version.
- A meta on a variable where it is bound, which Agda highlights as a bound
  name whose definition is itself, is that variable's type; its warning says
  so, as `Unsolved meta _360 : Set, the type of m`.
- An `[UnsolvedConstraints]` error without a location goes on the ranges Agda
  highlights as `unsolvedconstraint`, one diagnostic each.
- `src/protocol.rs` reads `Constraints` answers and the kind of a goal or
  meta; the output file shows a `Constraints` answer as a list, for item 4.
- The output file lists the metas under "Unsolved metas", as Emacs's
  `*All Goals*`, each with its location as a link from the output file,
  relative to its directory, such as
  `[Metas.agda:22.7-12](../Metas.agda:22:7)`. Zed 1.23.2's Markdown preview
  resolves such a link against the Markdown file's directory, normalising
  `..` (`resolve_open_target` in `crates/workspace/src/path_link.rs`), and
  opens the file at that line and column, which it counts in characters, as
  Agda does (`point_from_external_input` in `crates/text/src/text.rs`).
  Metas on a goal are left out, as in the diagnostics. The links point where
  the metas were at the load.
- The code action "Make goal of `_`" puts `{!  !}` in place of a `_`, and
  "Make goal of implicit argument `m`" puts `{m = {!  !}}` after the name of
  a function applied in prefix form; not after an operator applied infix,
  which only the prefix form can give a named argument, nor for the type of
  a binder. The edit is in the code action itself; Agda checks the goal at
  the next save. "Open output file" is offered on every meta.
- The README has a section "Unsolved metas".

---

### 7. Checks and their results

| Check | Result |
| --- | --- |
| Unit tests, after items 3 and 5 | 79 pass: links relative to the output file, also with `..` and percent-encoded characters, in `src/output.rs`; the list of metas with their locations, without those on a goal, in `src/render.rs` |
| `explains_unsolved_metas`, items 3 and 5 | the output file lists seven metas, with the link for `const`, and none on the `with` goal; "Make goal" offered on `const` and a `_`, with their edits, not on `⊕` nor on a binder; after both edits and a save, two more goals and two fewer warnings |
| Mutation checks, items 3 and 5 | metas on goals listed: fails at the count; no links: fails at the link; operators applied infix too: fails at `⊕` |
| Unit tests | 77 pass; 10 are new, in `src/metas.rs`: reading metas and their kinds, names and implicit arguments, following edits, keeping the metas Agda still lists, the innermost meta first, every kind of constraint the port prints, which constraints are on which meta, the hover's text with all its parts, where else a meta occurs, and metas as Agda prints them; `src/protocol.rs` also reads Agda's `Status`, with and without `showImplicitArguments` |
| A new fixture, `tests/fixtures/Metas.agda`, with implicit arguments of a function and an operator, two `_`, a constraint stuck on one, a goal, and a `with` goal | Agda 2.8.0.2 reports the four metas expected, two on the `with` goal, and the `[UnsolvedConstraints]` error |
| `explains_unsolved_metas`, end to end | the four warnings, their ranges and messages; none for the metas on the `with` goal; the error on `refl`; the hovers, with the name's hover below the meta's; metas following an edit, and going when one touches them; a filled `_` gone after a save |
| `explains_unsolved_metas`, second round, with a binder without a type and a `_` of type `zero ≡ zero` added | the binder's two warnings, its type's named `the type of k`; the binder hover; the sort's hover saying it is the type of the other meta; `_≡_ {ℕ} zero zero` with implicit arguments; Agda without implicit arguments afterwards; a file with `--show-implicit` keeping them |
| `goal_types_follow_auto`, changed | `Depends.agda`'s `_B_11` is a warning before auto, its hover says it is the type of goal `?1`, and it is gone after auto solves it |
| Mutation checks, each change undone in turn | no meta diagnostics: `explains_unsolved_metas` fails; no dropping of solved metas: `goal_types_follow_auto` fails; no constraints: fails at "Constraints on it:"; metas not following edits: fails at the hover after the edit; the error not placed: fails, the error at 0:0; metas on goals kept: fails with six warnings instead of four; implicit arguments not switched off again: fails at the type of `refl`; switched off in a file with `--show-implicit`: fails there; no binders: fails at the warnings on `k`; no uses: fails at the binder's hover |
| The full suite, outside the sandbox, on a clean checkout | 79 unit tests and 15 end-to-end tests pass at `d238bb7`, 77 at the second round; the first run, before the second round, found `goal_commands` waiting for six diagnostics and getting eight, which led to decision 3 |
| `Judgement.lagda.typ` from `system-b-proofs`, on a copy | three warnings, `_m_304` on `ℂ`, `_360 : Set, the type of m` on the `m` of `∀{… m …}`, and `_m_388` on `⊢ᵇ`, each with its hover; their types are the same with implicit arguments, and no constraints are on them; the output file lists the three with links, and "Make goal" is offered on `_m_304` (`ℂ`, prefix) and not on `_m_388` (`⊢ᵇ`, an operator applied infix) nor on `_360` |
| Clippy, rustfmt | no warnings; rustfmt only flags `src/highlight.rs`, unformatted before this work |

#### Goals

A goal's hover shows Emacs's `C-c C-,` (and `C-c C-.` when the goal has
text): the goal's type, simplified as Emacs asks; `Have:`, the type of the
goal's text; a cubical boundary; the context, with `(not in scope)` marks;
and the constraints that mention the goal, which Agda sends as text
(`outputForms`). So "Show constraints" is there for goals already, limited to
those that mention the goal. Two things could be added:

- Agda's text for those constraints carries internal detail, as in
  `pred ?0 = zero : ℕ\n  (blocked on _13@14553810981960600810, belongs to
  problem 20)`, where the number after `@` is a module's hash.
- Implicit arguments are hidden: `y ≡ x` is `_≡_ {A} y x` with them, in the
  goal's type and its context. Showing them takes two more Agda commands per
  goal hover, around the one the hover asks.

---

### 8. Sources and how they were used

| Source | What I took from it |
| --- | --- |
| Agda 2.8.0.2 on this machine, through `--interaction-json` | what Agda reports about metas and constraints, on the fixture, on `Depends.agda` before and after auto, after a `with`, and on `Judgement.lagda.typ` |
| [agda/agda](https://github.com/agda/agda) at tag `v2.8.0`: `src/full/Agda/Interaction/JSONTop.hs`, `BasicOps.hs`, `Base.hs`, `InteractionTop.hs`, `src/data/emacs-mode/agda2-mode.el` | the JSON encoding of constraints and blockers, Agda's printer for them, that no command takes a meta, and what Emacs shows |
| [banacorn/agda-mode-vscode](https://github.com/banacorn/agda-mode-vscode) and [willtunnels/agda2-vscode](https://github.com/willtunnels/agda2-vscode) | what the VS Code extensions show for metas and constraints |
| This repository (`src/server.rs`, `render.rs`, `protocol.rs`, `links.rs`, `text.rs`, `tests/lsp.rs`) | where metas fit: the load, edits, goal commands, diagnostics and hover |
