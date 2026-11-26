# Masses, modifiers and booleans

A **mass** is a free 3D shape you can model like in Blender: for early design studies, or anything that isn't a wall, slab or roof.

## Making a mass

- Pick the **Mass** tool and click to place a box.
- Or extrude a closed [sketch](sketches) with `E`.

## Editing its shape

- Click a face in the 3D view to select it, then press `E` to extrude it, or move it with `G`.
- Use [Pull](move#pull-p) (`P`) to take any face out or in with the pointer or a typed distance.
- Rotate, tilt and scale it with `R` and `S` (see [Move, rotate and scale](move)).

### Cutting a face with a line

Draw a **Line** on a face of a mass in the 3D view, from one of its edges to another (through corners in between if you like), and finish it with `Enter`. The line cuts the face in two and is not kept: each part is now a face of its own, and [Pull](move#pull-p) takes either out or in. That is how a step, a recess or a raised part is made. One `Ctrl+Z` puts the face back.

A line that stops short of an edge, or runs off the face, is left as a drawn line and cuts nothing.

### An outline inside a face

Draw a closed outline (a **Rectangle**, a **Circle**, or lines back to the first corner) inside a face of a mass, clear of its edges. It becomes a face of the mass, and is picked: [Pull](move#pull-p) sinks it for a recess or a niche, or raises it. The face round it is still one face; pulled, it rises round the outline. One `Ctrl+Z` takes the outline away.

An outline that touches or crosses the face's edge is left as a drawing of its own, which Pull makes into a separate solid.

Selected, a mass shows only its main grips: its base point (moves it), its turn grip and the clicked face's push and pull arrow. Every face's arrow, its corners, its edges and the lift arrow come in [Edit mode](edit-mode) (`Tab`).

*Properties* shows its **Location**, **Rotation (XYZ Euler)** and **Scale**, as Blender's Transform panel: a mass turns about X, Y and Z, so a shape drawn on a wall turns about the wall's axes.

Its **Role** in *Properties* says what it is for: **Solid** is part of the building; **Void** is something to cut with; **Guide** is something to draw by. A void or a guide shows in 3D and is left out of plans, sections, elevations and exports.

A mass can be several pieces. In [Edit mode](edit-mode), double-click a point, an edge or a face to pick the whole piece it belongs to, or press `L` to grow what is picked to its pieces; then move, turn or scale it. **Separate** makes each piece a mass of its own; **Join**, with two or more masses selected, makes them one.

Give a mass an IFC class in *Properties* (such as `IfcRoof`) and it is exported as that.

## Mesh edit

The *Mesh edit* section of *Properties* works on the face you clicked: **Extrude** pushes it out with new sides (`E`), **Move** slides it, **Taper** shrinks it, and **Inset** draws a smaller face inside its edges, by the distance in *Inset by*. Inset makes a reveal, a recessed panel or a parapet: inset the roof face, then push it down. These change the mesh under its modifiers.

## Modifiers

Modifiers change a mass's shape without touching it, so you can change or remove them later. Select a mass and open the **Modifiers** tab of *Properties* (the wrench), then **Add modifier**. The menu groups them under *Repeat*, *Facade*, *Shape* and *Deform*, each with its icon; hover one for what it is for:

| Modifier | What it does |
| --- | --- |
| **Array** | Copies in a row (a pergola's rafters): a count, or to fit a length; each copy can step by its own size, turn and grow |
| **Radial array** | Copies round a centre, picked in a plan or set to its middle: seats round a table, columns round a rotunda |
| **Path array** | Copies along a sketch or a guide: balusters, posts along a curve |
| **Mirror** | A mirrored copy, its plane at the middle or at an edge |
| **Bevel** | Rounds or chamfers the upright corners: all of them, the outside or the inside ones |
| **Shell** | Hollows it to a thickness, open at the top, the bottom or both |
| **Solidify** | Gives a surface a thickness, growing in or out |
| **Slice by levels** | Floors cut out of the mass, with a gap between: a quick tower study |
| **Panelize** | Facade panels with joints on its upright, rectangular faces |
| **Louvres / fins** | Fins on the sides facing north, east, south or west, at an angle |
| **Taper** and **Twist** | Narrows it towards the top, or turns it as it rises |
| **Smooth** | Smooths it; its *Split* kind keeps the shape and only adds faces |

![Slice by levels on a mass: the floors and the gaps between them, with its card in the Modifiers tab](img/modifiers.jpg)

Each modifier is a card, as in Blender. Its icon switches it on and off, and folded it shows a one-line summary. Rename it, and use its `…` menu for **Apply** (make it part of the shape), **Duplicate**, **Move up** and **Move down**; the cross removes it. Open, its settings are rows, and the `?` on the card explains it. The card you are working in shows its guides in every view: the radial centre and circle, the mirror plane, the slice floors as see-through planes. The *Element* tab says how many modifiers the mass has.

## Booleans: cutting one shape from another

Draw a shape (a mass), then select it together with the walls, slabs, roofs, columns, beams or other elements it should work on. The selection strip offers **Add** (join it on), **Subtract** (cut it out) and **Intersect** (keep only what both hold), and **Reach**: *As drawn*, or running on *Up*, *Down* or *Both* without end, so one rectangle cuts every floor. The shape stays as a see-through ghost: move or reshape it to change the result.

The *Junctions and booleans* tab of an element lists what adds to or cuts it under **Booleans**, one folded box each: open it to change its *Kind* or *Reach*, click the shape's chip to select it, or × to remove it. A selected shape lists what it **Works on** the same way. If a shape is selected with something it can't work on (a door, a dimension), the status bar says which one to leave out.
