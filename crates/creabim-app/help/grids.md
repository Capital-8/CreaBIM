# Grids

**Grid lines** are the labelled reference lines that structure a building: A, B, C one way and 1, 2, 3 the other. Columns and walls are placed on them, and everything snaps to them.

## Drawing grid lines

1. Pick the **Grid** tool.
2. Click two points for each line. `Shift` keeps it straight.

For a curved line, pick **Arc** in the tool options: click its two ends, then a point it passes through.

Each line takes the next label of its direction. Sections and elevations draw every grid line their section line crosses, or would cross in what they see (an elevation drawn outside the building), upright through the levels with its bubble on top. Select one in a section to drag its foot or top by the square grips: the heights are that view's own. Lines show in plans, sections and in 3D, with bubbles at their ends.

## Grid systems

A project with several buildings can have a **grid system** for each, with its own prefix ("A-", "B-"), labels and colour.

- The Grid tool's *Grid system* menu in the tool options picks the active system, makes a new one, renumbers the lines in order, selects its lines, moves selected lines into a system, and opens *Edit the grid system…*.
- A grid system's *Properties*: name, prefix, shape, turn, origin, numbering, colour, what new lines get, and the spacing to lay its lines out from.

Changing the prefix relabels the whole system.

### Turned and curved systems

- **Turn** gives a system its own north. A building set at 32° counts its lines in its own directions: lines across it are A, B, C…, lines along it 1, 2, 3…, whatever their angle on the site.
- **Shape · Curved** makes a system of rings round its **origin** and spokes out of it. Rings are counted outwards, spokes anticlockwise.

### Diagonal lines

A line more than 20° off both of its system's directions is a **diagonal**. Diagonals are counted on their own, so A, B, C and 1, 2, 3 stay unbroken: A', B', C'… by default. *Numbering* sets their style and the mark after them. IFC calls these a grid's W axes.

### Laying out a whole grid

In *Lay out lines*, type the spacing each way, as the engineer gives it: `4000, 3400, 5000` or `3 x 4000`. A curved system takes ring spacing as lengths and spoke spacing in degrees. **Add lines** places them all from the origin in one undo step, each running past the last line crossing it by *Run past*. Lines already there are kept, so you can add to the spacing and press it again.

![The starter house's grid in 3D: lines A to E and 1 to 3](img/grids-3d.jpg)

## A grid line's own settings

Select a grid line to see, in *Properties*:

- **Runs**: which way it runs in its system, and so how it is counted.
- **Bubbles**: at its start, its end, or both. New lines get what their system's *New lines* says.
- **Elbows**: a crowded bubble can step aside on a jog. Drag the bubble's square grip sideways in a plan, or type how far in the *Elbows* box, a row for each bubble; 0 puts it back on the line.
- **Shape**: a **Line**, or an **Arc** through its two ends, its *Rise* how far its middle stands off the straight line. Drag the square grip at its middle to bow it more or less.
- **Levels**: *All*, or *Some* and tick the levels it shows on. A wing's grid need not cross the floors it doesn't reach.
- **In this plan**: draw it shorter at either end in the plan in front only; the line itself, and every other view, keep its full length. *Draw it whole here* undoes it.

## Moving grid lines

Select a line in a plan and drag its grips, or pick it in 3D by its bubble or a line and drag it square to itself. [Extend](align#extend-to-it) stretches a grid line's end to a reference.

## Hiding grids

Each view's [visibility](visibility) can hide grids, or a single grid system. *Show* in the 3D view's buttons does the same for 3D.
