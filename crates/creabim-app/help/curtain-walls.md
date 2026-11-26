# Curtain walls

A **curtain wall** is a wall made of a grid of mullions holding glass or solid panels: a shopfront, a glazed façade.

## Drawing one

Use *Create › Curtain wall*: it starts the Wall tool with a curtain wall type. Or pick any curtain wall type in the Wall tool's **Type** menu. Then draw it like any wall.

![A curtain wall in 3D, with its settings in Properties](img/curtain-wall.jpg)

## Its grid

Select it. Under *Curtain wall* in *Properties*, **Grid along** and **Grid up** each have a **Rule**:

- **Count**: so many equal bays;
- **Fit**: the fewest equal bays no wider than a size, so they all stay the same;
- **Spacing**: a line every so far, **set out** from the *Start*, the *Centre* or the *End*. That decides where the odd bay goes: at the end, the same at both ends, or at the start.

Two checks make the grid follow the building:

- **Transoms at floor levels**: a transom where each floor meets the wall, so glass never runs past a slab edge;
- **Mullions on the grid axes**: an upright wherever a structural grid line crosses it.

Then the mullions' width and depth, and glass or solid panels with their thickness.

### Lines by hand

Open the curtain wall in **Edit mode** (Enter, a double-click, or Edit in *Properties*). Each grid line has a grip: slide an upright along the wall, or a transom up it, and type a distance if you like. Drop a line on its neighbour or on an edge and it goes. Lines set by hand stay where you put them when the rule changes. *Properties* says how many there are, and **Reset grid to the rules** puts the grid back.

A door in a curtain wall takes its panel. The glass over it sits on a transom at the door's head.

### Corners

Where two curtain walls meet at a corner, one of them carries a square **corner post** and the other stops against it, so a corner has one post, not two mullions side by side.

These usually come from its [type](types), so changing the type changes every curtain wall of it.

## Its mullions: one swap from an idea to a built system

**Profile** says how the mullions are made. Start with a quick block, and change it when the design settles; nothing else moves.

- **Box**: one block per mullion, the glass in the middle. Quick to read while you design.
- **Stick**: a body behind the glass, the glass held near the outside, and a **cap** standing proud of it outside. Most low- and mid-rise curtain walls are built this way.
- **Flush**: the glass is the outside face, with a dark sealed **joint** between panes and the frame inside.

**Transoms**, the horizontal mullions, can be *shallower* than the mullions, so the uprights lead the façade.

In plans at 1:50 and finer, a Stick or Flush wall is cut through its profile: cap, plate, body and the glass as two lines. At 1:100 and coarser it is drawn as blocks, so the plan stays readable. Sections, elevations and 3D always show the profile, and IFC keeps it, with each piece a mullion.

## Set in a wall

Draw a curtain wall along a wall, or move one into it, and it cuts its own opening there, like a door or window: the wall loses the curtain wall's length and height. The opening follows the curtain wall when you move it, and closes again when it leaves the wall. *Properties* says which wall it is set in. In IFC the wall gets an `IfcOpeningElement` that the curtain wall fills.

A curtain wall drawn on its own stands alone, as before.

## Under a roof

In a gable, select the curtain wall and press **Attach to roof / slab** (*Properties*, under *Top*), as for any wall. Its grid then runs up to the roof: every mullion and pane stops under a **head mullion** that follows the roof's slope, and the wall it is set in opens up to the roof for it. Detach it and it goes back to its own height, with the wall closed over it.

A curtain wall that isn't attached keeps its own height: the gable above it stays wall, even when the curtain wall is as tall as the gable's eaves.

## Doors in it

Place a door or window on a curtain wall like on any wall: it sits in a panel.

Curtain walls are drawn in plans, sections, elevations and 3D, and export to IFC as `IfcCurtainWall` with its mullions and panels.
