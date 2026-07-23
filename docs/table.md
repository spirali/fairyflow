---
icon: lucide/table
---

# Table

`Table` builds tabular data as a `Group().grid(...)` of cells, so everything
still animates with the normal vocabulary — it is not a full table renderer.

```ffpy frame="0"
with Scene(width=260, height=120):
    Table([
        ["Name", "Age", "City"],
        ["Alice", "32", "Oslo"],
        ["Bob", "28", "Brno"],
    ], header=True)
```

`Table(rows, *, header=False, widths="auto", row_height="auto", padding=(12, 6),
stroke="#888", fill=None, align="left")`:

- `rows` — a list of lists. A cell value is a `str` (becomes a `Text`) or any
  other node (e.g. an `Image`) — the node is moved under the cell it's placed
  in, wherever it was originally constructed.
- `widths` / `row_height` — `"auto"` (default) sizes each column to its widest
  cell and each row to its tallest; a single number fixes every column/row to
  the same size; a list gives an explicit size per column/row.
- `header=True` — row 0 is rendered bold with a distinct background.
- `padding` — `(x, y)` inner spacing for every cell (see [Padding](layout.md#padding)).
- `stroke` / `fill` — outline and background color applied to every cell;
  pass `fill=None` (default) to leave cells unfilled.
- `align` — `"left"`, `"center"`, or `"right"`; a single value applies to
  every column, or pass a list for one alignment per column.

## Addressing cells

Cells are addressed 0-based, **including** the header row:

```ffpy video="mp4"
with Scene(width=260, height=120):
    t = Table([
        ["Name", "Age", "City"],
        ["Alice", "32", "Oslo"],
        ["Bob", "28", "Brno"],
    ], header=True)
    wait(0.5)
    t.cell(2, 1).fill("gold", dur=0.3)     # highlight one cell
```

- `t.cell(r, c)` — the cell at row `r`, column `c`.
- `t.row(r)` — all cells in row `r`, left to right.
- `t.col(c)` — all cells in column `c`, top to bottom.

`t.cell(r, c).fill(color)` sets that cell's background directly (it forwards
to the cell's own background rect); a `Table` is a node like any other, so
`.align()`, `.xy()`, `.rotate()`, etc. all work on it as a whole:

```python
for c in t.row(0):
    c.fill("#333")       # style the header row
t.align(0.5, 0.3)
```

## Per-column alignment and an image cell

```ffpy frame="0"
with Scene(width=200, height=110):
    icon = Rect().size(16, 16).fill("steelblue")
    Table([
        ["Name", "Icon"],
        ["Alice", icon],
    ], header=True, align=["left", "center"])
```

## Padding

`padding=(x, y)` sets the inner spacing between every cell's border and its
content, table-wide. With the default `(12, 6)`, left-aligned text sits with
a clear gap from the cell's left border line, instead of touching it:

```ffpy frame="0"
with Scene(width=260, height=120):
    Table([
        ["Name", "Age", "City"],
        ["Alice", "32", "Oslo"],
        ["Bob", "28", "Brno"],
    ], header=True)
```

A tighter table-wide padding shrinks that gap — `(2, 2)` leaves text sitting
right up against each cell's edge:

```ffpy frame="0"
with Scene(width=260, height=120):
    Table([
        ["Name", "Age", "City"],
        ["Alice", "32", "Oslo"],
        ["Bob", "28", "Brno"],
    ], header=True, padding=(2, 2))
```

Since a cell is just a `Group` (`t.cell(r, c)`), `.padding()` also works on
an individual cell directly. Note that with `widths="auto"` (the default)
every column's width is already fixed at construction time to fit its
widest cell *plus* the table-wide padding — so a per-cell override only has
visible room to work with under an explicit `widths=`:

```ffpy frame="0"
with Scene(width=260, height=120):
    t = Table([
        ["Name", "Age", "City"],
        ["Alice", "32", "Oslo"],
        ["Bob", "28", "Brno"],
    ], header=True, widths=70, padding=(2, 2))
    t.cell(1, 0).padding(x=20)   # "Alice" gets extra left breathing room
```

## Non-goals

Cell spanning, per-cell border control (`stroke` applies uniformly to every
cell), and scrolling are deliberately out of scope — build the grid manually
with [`Group().grid(...)`](layout.md#grid-layout) if you need any of those.
