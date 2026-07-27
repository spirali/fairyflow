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
code(source)                  # picks up "python"/"base16-ocean.dark" from above
code(source, "rust")          # a per-call language still overrides it
```

`family` defaults to `"monospace"` — pair it with `[font-aliases]` in
`fairyflow.toml` (see [Font aliases](text.md#font-aliases)) to point `"monospace"`
at a specific installed font.

For an inline code literal inside a sentence, `.code(language=None, *, theme=None)`
on a `Text`/`TextGroup`/`TextSpan` applies the configured style directly:

```python
t = Text("Call ")
t.span("map()").code()               # mono, code size — inline literal
t.span(" over the list.")
```

On a run, `.code()` is style-only: `sh_language` is a block-level flag, so passing
an explicit `language=` to a run's `.code()` raises `TypeError` — call it with no
arguments to just pick up the configured family/size.

For lower-level control — styling a `Text` block you didn't build with `code()` —
`.sh(language, theme=None)` enables highlighting directly; `code()`/`.code()` use it
internally.
