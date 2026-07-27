---
icon: lucide/code
---

# Code

`code(source, language=None, *, theme=None, dedent=True)` builds a `Text` block for
a source-code snippet. Unlike `stext()`, it never runs the markup parser — `<`, `>`
and `&` in the source stay literal, so a snippet containing a generic (`Vec<T>`) or
a comparison (`a < b`) is never misread as a tag. `dedent=True` (the default) strips
the common leading whitespace and any leading/trailing blank lines, so the snippet
can be indented to match the surrounding scene code.

```ffpy frame="0"
with Scene():
    code(
        """
        x = "world"
        print(f"Hello {x}!")
        """,
        "python",
    )
```

An optional `theme=` argument selects the color theme. Preinstalled themes:

* "InspiredGitHub" (default)
* "base16-ocean.dark"
* "base16-eighties.dark"
* "base16-mocha.dark"
* "base16-ocean.light"
* "Solarized (dark)"
* "Solarized (light)"

```ffpy frame="0"
with Scene(background="#2b303b"):
    code(
        """
        x = "world"
        print(f"Hello {x}!")
        """,
        "python",
        theme="base16-ocean.dark",
    )
```

## Project-wide defaults

Call `set_default_code(language=None, *, theme=None, family=None, size=None)` once
in `prologue.py` to set the language/theme/font for every `code()` block in the
project:

```python
# prologue.py
from fairyflow import *

set_default_code("python", theme="base16-ocean.dark")
```

```python
code(source)  # picks up "python"/"base16-ocean.dark" from above
code(source, "rust")  # a per-call language still overrides it
```

`family` defaults to `"monospace"` — pair it with `[font-aliases]` in
`fairyflow.toml` (see [Font aliases](text.md#font-aliases)) to point `"monospace"`
at a specific installed font.

For an inline code literal inside a sentence, `.code(language=None, *, theme=None)`
on a `Text`/`TextGroup`/`TextSpan` applies the configured style directly:

```python
t = Text("Call ")
t.span("map()").code()  # mono, code size — inline literal
t.span(" over the list.")
```

On a run, `.code()` is style-only: `sh_language` is a block-level flag, so passing
an explicit `language=` to a run's `.code()` raises `TypeError` — call it with no
arguments to just pick up the configured family/size.

For lower-level control — styling a `Text` block you didn't build with `code()` —
`.sh(language, theme=None)` enables highlighting directly; `code()`/`.code()` use it
internally.

## Manually highlighting part of a snippet

`.sh()` only colors runs whose fill is still the default — a run with its own
explicit `.fill()` keeps that color instead. That makes it possible to combine
automatic syntax highlighting with a manual highlight on just one identifier:
build the block with `stext()` (whose tags apply `.fill()` immediately) and
chain `.sh()` onto the result, so the tagged word keeps its manual color while
every other run still gets normal per-token syntax colors:

```ffpy frame="0"
with Scene(background="#2b303b"):
    stext(
        """
def total(items):
    return sum(<s bold color="yellow">items</s>)
        """
    ).sh("python", theme="base16-ocean.dark").font("monospace", 20)
```

`stext()` runs its markup parser over the whole string, which misreads bare
`<`, `>` or `&` as tags (see [stext](text.md#stext)); so reconfigure delimiters or build the block with `.line()`/`.span()` instead (plain
strings, no parsing) and call `.fill()` on the run you want to stand out.
