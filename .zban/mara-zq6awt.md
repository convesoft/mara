---
title: pulldown-cmark ends a script, pre, style or textarea HTML block only at a lowercase end tag
status: todo
type: bug
tags:
  - maintainer
discovered_from: mara-t6xn9z
created_at: 2026-11-14T15:26:41Z
---

Goal: once `pulldown-cmark` ends a type-1 HTML block as CommonMark does, Mara reads such a block as GitHub renders it and the `html-end-tag-case` error is gone.

Context: CommonMark 0.31.2 ends an HTML block that starts with `<script`, `<pre`, `<style` or `<textarea` at a line containing any of `</script>`, `</pre>`, `</style>`, `</textarea>`, in any case. `pulldown-cmark` 0.13.6 (`firstpass.rs`, `get_html_end_tag` and the `contains` check that ends the block) ends it only at its own end tag in lowercase, so after `</SCRIPT>` the rest of the container is read as raw HTML: code spans, links, separators and items disappear. GitHub renders such a file correctly.

Decided by the maintainer (mara-t6xn9z): "B: dedicated error now — The parser detects an HTML block of those four kinds whose end tag is not lowercase and reports a specific error telling the user to lowercase it. Small and honest, but it makes a document GitHub renders correctly invalid in Mara until the dependency is fixed." Done in mara-t6xn9z: the error `html-end-tag-case` at that end tag, rule and reason in DES-REGIONS (docs/format.mara.md), no anchor line reported in the rest of the block; tests in `reports_an_end_tag_that_the_parser_misses` (`crates/mara-engine/src/document/tests.rs`).

The other divergence, a block of one kind ended by another kind's end tag (`<pre>` … `</script>`), is not reported: the maintainer's decision covers a block's own end tag only.

```markdown
<script>
let a = 1;
</SCRIPT>

`code`
```

- [ ] Report both divergences upstream to `pulldown-cmark`, with the input above
- [ ] Once a fixed release exists: pin it, remove `html-end-tag-case` and its sentence in DES-REGIONS, and turn its test into one that the input above yields a code span
