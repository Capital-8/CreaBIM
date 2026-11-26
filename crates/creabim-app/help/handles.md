# Handles and grips

A selected element shows small handles you can drag to change it directly, in 3D, plans and sections.

- **Square grips** move a point: a wall's ends, a slab's or roof's corners and edge midpoints, a beam's ends, a grid line's ends, a column's base, a stair's start, a railing's points.
- **Round handles** change a size: a wall's height and faces, a door's or window's jambs (width), head and sill, a column's top.

Grips snap to what they land on, and the readout shows the new value. Every drag is one undo step.

Drags are free: the element goes exactly where the pointer takes it, and snaps catch it only when the pointer comes near a corner, an edge, a guide or a grid line. Hold `Alt` to drag with no snapping at all. Two aids are there when you want round numbers, both off until you turn them on in the status bar's **Grid snap** menu:

- **Grid snap** (`Alt+S`): points and drags land on a grid of the spacing you pick (10 mm, 50 mm, 100 mm or your own), measured from the project's origin.
- **Distance marks**: while you drag, faint marks every spacing from where you grabbed; near one, the drag holds on it.

For an exact value, type it during the drag (`Tab`), or click a temporary dimension.

## Your grip style

*CreaBIM › Settings… › Viewport › Grips* changes how grips look, for you only: *Quiet* (squares at corners, bars on edges), *Edges are the handles* (almost nothing at rest; an edge shows its bar under the pointer) or *One grip shape* (the same small circle everywhere). Rest on one to see it on the selection. It never changes anyone's drawings, and it goes with you from project to project.

## Turning, sliding and sizing

- A selected **object** (furniture or a fixture) has a round **turn dot** in front of it: drag it round to turn the object about its middle. A protractor shows the turn, which goes in Rotate's step (whole degrees when that is *Free*) and locks at 0°, 90° and 180° as it passes them; hold `Alt` to turn it freely. The arrow beside it turns it round to face the other way.
- A **column** has a turning grip beside it, and round grips on its sides for its width and depth (a round column's diameter); it stays on its point.
- A **stair** has a turning grip behind its first riser.
- A **door or window** has a round grip in its middle to slide it along its wall at the same width; a **beam** has a round grip on its side for its width.
- A **wall** has a round grip off the face opposite its thickness grip: drag it to slide the whole wall square to itself, and the walls joined to it stretch to stay joined. Right-click a wall and pick *Split wall here* to make two walls meeting at that point; doors and windows stay where they are.

## Walls stay joined

Drag a wall's end and the walls joined to it follow, so the corner stays closed. To drag an end on its own, hold `Ctrl` or `Alt` while dragging, or disallow the join at that end (see [Walls](walls#joins)).

## Outline corners

Slabs, roofs, shafts, sketches and railing paths all show the same grips on their outline: a square on each corner to move it and, on a closed outline, a round grip on each edge to push it. To add or remove corners, or reshape holes, open the element in [Edit mode](edit-mode) (`Enter` or a double-click): pick corners and press `Delete`, drag a ◇ to add one, or right-click for more.

Rest the pointer on any grip for a tip saying what it does. During a drag, press `Tab` to type a length.

## Edges in Edit mode

In [Edit mode](edit-mode), an edge of the outline is taken anywhere along it. It thickens and shows its length under the pointer.

- **Drag** it and it moves as it is; the edges beside it stretch.
- **Click** it and it is picked: it turns orange, with its two corners filled. `Shift`+click picks more, and a box picks the corners inside it, in the 3D view too.
- `M` moves what is picked by a base point, and leaves the rest of the outline where it is.
- A click on nothing lets go of what is picked. `Esc` or `Tab` leaves Edit mode.

With an edge picked, the strip at the right of the view, and a right-click on the edge, have:

- **Offset edge** pushes the edge out or pulls it in, square to itself.
- **Offset all** (*Offset* on the strip) grows or shrinks the whole outline by one distance. Press `Ctrl` to make a copy instead.
- **Insert point** adds a corner in its middle, then moves it. A right-click adds it where you clicked.

Each follows the pointer with snaps. Type a distance and press `Enter` to send it that far the way the pointer went, click to place it, or press `Esc` to stop.

## A face or a wire

A closed outline is a face: [Pull](masses) makes it a solid. **Remove face**, on the strip (or the Face switch in Properties), leaves a closed wire: lines only, and Pull makes each line a wall, open at both ends. **Make face** fills it again.

## Shafts

A selected shaft has a square grip on each corner, a round one on each edge and round grips for its base and its top, in every plan, section and 3D view it passes through.

## In sections

Select an element in a section: its plan grips slide along the section line, and its height grips move up and down.

## Section and elevation lines

Select a section or elevation mark in a plan: the square handles move its ends, the round middle handle moves the whole line, and the handle on the shaded band sets how deep it sees. See [Sections and elevations](sections).

## Crop boxes

A selected crop box has round handles on its faces and a square handle at its corner to move it. See [Crop boxes](crop-boxes).
