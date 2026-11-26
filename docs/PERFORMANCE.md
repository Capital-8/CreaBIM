# Performance

CreaBIM should feel instant: no frame waits for a drawing, the app uses no CPU when left alone, and the download stays
small. This page records what was measured, what changed, and what is still to do (Brief 02, Addendum A6).

All figures are from the owner's Windows machine (16 cores) and the starter house: 640 elements, about 57,000
triangles.

## Where it stands

| What | Before A6 (main `0e5ed46`) | Now (`a6-performance`) | Target |
| --- | --- | --- | --- |
| CPU with the app left alone | 50–57 % of one core | 0.0–0.3 % (0.3–0.5 % with the WebView2 processes) | under 1 % |
| `creabim-app.exe` | 38.5 MB | 27.9 MB | under 20 MB |
| Window on screen (warm start) | 190–360 ms | 190 ms | under 300 ms |
| Selecting something with 3D open | about 80 ms (scene rebuilt, edges found again) | a scene copy (3 ms), recolouring and the upload | under 16 ms |
| A drag with sections open | every frame rebuilt every section (300–450 ms a frame) | sections and 3D build on workers; the frame never waits | 16 ms a frame |
| Section A-A | 224 ms | 73 ms | 30 ms |
| South / East Elevation | 333 / 455 ms | 89 / 113 ms | 30 ms |
| 3D Street view (a 3D drawing) | about 1,100 ms | 165 ms | 100 ms |
| A plan | 15 ms | 10–16 ms | 15 ms |

## What changed

1. **Idle means idle.** The web panes wake the app when they send a message; the 30 ms repaint poll is gone on Windows
   and macOS (Linux keeps it, as GTK only runs while frames pump it).
2. **Release profile:** fat LTO, one codegen unit, stripped, abort on panic. `cargo build --profile fast` gives
   near-release speed with a quick link for day-to-day testing.
3. **WebView data** lives in the user's local app data, not beside the program (an installed copy can't write
   there).
4. **One shared 3D scene per model revision** (`creabim_geom::shared_scene`). The 3D view, every section and
   elevation, and the tools use it instead of each building their own. Model revisions are unique across the
   process, so a cache can key on the revision alone.
5. **The 3D view's edges are cached by mesh content.** A selection change or a one-element edit doesn't find every
   element's edges again (32 ms → 1.4 ms).
6. **Drawings build off the UI thread** (`crates/creabim-app/src/builds.rs`). A view whose drawing takes 20 ms or more
   is rebuilt on a worker when the model changes. Its area keeps the last finished drawing until the new one arrives.
   The 3D view does the same with the scene. Plans are still built on the spot.
7. **Hidden lines on every core.** The depth buffer is built in bands of rows, one per core, each row only over the
   triangle's span. The edges are sampled per element in parallel. A test checks the banded buffer against a
   whole-box scan cell for cell.

8. **A move of many elements redoes less.** Measured moving 180 of the starter house's elements with G: a frame in
   which the model changed took 51 ms and now takes 29 ms; the slowest tenth went from 72 ms to 51 ms. What a move
   held still costs went from 50 ms a frame to 10 ms. Five causes, none of them in what is drawn:
   - the elements attached to the moving ones were searched for once per moving element, every frame;
   - the browser tree was built again every frame (it already held still for grip drags);
   - a move held still committed the same change again every frame, so every drawing and the scene were rebuilt;
   - the snap search checked every snap point against a list of the moving elements (12 ms a frame for 200);
   - the 3D view found the moved elements' edges again in the frame that showed a new scene (about 25 ms); the worker
     that builds the scene finds them now.

9. **A view's graphic style redraws that view alone.** Every edit used to count as a change to the whole model, so
   choosing a drawing style (Technical, Poché, the washes…), Shaded or Hidden line, hidden things dashed or in
   colour, or room colours rebuilt every open drawing and the 3D scene: 40 to 160 ms each on the starter house,
   though only one view's look had changed. The model now keeps, for each view, the revision of the last change to
   its own look (`Model::drawing_revision_of`); such a change leaves `drawing_revision` alone, and the shared 3D
   scene keys on `drawing_revision`. No other view reads these settings of a view (`tests/view_look.rs` holds
   that line). No style is slower to draw than another: all seven are within 5 % of each other in every view.
   The 3D view's buffers and its snaps key on the same revision (they were rebuilt after a style click: 18 ms and
   78 ms), except while the drawing is shown in 3D or a view is traced there as a reference. Measured in the app:
   a click on a drawing style now costs the plan's own redraw, 18 ms, and nothing else.

