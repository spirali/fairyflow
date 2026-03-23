---
icon: lucide/download
---

# Exports

FairyFlow can export your sequences in three formats. All export options are available in the sequence editor.

| Format | Use case |
|---|---|
| **Player package** (`.ffpkg`) | Interactive playback with cue-based navigation |
| **Video** (`.mp4`) | Sharing or embedding; requires ffmpeg |
| **PDF** | Slide handouts; each selected frame becomes one page |

---

## Player package

A `.ffpkg` is FairyFlow's native self-contained format. It bundles the animation data so it can be played without a running server.

Play a package with:

```
fairyflow play <mypackage.ffpkg>
```

**Keyboard shortcuts in the player:**

| Key | Action |
|---|---|
| ++right++ | Continue to next cue / advance animation |
| ++left++ | Step back |
| ++f5++ | Toggle fullscreen |

The player automatically pauses at every [cue](cues.md).