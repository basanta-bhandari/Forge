# ForgeTUI

ForgeTUI is a keyboard-first 3D modeller with a real-pixel terminal viewport. It uses Kitty graphics or Sixel when available and keeps ASCII as a portable fallback, while the surrounding panels remain a fast native TUI.

![Status: prototype](https://img.shields.io/badge/status-functional_prototype-37b7b5)

## Build and run

Requires Rust 1.88 or newer.

```bash
cargo run --release -- --demo
```

Graphics are auto-detected. Press `v` to cycle modes, use `:graphics kitty`, `:graphics sixel`, or `:graphics ascii`, or set `FORGETUI_GRAPHICS=kitty` before launching.

Other useful invocations:

```bash
cargo run -- --help
cargo run -- --render model.forge
cargo run -- model.obj
cargo test
```

Inside the app, press `?` for the complete key and command reference. The fastest workflow is `a` to add a primitive and `:` to enter precise commands such as:

```text
move x 2.5
rotate y 45
scale z 0.5
rename Bracket
save bracket.forge
export bracket.stl
```

## File formats

| Ecosystem | Format in ForgeTUI | Status |
|---|---|---|
| ForgeTUI | `.forge` | Native editable JSON; always available |
| Fusion 360 | `.f3d` / STEP | `.f3d` is proprietary; exchange through `.step` |
| Onshape | STEP | Onshape is cloud-native and has no local native document file; `.step` import/export uses `FreeCADCmd` |
| Blender | `.blend` | Import/export uses the `blender` command-line application |
| General mesh | `.obj`, `.stl` | Built in; no external applications required |

ForgeTUI refuses to create fake `.f3d` or `.blend` files. When the official/native format writer is unavailable it gives an actionable conversion message. This keeps exported work trustworthy.

## Current scope

- Anti-aliased RGB software renderer with lighting and solid, wireframe, and X-ray modes
- Kitty and Sixel terminal image protocols with automatic detection and ASCII fallback
- Perspective and orthographic cameras, orbit, zoom, and frame-selection
- Cube, sphere, cylinder, cone, torus, wedge, and a larger 10×10 plane
- Mouse selection, left-drag orbit, right/middle-drag pan, and wheel zoom
- Five themes: Midnight, Amber, Matrix, Light, and Mono (`t` cycles them)
- Grid snapping, keyboard nudging, duplicate, mirror, reset, dimensions, and standard CAD views
- Event-driven redraws for near-zero idle CPU use
- Exact move, rotate, scale, visibility, rename, delete, selection, undo, and redo
- Native project round trips and built-in OBJ/STL interchange
- Optional Blender and FreeCAD conversion adapters
- Headless `--render` mode for scripts and snapshots

This is a mesh modeller rather than a production parametric/constraint CAD kernel. Sketch constraints, booleans, B-Rep/NURBS editing, materials, animation, and GPU rendering are logical next stages.

## Controls

| Key | Action |
|---|---|
| `h/l`, arrows | Orbit camera |
| `+/-` | Zoom |
| `j/k` | Select next/previous object |
| `a` | Add menu |
| `g`, `r`, `s` | Guided move, rotate, scale commands |
| `d` | Duplicate selection |
| `1`, `3`, `7`, `0` | Front, side, top, isometric views |
| `5` | Toggle perspective |
| `t` | Cycle themes |
| `v` | Cycle Auto/Kitty/Sixel/ASCII graphics |
| `Delete` | Remove selection |
| `Space` | Cycle shading mode |
| `p` | Toggle perspective |
| `f` | Frame selection |
| `u`, `Ctrl-r` | Undo, redo |
| `Ctrl-s` | Save native project |
| `:` | Command prompt |
| `?` | Help |
| `q` | Quit |

Mouse controls are intentionally conventional: click to select, left-drag the viewport to orbit, right/middle-drag to pan, and use the wheel to zoom. Try `theme amber`, `snap .25`, `mirror x`, or `view top` in the command prompt.