10. **A large mass is welded once.** Turning a mass with panels or louvres into triangles (thousands of faces)
    cost about 5 ms in every scene build, whether it had changed or not; fifty such towers, a quarter of a second.
    The welding is now kept by the mesh's content and only the corners are placed again
    (`scene::edit_mesh_to_tri`). The 3D scene with 48 panelled towers: 300 ms → 75 ms; with 96: 580 ms → 117 ms.

11. **A snap looks only near the pointer.** Every snap went through every point and edge of the model, on every
    frame of a move or a hover. Once a view holds still for a second snap, its points and edges are sorted into
    32-pixel cells on the screen and a snap looks only at the cells round the pointer (`Snapper::near_cursor`); a
    view that moves is looked through whole as before. The result is the same snap: a test compares the two on the
    starter house from two views. Measured in the app, the move's own step: 6.5 ms to under 3 ms on the starter
    house; about 20 ms to 9 ms with 48 panelled towers.
12. **A moved mass keeps its feature edges.** The edge cache keys on a mesh's positions, so a mass that only moved
    was new to it and its edges were found again for each scene. A large mass's mesh now carries which shape it is
    and where it stands (`scene::placed_shape`), and the edges of a shape are placed, not found again.
13. **Join cuts are shared between threads.** The cuts where walls meet slabs were kept in a cache of the thread
    that made them; each scene is built on a new thread, so every scene made every cut of the building again
    (100 ms and more for the starter house), whatever had changed. One cache for all threads; the cuts themselves
    are the same.
14. **A building that moves takes its cuts along.** While a move is live, each wall or slab piece with all its
    cuts is kept by how the shapes stand to each other, and set down where the building now stands instead of
    being cut again. Only a finished piece is carried, never a cut made on a carried shape (that was what split
    without end: a carried face and a fresh one a hair apart). When the move ends the model is drawn again with
    nothing carried, so what stays is cut where it stands, exactly as before this change: no golden drawing
    changes. Starter house, each step of moving all of it (`tests/perf_move.rs`): 120 ms to 1.1 s before, 35 ms
    now. A move that changes how walls and slabs stand to each other (one wall alone, a rotation) cuts as before.

## Masses and modifiers, measured

`cargo test --release -p creabim-geom --test perf_mass -- --ignored --nocapture` adds towers (20 × 15 × 45 m) to
the starter house and changes a modifier's setting each round. Working out a modified shape is never the cost
(under 5 ms for any modifier, up to 35,000 faces); what the modifier produces is:

| One tower with | Faces | 3D scene | Section A-A |
| --- | --- | --- | --- |
| nothing (the starter house alone) | 0 | 37 ms | 58 ms |
| Slice, Bevel, Shell, Taper, Twist, Smooth (4 levels), arrays | under 1,600 | 34–36 ms | 90–105 ms |
| Panelize 1.2 × 3.0 m | 4,866 | 40 ms | 119 ms |
| Panelize 0.6 × 1.5 m | 18,006 | 58 ms | 150 ms |
| Slice + Panelize + Louvres | 35,490 | 87 ms | 227 ms |

A plan stays at 16 ms throughout. With many towers (Slice + Panelize each), after change 10:

| Towers | Triangles | 3D scene | Section A-A |
| --- | --- | --- | --- |
| 12 | 200,000 | 50 ms | 240 ms |
| 48 | 600,000 | 75 ms | 500 ms |
| 96 | 1,130,000 | 117 ms | 740 ms |

In the app, moving the house and 48 towers together: the plan follows at 20–25 frames a second, the 3D view
catches up about once a second. What holds it there is listed under "Still to do" (density).

15. **A shape that is pulled or moved is built again alone.** A pull changed one shape, and the 3D view waited for
    a worker to build the whole scene again: about 50 ms a step on the starter house, so the shape trailed the
    pointer. The scene as it was before the edit is kept while a pointer edit runs; a shape or a drawing with
    nothing cut by it or cutting it is built again by itself into that scene on the spot
    (`creabim_geom::shared_scene_patched`, 2.7 ms a step). A test holds the patched scene to the one built whole.
    The plan beside it is redrawn on a worker during a pull or a move in the 3D view, as it already was for a grip
    drag there (it was 20 ms of every step, on the spot). Walls, slabs and whatever is joined or cut still take
    the whole build.
16. **The log writes while something follows the pointer, not while a tool waits.** Pull stays in hand, and a line
    was written every frame for as long as it was. With the Log view open, redrawing those lines took the whole app
    to about four frames a second. They are written only while a face, a grip or a selection follows the pointer,
    four times a second at most.
