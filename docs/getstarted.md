---
icon: lucide/rocket
---

# Get started

## Installation

```bash
$ pip install fairyflow
```

## Create project


```bash
$ fairyflow init <project_name>
```

It will create an initial project layout, see TODO REF TO STRUCTURE.MD for more info.

## Start development environment

```bash
$ fairyflow open <project_name>
```

It starts a local web server, click on printed URL to open interactive environemnt:

<img src="screenshot_after_init.png"/>

## First code & render


```ffpy frame="0"
with scene():
   rect().size(30, 20).color("green")

```

Press ++ctrl+s++ to evaluate scene. You will get the folowing output:

<img src="screenshot_after_eval.png"/>


## You first video


```ffpy video="mp4"
with scene():
   r = rect().size(20, 20).color("green")
   linear()
   jump_time(1)
   r.color("blue")
```

Press again ++ctrl+s++ to evalute the scene, you will get the following result:

<img src="screenshot_after_eval2.png"/>

and resulting video:

TODO
