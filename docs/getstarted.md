---
icon: lucide/rocket
---

# Get started

## Installation

## Create project

## First code & render


```ffpy frame="0"
with scene():
   rect().size(30, 20).color("green")

```


```ffpy video="mp4"
with scene():
   r = rect().size(20, 20).color("green")
   jump_time(1)
   r.color("orange")
```