17. **What a pull in the 3D view still redid each step.** A step of a pull took 50 to 110 ms in use. Building the
    one shape again took 25 ms, because how the whole building's walls meet was worked out first: it is not, for
    shapes and drawings. The quick path of change 15 was dropped each time the pull passed back through where it
    began, and every step after that built the whole scene: it is kept for the whole pull. A plan that costs about
    20 ms fell on either side of the limit for building on the spot, so it was built there every other step: during
    a pull or a move in 3D only a drawing under 4 ms is. The plan's and the section's snaps were made again each
    step (10 ms) though the pointer was in 3D: they are left until the edit ends. The snap was looked up twice a
    step. The log splits a pull's step into "pull: snap" and "pull: change". Not measured in use yet.

## Still to do

- **A6-1.1 the live overlay:** during a drag, only the dragged elements redrawn in every area at 60 fps. Today the
  other areas update as fast as their background builds (about 10 a second for a section).
  What a changed frame of a 180-element move still spends: the plan drawn whole on the spot (17 ms), the 3D view's
  buffers built whole when a scene arrives (16 ms) and the selection's highlight (12 ms).
- **Cutting in local coordinates** (each pair cut beside its own first corner, rounded to a hundredth of a
  millimetre) was tried for the moving building and dropped: it changed 12 of the 52 golden drawings, and in one
  section a wall face came out leaning. Change 14 replaced it.
- **Not timed in the app yet:** change 14's figures are from the scene alone, without the app around it.
- **Density (many heavy elements):** everything below is linear in the model's triangles and is redone whole.
  - The 3D view's buffers and the selection's highlight are built whole on the main thread when a scene arrives
    (60 ms and 50 ms with 600,000 triangles). Build them per element, or off the main thread.
  - Identical shapes are stored and drawn once each; they could be instances of one.
  - A section draws every triangle (500 ms with 48 towers). One tall mass adds 35 ms to every section of the
    starter house, modifiers or not: not looked into yet.
- **A6-2.2 depth buffer size from the view:** a fixed 2,048 cells across, whatever the view's size on screen.
- **A6-2.3 fewer edges:** 48,000 feature edges for a small house, most of them seams on flat faces; merge them
  before sampling.
- **A6-2.4 per-element caching:** rebuild only the elements a change touches.
- **A6-5 cached tessellation** of heavy drawings while the camera doesn't move.
- **A6-6.2 graphics back ends:** only what each platform needs. Also compress the help pictures.
- **Memory:** about 410 MB for the app with the starter house. Not traced yet.
- **Start-up:** the first launch builds every open view's drawing on the spot (its cost isn't known yet), so the app
  is busy for a few seconds after the window appears. Build the first drawings on workers too, with a placeholder.
- **The rest of a view's settings still count as a model change:** its scale, what it hides, its range, its
  overrides. Each rebuilds every open drawing, since another view may show it (a plan's range in a section, a
  section's line in a plan). Only the look listed under "What changed" is told apart so far.
- **Run-to-run differences:** some hidden-line drawings differ slightly between runs. Some data is kept in hash maps
  keyed by element IDs, so the scene's triangle order can change between launches, and equal-depth ties then resolve
  differently. Sort those maps, or break ties by element order, to make drawings repeatable.

## Measuring

```bash
# drawing and scene budgets (fails when a drawing is clearly slower than recorded)
cargo test --release -p creabim-geom --test perf -- --ignored --nocapture

# the background builds never make a frame wait
cargo test -p creabim-app builds::
```

**Where a slow frame's time went.** Every frame over 50 ms writes one line to the activity log (*Help › Open the
activity log*) with each step's own time, the longest first:

```text
slow: frame of 161 ms: snaps_3d 80 + rest of show_3d 41 + show_plan ×2 20 + rest of frame 19
```

Steps nest: "rest of show_3d" is the 3D area's time without the steps timed inside it. A wait of over 50 ms
between two frames while a mouse button is held is logged as `between frames`: time lost outside the frame
(painting, the graphics card). Start the app with `CREABIM_PERF=1` to print every step over 8 ms to stderr as well
(`CREABIM_PERF_MS=4` lowers that). To time a new step, put `let _p = Perf::new("name");` at its start.

To check idle CPU, start the release build, leave it alone for 10 seconds, and read Task Manager's CPU column for
`creabim-app.exe` (or use `top` on Linux and macOS).
