# The CreaBIM model JSON format

A plain JSON file that describes a building model: levels, grids, walls
with their types and layers, doors and windows placed by wall and offset,
slabs with holes, roofs, columns, beams, stairs, railings, extrusions and
sweeps, site elements, furniture from the catalogue, rooms, views and
sheets with views placed on them. Any script, in any language, can write
one, and the `creabim` command-line tool turns it into a project you can
open in the app. Nothing has to be compiled against CreaBIM's code.

```sh
creabim build house.json house.creabim     # build a project
creabim build --check house.json           # only check the file
creabim export-json house.creabim out.json # describe a project in the format
creabim json-schema creabim-model.schema.json
```

The format has a JSON Schema, [`creabim-model.schema.json`](creabim-model.schema.json).
Point an editor at it with a `"$schema"` field, and it completes and checks
the field names as you type. A bigger example, a small house with a site,
furniture and three sheets, is [`examples/small-house.json`](../examples/small-house.json).

## A full small example

A one-room studio on a level, with a door, two windows, a floor, a gable
roof, a plan, a section, a 3D view, a door schedule and one sheet.

```json
{
  "$schema": "https://github.com/chopinregis/CreaBIM/blob/main/docs/creabim-model.schema.json",
  "version": 1,
  "units": "m",
  "materials": [
    { "id": "brick", "library": "Brick, common red" },
    { "id": "roof tiles", "hatch": "diagonal", "colour": "#7a3b2e" }
  ],
  "types": [
    { "id": "Brick wall", "category": "wall", "thickness": 0.3, "material": "brick" },
    { "id": "Window 1000", "category": "window", "width": 1.0, "height": 1.2, "sill": 0.9 }
  ],
  "levels": [
    { "id": "Ground", "elevation": 0, "height": 2.7 },
    { "id": "Eaves", "elevation": 2.7 }
  ],
  "walls": [
    { "id": "S", "level": "Ground", "start": [0, 0], "end": [6, 0], "type": "Brick wall", "top_level": "Eaves" },
    { "id": "E", "level": "Ground", "start": [6, 0], "end": [6, 4], "type": "Brick wall", "top_level": "Eaves" },
    { "id": "N", "level": "Ground", "start": [6, 4], "end": [0, 4], "type": "Brick wall", "top_level": "Eaves" },
    { "id": "W", "level": "Ground", "start": [0, 4], "end": [0, 0], "type": "Brick wall", "top_level": "Eaves" }
  ],
  "openings": [
    { "id": "Door", "kind": "door", "wall": "S", "centre": 1.5, "width": 0.9, "height": 2.1 },
    { "id": "South window", "kind": "window", "wall": "S", "centre": 4.2, "type": "Window 1000" },
    { "id": "North window", "kind": "window", "wall": "N", "centre": 3, "type": "Window 1000", "width": 1.8 }
  ],
  "slabs": [
    { "id": "Floor", "level": "Ground", "thickness": 0.2,
      "outline": [[-0.15, -0.15], [6.15, -0.15], [6.15, 4.15], [-0.15, 4.15]] }
  ],
  "roofs": [
    { "id": "Roof", "level": "Eaves", "shape": "gable", "pitch": 35, "overhang": 0.3,
      "thickness": 0.2, "material": "roof tiles", "edge": 0,
      "outline": [[-0.15, -0.15], [6.15, -0.15], [6.15, 4.15], [-0.15, 4.15]] }
  ],
  "rooms": [
    { "id": "Studio", "number": "01", "level": "Ground", "at": [3, 2] }
  ],
  "views": [
    { "id": "Plan", "kind": "plan", "level": "Ground", "scale": 50 },
    { "id": "Section", "kind": "section", "start": [-2, 2.5], "end": [8, 2.5], "depth": 5 },
    { "id": "3D", "kind": "3d", "azimuth": 30, "altitude": 25 },
    { "id": "Doors", "kind": "schedule", "category": "doors" }
  ],
  "sheets": [
    { "id": "A101", "name": "Studio", "size": "A3",
      "viewports": [
        { "view": "Plan", "at": [100, 190], "crop": { "min": [-1.5, -1.5], "max": [7.5, 5.5] } },
        { "view": "Section", "at": [300, 220] },
        { "view": "3D", "at": [300, 120] },
        { "view": "Doors", "at": [100, 60] }
      ] }
  ]
}
```

## How a file is put together

- **The top level** is an object with a `version` (required) and lists:
  `materials`, `assemblies`, `types`, `levels`, `grid_groups`, `grids`,
  `walls`, `openings`, `slabs`, `roofs`, `columns`, `beams`, `stairs`,
  `railings`, `parts`, `site`, `objects`, `rooms`, `views`, `sheets`,
  `tags` and `notes`.
  Every list may be left out, and the lists may come in any order.
- **Ids.** Every element has an `id` you choose: any non-empty string,
  unique across the whole file (a wall and a view can't share one). Elements
  refer to each other by these ids: a door names its wall, a plan names its
  level, a viewport names its view. The id also names the element in the app
  unless you give a `name`. A sheet's id is its number ("A101").
- **References are checked by kind.** A door's `wall` must be the id of a
  wall, not of a level. An element may refer to one listed after it.
- **Units.** Lengths are in metres. A top-level `"units"` of `"mm"`, `"cm"`,
  `"ft"` or `"in"` makes every length in the file that unit instead. Angles
  (pitch, rotation, direction, azimuth) are always degrees. Positions and
  sizes on sheets (`at`, a sheet's `width` and `height`) are always paper
  millimetres. `display_units` only sets how the app shows lengths.
- **Points** are arrays: `[x, y]` in plan (x east, y north), `[x, y, z]`
  in space, `z` above the element's level.
- **Heights hang off levels.** Every building element names its `level`;
  its heights (`base_offset`, a slab's `top_offset`, a railing's path `z`)
  are measured from that level's elevation, so moving a level in the app
  moves what is on it.
- **Types.** A wall, door, window, slab, roof, column or beam may name a
  `type`; it then takes the type's values, and any value it gives itself
  overrides the type's for that element only (as in the app).
- **Defaults.** A field with a default may be left out. The reference
  below gives each one.
- **Colours** are `"#rrggbb"`, or `[r, g, b]` from 0 to 1.
- **No nulls.** Leave a field out instead of writing `null`.

### Placing things

- A wall runs along its **centreline** from `start` to `end`. Its exterior
  face is on its right, looking from start to end, unless `flip` is set.
  Give `top_level` to tie its top to a level (it then follows that level),
  or a `height`; left out, it is as tall as its level's `height`.
- An opening sits in its `wall`, `offset` along it from the wall's start to
  the opening's near edge, or with `centre` to its middle. `sill` is its
  bottom above the wall's base.
- A slab's outline is the slab's edge in plan; its top face is
  `top_offset` above its level and it is `thickness` deep below that.
- A roof's outline is the wall plate, where the slopes start; `overhang`
  reaches past it. A shed roof rises from outline edge `edge`; a gable
  roof's ridge runs along it.
- An object from the catalogue is placed by its origin, the middle of its
  back edge (or by its middle, with `"anchor": "centre"`); at a `rotation`
  of 0 its back is towards +y (north). `creabim catalogue` lists the keys.
- A room is the space the walls close off round its `at` point, unless
  you give it an `outline`.
- A section or elevation looks to the left of its line: a line drawn west
  to east (`[0, 0]` to `[10, 0]`) looks north.
- A viewport puts a view's drawing on a sheet: `at` is where on the sheet
  (mm from its lower left corner), `target` the drawing point that goes
  there (left out: the middle of the crop, or of the whole drawing), `crop`
  the part of the drawing shown.

## Checking a file

`creabim build --check file.json` reads the file and builds the model in
memory, and reports every problem it finds, each at its JSON path:

```text
house.json: $.walls[3].thikness: unknown field `thikness`: did you mean `thickness`?
house.json: $.openings[0].wall: there is no wall with the id "W9"; did you mean "W1"?
house.json: $.openings[2]: it runs from 3.600 to 4.500 m along its wall, which is 4.200 m long
house.json: $.views[1].level: `level` is not used by a section view
4 problems; nothing was built
```

Fields the format doesn't know are errors, never ignored, so a misspelt
field can't silently fall back to its default. An element with a problem
is left out of the check that follows, so what refers to it doesn't repeat
the message. Problems with the file's shape (unknown fields, wrong types,
missing fields) are reported first; what refers to what is checked once
the shape is right. A file with problems builds nothing; the command fails with a
non-zero exit code, which makes it usable in a script or a CI job.

Some things build but look wrong; those are printed as warnings (a window
whose top is above its wall's).

## Exporting

`creabim export-json project.creabim out.json` writes a project in the
format, ids made from the elements' names (with `-2`, `-3` added where
names repeat). Building the exported file gives back the same model for
everything the format covers: exporting that again gives the same file.
What the format can't describe yet is left out and listed on the terminal,
for example:

```text
left out: 14 Tag (annotation is not in the format yet)
left out: the title block template of 12 sheets (they get the built-in one)
```

## Versions and what comes next

The format is versioned by the top-level `version`. This is version 1.
CreaBIM refuses a file of a newer version with a message naming the version,
rather than misreading it. New elements and fields are added to the format
as new optional fields and lists, so a version 1 file stays valid; a change
that would make an existing file mean something else raises the version.

### Not yet in the format

These exist in CreaBIM but can't be written in a file yet. A model made
from a file can have them added in the app, and `export-json` lists any it
leaves out.

- Annotation: dimensions, spot levels, text styles, a tag's room fields,
  and note leaders that point at an element (rather than a place).
- Title block templates (sheets get the built-in title block) and mark
  numbering rules.
- Per-view settings: visibility (hidden categories and elements), plan
  graphics and view range, underlays, level heads, crop boxes of 3D views,
  view templates and style packs.
- Curtain walls and curtain wall types, and openings in curtain walls.
- Footings, shafts, ceilings, room separators, sketches, guides, free
  meshes (with their modifiers) and booleans between elements.
- Objects other than the starter catalogue's (a catalogue folder of your
  own, or objects imported from Blender, FreeCAD or IFC), and footing types.
- Wall joins switched off at a wall's end.
- Join rules between walls (which wall cuts which, cleanup switched off)
  and 2D detail components drawn in detail views. Callout views and layer
  join priorities are in the format.
- Elements added by work in progress (graphic styles and view ranges)
  will join the format as they land.

## Reference

Every object of the format, field by field. This section is made from the
same declarations the reader uses, so it can't drift from what CreaBIM
accepts; the schema is made from them too.

<!-- reference:start (made by `CREABIM_BLESS=1 cargo test -p creabim-spec`) -->

### LengthIn

A unit lengths are written in.

- `"m"`
- `"mm"`
- `"cm"`
- `"ft"`
- `"in"`

### DisplayLength

How lengths are shown.

- `"mm"`
- `"cm"`
- `"m"`
- `"ft-in"`: Feet and fractional inches: 3'-6 1/2".
- `"ft"`: Decimal feet.
- `"in"`: Fractional inches.

### DisplayUnits

The project's display units.

| Field | Type | | Description |
|---|---|---|---|
| `length` | [DisplayLength](#displaylength) | required |  |
| `precision` | integer | default `0` | Decimal places; for feet and inches, the smallest fraction of an inch as a power of two (3 is 1/8"). |

### Hatch

The pattern on a material's cut faces.

- `"empty"`
- `"solid"`
- `"diagonal"`
- `"cross"`
- `"brick"`
- `"concrete"`
- `"timber"`

### RatePer

What a material's rate is priced per.

- `"m2"`
- `"m3"`
- `"m"`
- `"each"`

### Material

What elements are made of: their colour in 3D and the hatch on their cut faces. Start from a library material by its name, or give the values yourself; any value given overrides the library's.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  | Shown in the app; the id when left out. |
| `library` | string |  | A material of CreaBIM's library, by name ("Concrete, cast in place"), to start from. |
| `hatch` | [Hatch](#hatch) |  |  |
| `colour` | colour |  |  |
| `category` | string |  | IFC's material category: concrete, brick, wood, steel, glass... |
| `description` | string |  |  |
| `roughness` | number |  | 0 mirror-smooth to 1 fully matte. |
| `metallic` | number |  | 0 for almost everything, 1 for bare metal. |
| `opacity` | number |  | 1 opaque; glass is about 0.3. |
| `rate` | number |  | Its price for the take-off, in the project's currency, for each `rate_per`. |
| `rate_per` | [RatePer](#rateper) |  | What `rate` prices: square metres (the default), cubic metres, metres or pieces. |

### AssemblyUsage

What an assembly is for.

- `"wall"`
- `"floor"`
- `"roof"`

### LayerFunction

What a layer does. The core runs from the first structure layer to the last.

- `"structure"`
- `"substrate"`
- `"insulation"`
- `"air-gap"`
- `"membrane"`
- `"finish"`

### Layer

One layer of an assembly.

| Field | Type | | Description |
|---|---|---|---|
| `function` | [LayerFunction](#layerfunction) | default `"structure"` |  |
| `material` | string |  | A material's id; none for an air gap. |
| `thickness` | number | required |  |
| `priority` | integer |  | Which layer wins where walls join: 1 cuts through 5. Left out, it follows the layer's function (structure 1, finish 5). |

### Assembly

A layered build-up. Walls list layers from the exterior face in; floors and roofs from the top down. An element built of one is as thick as its layers.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `usage` | [AssemblyUsage](#assemblyusage) | default `"wall"` |  |
| `layers` | array of [Layer](#layer) |  |  |
| `description` | string |  |  |

### TypeCategory

What a type is for.

- `"wall"`
- `"door"`
- `"window"`
- `"slab"`
- `"roof"`
- `"column"`
- `"beam"`

### Location

The line a wall is drawn along and kept on when it gets thicker. The exterior is on the wall's right, looking from start to end, unless the wall is flipped.

- `"centre"`
- `"core-centre"`
- `"exterior-face"`
- `"interior-face"`
- `"core-exterior"`
- `"core-interior"`
- `"layer-boundary"`: One of the boundaries between layers, named by `boundary`.

### Outline

An opening's shape in its wall.

- `"rectangle"`
- `"segmental"`: A segmental arch over upright sides, rising `rise`.
- `"round"`: A semicircular arch over upright sides.
- `"raked"`: A sloping head, falling `rise` along the opening.

### LeafStyle

What a door's leaves are made as.

- `"flush"`: A plain slab.
- `"panelled"`: Stiles and rails round four sunk panels.
- `"glazed"`: Stiles and rails round one light of glass.
- `"part-glazed"`: Glass above, a solid panel below.
- `"louvred"`: Stiles and rails round horizontal slats.
- `"vision-panel"`: A flush leaf with a narrow upright light.
- `"all-glass"`: A frameless leaf of toughened glass held by metal fittings (`fittings`), with a long pull handle.
- `"french"`: Stiles and rails round small panes between glazing bars.
- `"slim-steel"`: Thin black steel stiles and rails round small panes.

### GlassFittings

How an all glass leaf is held.

- `"patch"`: Type F: a patch fitting at each corner.
- `"bottom-rail"`: Type BP: a rail along the bottom, a patch fitting at the top.
- `"rails"`: Type A/P: a rail along the top and the bottom.

### Track

A column or row of an opening's grid.

| Field | Type | | Description |
|---|---|---|---|
| `share` | number | default `1.0` | Its share of the clear width or height, relative to the others ([1, 1] splits it equally). |
| `bar` | boolean | default `false` | A mullion or transom between it and the one before. |

### Mechanism

How a cell opens.

- `"fixed"`: A fixed light, or a sliding door's fixed panel.
- `"hinged"`: A leaf on hinges at one side: a door leaf, a casement.
- `"top-hung"`: A sash hinged at its top, opening out at the bottom: an awning.
- `"bottom-hung"`: A sash hinged at its bottom, opening in at the top: a hopper.
- `"louvred"`: Glass or timber blades turning together.
- `"surface-sliding"`: A leaf hung from a track on the wall's face.
- `"pocket"`: A leaf sliding into a cavity in the wall.
- `"sliding"`: A panel sliding on a track in the frame.
- `"pivot"`: A door leaf turning on a pivot set in from its hinge side.
- `"centre-pivot"`: A door leaf or window sash turning on a pivot at its middle, about an upright axis.
- `"double-action"`: A door leaf swinging both ways.
- `"folding"`: A door panel or window sash folding against its hand's jamb with its neighbours.
- `"tilt-turn"`: A sash turning on side hinges or tilting in on bottom hinges.
- `"vertical-sliding"`: A sash sliding up or down: a hung (sash) window.
- `"horizontal-pivot"`: A sash turning on pivots at the middle of its sides.
- `"sectional"`: A door of hinged horizontal panels running up under the ceiling.
- `"rolling"`: A slatted curtain rolling up into a coil box over the opening.
- `"revolving"`: Four wings turning in a drum.

### CellDisplay

How drawings show a cell.

- `"convention"`: As drawings usually do: a hinged door open in plan, the rest closed.
- `"closed"`
- `"open"`

### Cell

One cell of an opening's grid.

| Field | Type | | Description |
|---|---|---|---|
| `mechanism` | [Mechanism](#mechanism) | default `"fixed"` | How it opens. |
| `hand` | boolean | default `false` | Hinged (or sliding) towards the opening's end, not its start. |
| `flipped` | boolean | default `false` | Opens to the other side from its opening's own: from the side a door opens to, or the outside a window faces. |
| `active` | boolean | default `false` | Has the handle: the leaf opened first in a pair. |
| `track` | integer | default `0` | Which track a sliding panel runs on (0 or 1). |
| `display` | [CellDisplay](#celldisplay) | default `"convention"` | How drawings show it. |
| `panes_across` | integer | default `1` | Glass panes side by side, split by glazing bars. |
| `panes_up` | integer | default `1` | Glass panes one above the other. |
| `across` | integer | default `1` | How many columns it spans from its own, as a transom light over the whole opening; the places it covers are not cells of their own. |

### Operation

How a door or window opened before cells.

- `"standard"`: A door's single hinged leaf; a window's fixed glazing.
- `"double-swing"`: A door's pair of hinged leaves.
- `"surface-sliding"`: A door's leaf sliding on a track on the wall's face.
- `"surface-sliding-pair"`: A door's pair of leaves on a track on the wall's face, parting from the middle.
- `"pocket"`: A door's leaf sliding into a cavity in the wall.
- `"pocket-pair"`: A door's pair of leaves sliding into cavities either side.
- `"multi-slide"`: A door of two to four glazed panels on offset tracks, each fixed or sliding.

### Frame

How a door or window is built inside its opening: a frame round its outline, and inside it a grid of cells, each a fixed light or a panel that opens.

| Field | Type | | Description |
|---|---|---|---|
| `width` | number | default `0.06` | Face width of the frame members. |
| `depth` | number | default `0.1` | Depth of the frame through the wall. |
| `outline` | [Outline](#outline) | default `"rectangle"` | The opening's shape. |
| `rise` | number | default `0.0` | A segmental arch's rise from its springing to its crown, or a raked head's fall from its start to its end (negative: it falls towards the start), in metres. Not used by the other outlines. |
| `leaf` | [LeafStyle](#leafstyle) | default `"flush"` | What a door's leaves are made as. |
| `fittings` | [GlassFittings](#glassfittings) | default `"patch"` | How an `all-glass` leaf is held. |
| `leaf_panes_across` | integer |  | A `french` or `slim-steel` leaf's panes across, each leaf (none: as `leaf_panes_up` says, or about 300 mm wide each, 400 for slim steel). |
| `leaf_panes_up` | integer |  | Its panes up (none: about 360 mm high each, 450 for slim steel). Set both or neither. |
| `lining` | number | default `0.0` | A lining board's thickness on the reveal, room side (0: none). |
| `architrave` | number | default `0.0` | An architrave's face width round the opening, room side (0: none). |
| `threshold` | number | default `0.0` | A threshold's height under a door (0: none). |
| `fire_rating` | string |  | Declared fire resistance, information only: "FD30", "EI 30". |
| `acoustic_rating` | string |  | Declared sound reduction, information only: "Rw 32 dB". |
| `security_rating` | string |  | Declared security class, information only: "RC2". |
| `u_value` | number |  | Declared thermal transmittance, W/m²K, information only. |
| `accessible` | boolean | default `false` | Declared suitable for people with disabilities. |
| `columns` | array of [Track](#track) |  | The grid's columns from the opening's start (none: one). |
| `rows` | array of [Track](#track) |  | The grid's rows from the bottom (none: one). |
| `cells` | array of [Cell](#cell) |  | The cells, row by row from the bottom row, each row from the opening's start. None: a door's one hinged leaf or a window's one fixed light, or what `operation` describes. |
| `operation` | [Operation](#operation) |  | Before cells: how it opens, read only when `cells` is empty. |
| `active_share` | number |  | Before cells: a pair's active leaf share of the clear width. |
| `panels` | integer |  | Before cells: a multi-panel slider's panels. |
| `fixed` | array of integer |  | Before cells: a multi-panel slider's fixed panels, from 1. |
| `panes_across` | integer |  | Before cells: window panes side by side. |
| `panes_up` | integer |  | Before cells: window panes one above the other. |

### Rect

A rectangle by its lower left and upper right corners.

| Field | Type | | Description |
|---|---|---|---|
| `min` | [x, y] | required |  |
| `max` | [x, y] | required |  |

### Screen

The repeating unit of a screen wall (breeze block, louvres, lattice).

| Field | Type | | Description |
|---|---|---|---|
| `unit` | [x, y] | required | The unit's size, joints included: along the wall and up. |
| `holes` | array of [Rect](#rect) |  | The open rectangles of one unit, from its lower left corner. |
| `frame` | number | default `0.0` | A solid border this wide round the screen. |
| `stagger` | boolean | default `false` | Every other course shifted half a unit along. |

### Type

An element type: shared values its instances take, so editing the type in the app changes them all. An instance names it in its `type` field and may still set any of those values itself.

Which fields a category uses: wall: `thickness` (or `assembly`), `material`, `location`, `height`, `screen`; door and window: `width`, `height`, `sill`, `frame`, `material`; slab and roof: `thickness` (or `assembly`), `material`; column: `width` and `depth`, or `diameter`, and `material`; beam: `width`, `depth`, `material`.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `category` | [TypeCategory](#typecategory) | required |  |
| `description` | string |  |  |
| `thickness` | number |  |  |
| `assembly` | string |  | An assembly's id. |
| `material` | string |  | A material's id. |
| `location` | [Location](#location) |  | A wall type's location line. |
| `boundary` | integer |  | With location `layer-boundary`: which boundary, counted from the exterior face (0) to the interior face (the layer count). |
| `height` | number |  | A wall type's height for new walls; a door or window's height. |
| `width` | number |  |  |
| `sill` | number |  |  |
| `frame` | [Frame](#frame) |  |  |
| `depth` | number |  |  |
| `diameter` | number |  |  |
| `screen` | [Screen](#screen) |  | Makes a wall type a screen wall: a repeating unit with holes. |

### Level

A storey. Elements hang off a level, so moving it moves them.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `elevation` | number | required | Height of its floor above the project's zero. |
| `height` | number | default `3.0` | Floor-to-floor height: the height walls on it take unless told otherwise. |

### GridLabels

Grid labels.

- `"numbers"`
- `"letters"`: A, B, C... (I and O are skipped), then AA, AB...

### GridGroup

A grid system: how its lines are labelled and coloured.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `prefix` | string | default `""` | Put before every label: "A-" gives A-1, A-2... |
| `vertical` | [GridLabels](#gridlabels) | default `"letters"` | How lines running north-south are labelled, west to east. |
| `horizontal` | [GridLabels](#gridlabels) | default `"numbers"` | How lines running east-west are labelled, south to north. |
| `colour` | colour |  |  |

### Grid

A grid line.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `label` | string |  | Its label after its group's prefix; the id when left out. |
| `start` | [x, y] | required |  |
| `end` | [x, y] | required |  |
| `group` | string |  | A grid group's id. |

### Wall

A straight wall along its centreline on a level.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `mark` | string |  | Its mark, shown by tags and schedules; none numbers it automatically. |
| `level` | string | required | A level's id. |
| `start` | [x, y] | required |  |
| `end` | [x, y] | required |  |
| `type` | string |  | A wall type's id: the wall takes its thickness, assembly, material, location line and height. |
| `thickness` | number |  | Needed unless the wall has a type or an assembly. |
| `height` | number |  | Height above its base. Left out: up to `top_level` when given, else the type's height, else its level's height. |
| `base_offset` | number | default `0.0` | Its base above its level. |
| `top_level` | string |  | A level's id: the top stays `top_offset` above that level as levels move. |
| `top_offset` | number | default `0.0` |  |
| `material` | string |  |  |
| `assembly` | string |  |  |
| `location` | [Location](#location) |  |  |
| `boundary` | integer |  | With location `layer-boundary`: which boundary, counted from the exterior face (0) to the interior face (the layer count). |
| `flip` | boolean | default `false` | The exterior face on the left instead of the right. |
| `attach_top` | array of string |  | Roofs and slabs (ids) the top follows the underside of. |

### OpeningKind

What an opening is.

- `"door"`
- `"window"`
- `"hole"`

### ArrangementKind

What a group of windows makes.

- `"ribbon"`: Side by side in one wall, coupled.
- `"corner"`: Round a corner, one in each wall.
- `"bay"`: In a bay: a projecting run of short walls, a window in each.
- `"bow"`: In a bow: a projecting curve of short facets.

### CornerJoint

Where two windows meet at a corner.

- `"post"`: A corner post between the two frames.
- `"glass"`: Glass to glass, no post: the panes meet at a sealed joint.

### Arrangement

How a window stands with others. Each keeps its own frame; the windows of one group share its number.

| Field | Type | | Description |
|---|---|---|---|
| `kind` | [ArrangementKind](#arrangementkind) | required |  |
| `group` | integer | required | The group's number, the same for every window in it. |
| `corner_start` | boolean | default `false` | Its end at the wall's start reaches into the corner there. |
| `corner_end` | boolean | default `false` | Its end at the wall's end reaches into the corner there. |
| `joint` | [CornerJoint](#cornerjoint) | default `"post"` | How it meets the window round the corner. |
| `left_start` | boolean | default `false` | At its start's corner the other wall runs off to this wall's left, else to its right. |
| `left_end` | boolean | default `false` | The same at its end's corner. |
| `angle_start` | number |  | At its start's corner, the other wall's way off as an angle (radians) from this wall's direction towards its left. |
| `angle_end` | number |  | The same at its end's corner. |

### Opening

A door, window or bare hole in a wall, placed by its distance along the wall from the wall's start; or a roof window, placed in plan on its roof.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `mark` | string |  |  |
| `kind` | [OpeningKind](#openingkind) | required |  |
| `wall` | string | required | The host wall's id. |
| `offset` | number |  | From the wall's start to the opening's near edge. Give this or `centre`. |
| `centre` | number |  | From the wall's start to the opening's middle. |
| `type` | string |  | A door or window type's id: the opening takes its width, height, sill, frame and material. |
| `width` | number |  | Needed unless the opening has a type. |
| `height` | number |  | Needed unless the opening has a type. |
| `sill` | number |  | Its bottom above the wall's base. Left out: 0 for doors and holes, 0.9 m for windows (or the type's). |
| `material` | string |  |  |
| `frame` | [Frame](#frame) |  |  |
| `flip_hand` | boolean | default `false` | A door hinged on the jamb at the wall's end side. |
| `flip_facing` | boolean | default `false` | A door opening to the wall's exterior; a window facing its interior. |
| `roof_at` | [x, y] |  | A roof window's centre in plan. With it, `wall` is the id of the roof the window is set in, its width runs across the slope and its height up it, and `offset`, `centre` and `sill` are not used. |
| `arrangement` | [Arrangement](#arrangement) |  | Windows that make one thing together: side by side in a ribbon, round a corner, in a bay or a bow. Left out: on its own. |

### Slab

A floor slab: its outline pushed down from its top face.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `level` | string | required |  |
| `outline` | array of [x, y] |  | The corners in plan, either way round. |
| `bulges` | array of number |  | How each edge bends (corner i to i + 1): the tangent of a quarter of the arc's angle, positive bulging left. 0 or left out is straight. |
| `holes` | array of array of [x, y] |  | Holes through it, each an outline inside it. |
| `thickness` | number |  | Needed unless the slab has a type or an assembly. |
| `top_offset` | number | default `0.0` | Its top face above its level; negative for below. |
| `type` | string |  |  |
| `material` | string |  |  |
| `assembly` | string |  |  |

### RoofShape

A roof's shape.

- `"flat"`
- `"shed"`: One slope, rising from the `edge`.
- `"gable"`: Two slopes meeting at a ridge, with upright gable ends.
- `"hip"`: A slope from every edge.

### Roof

A roof over a wall-plate outline.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `level` | string | required |  |
| `outline` | array of [x, y] |  | Where the slopes start, in plan. |
| `shape` | [RoofShape](#roofshape) | default `"hip"` |  |
| `pitch` | number |  | Slope in degrees. Needed unless the roof is flat. |
| `overhang` | number | default `0.0` | How far it reaches past the outline, in plan. |
| `thickness` | number |  | Square to the slope. Needed unless the roof has a type or an assembly. |
| `base_offset` | number | default `0.0` |  |
| `edge` | integer | default `0` | The outline edge a shed roof rises from, or a gable roof's ridge runs along (edge i runs from corner i to i + 1). |
| `type` | string |  |  |
| `material` | string |  |  |
| `assembly` | string |  |  |
| `holes` | array of array of [x, y] |  | Holes cut upright through it (roof lights, a chimney). |

### Column

A column: a rectangular (`width` × `depth`) or round (`diameter`) profile standing on a level.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `mark` | string |  |  |
| `level` | string | required |  |
| `at` | [x, y] | required | The profile's centre in plan. |
| `type` | string |  |  |
| `width` | number |  |  |
| `depth` | number |  |  |
| `diameter` | number |  |  |
| `rotation` | number | default `0.0` | Turn of the profile, degrees anticlockwise. |
| `base_offset` | number | default `0.0` |  |
| `height` | number |  | A fixed height. Left out: up to `top_level`, else to the next level up. |
| `top_level` | string |  |  |
| `top_offset` | number | default `0.0` |  |
| `material` | string |  |  |

### Beam

A beam, its top `top_offset` below the top of its storey.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `mark` | string |  |  |
| `level` | string | required |  |
| `start` | [x, y] | required |  |
| `end` | [x, y] | required |  |
| `type` | string |  |  |
| `width` | number |  |  |
| `depth` | number |  | Height of the section. |
| `top_offset` | number | default `0.0` |  |
| `material` | string |  |  |

### Side

Left or right.

- `"left"`
- `"right"`

### Landing

A landing that turns a stair.

| Field | Type | | Description |
|---|---|---|---|
| `after` | integer | required | The number of risers before it. |
| `turn` | [Side](#side) | required |  |

### StairLine

Which line of a stair it was drawn by.

- `"walk"`
- `"left"`
- `"right"`

### StairFlight

A flight of a line stair given its own number of treads.

| Field | Type | | Description |
|---|---|---|---|
| `straight` | integer | required | The straight of the line it runs along, from 0. |
| `treads` | integer | required |  |

### StairLanding

A landing of a line stair made deeper: the flight before it stops this much sooner.

| Field | Type | | Description |
|---|---|---|---|
| `turn` | integer | required | The turn of the line it sits at, from 0. |
| `deeper` | number | required |  |

### StairTurnKind

What a turn of a stair becomes.

- `"landing"`
- `"winders"`

### StairTurn

A turn of a line stair given its own kind.

| Field | Type | | Description |
|---|---|---|---|
| `turn` | integer | required | The turn of the line, from 0. |
| `kind` | [StairTurnKind](#stairturnkind) | required |  |
| `winders` | integer | default `0` | How many winders (0: one for each 30° it turns). |

### StairBuild

How a stair is built.

- `"concrete"`
- `"stepped"`
- `"folded_plate"`
- `"stringers"`
- `"cut_stringers"`
- `"spine"`
- `"cantilever"`
- `"post"`

### StairFinish

The finishes over a stair's structure. Its risers are measured finish to finish; the structure sits the tread finish lower.

| Field | Type | | Description |
|---|---|---|---|
| `tread` | number | default `0.0` | The finish on each tread, landing and winder: its thickness. |
| `tread_material` | string |  | A material id. |
| `riser` | number | default `0.0` | The finish on each riser's face (none where risers are open). |
| `riser_material` | string |  | A material id. |
| `nosing` | number | default `0.0` | How far each tread's finish runs out past the riser below. |

### Stair

A stair: drawn by a line (its walk line or a side), with a landing at each corner; or, without a line, one flight or two with a landing between.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `level` | string | required |  |
| `start` | [x, y] | required | The middle of the first riser, in plan. |
| `direction` | number | default `0.0` | Which way the first flight climbs, degrees anticlockwise from x. |
| `width` | number | required |  |
| `max_riser` | number | default `0.18` | The tallest riser allowed; the risers share the height equally. |
| `tread` | number | default `0.28` | The depth of each tread. |
| `height` | number |  | A fixed rise. Left out: up to the next level. |
| `base_offset` | number | default `0.0` |  |
| `waist` | number | default `0.15` | Thickness of the sloping slab under the steps. |
| `landing` | [Landing](#landing) |  |  |
| `material` | string |  |  |
| `line` | array of [x, y] |  | The line it was drawn by, in plan, from `start`. Its corners are landings. |
| `bends` | array of number |  | How far each straight of `line` bends into an arc: degrees its direction turns through, to the left when positive. |
| `drawn` | [StairLine](#stairline) | default `"walk"` | Which line of the stair `line` is. |
| `flights` | array of [StairFlight](#stairflight) |  | Flights given their own number of treads; the others take the rest. |
| `landings` | array of [StairLanding](#stairlanding) |  | Landings made deeper than the line leaves them. |
| `turns` | [StairTurnKind](#stairturnkind) | default `"landing"` | What its turns become. |
| `winders` | integer | default `0` | How many winders at each turn (0: one for each 30° it turns). |
| `turn_kinds` | array of [StairTurn](#stairturn) |  | Turns given their own kind. |
| `build` | [StairBuild](#stairbuild) | default `"concrete"` | How it is built: what carries its treads. |
| `finish` | [StairFinish](#stairfinish) |  | What its treads, risers and landings are finished in. |
| `rules` | string | default `"ontario"` | The rule set it is checked against: `comfort`, `ontario`, `us`, `uk`, or the name of one of the project's own. |
| `ramp` | number |  | A ramp, not steps: its steepest slope, rise over run (1:12 is 0.0833). A ramp's `tread` and `max_riser` follow from it. |
| `typical` | string |  | Stacked: the id of the stair at the foot of the stack this one repeats on a typical floor. It is kept the same as that stair. |
| `split` | boolean | default `false` | A split stair: its line is one arm; the stair is that arm and its mirror image, sharing one flight twice as wide up to the first landing. |
| `up_label` | [x, y] |  | Where its UP label stands, moved from before its first flight: the offset in plan. Left out: before the flight. |

### RailExtend

How a stair's handrail runs on past its bottom and top nosings.

| Field | Type | | Description |
|---|---|---|---|
| `bottom` | number | default `0.3` |  |
| `top` | number | default `0.3` |  |
| `slope_bottom` | boolean | default `true` | The rail keeps its slope for one going before running level. |

### RailInfill

What fills a guard below its top rail.

- `"rail"`
- `"balusters"`
- `"glass"`

### Railing

A railing along a path, or along one side of a stair.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `level` | string | required |  |
| `path` | array of [x, y, z] |  | Points of its path, heights above the level. Not used when it follows a stair. |
| `height` | number | default `1.0` |  |
| `post_spacing` | number | default `1.0` |  |
| `stair` | string |  | A stair's id: the railing follows its `side`. |
| `side` | [Side](#side) |  |  |
| `material` | string |  |  |
| `extend` | [RailExtend](#railextend) |  | On a stair: how far the handrail runs on past its ends. |
| `on_wall` | boolean | default `false` | A handrail on a wall: brackets, ends back to the wall. |
| `infill` | [RailInfill](#railinfill) | default `"rail"` | What fills a guard below its top rail. |
| `guard_height` | number |  | A guard taller than its handrail: its top rail's height; the handrail stays at `height` on its posts. |
| `newels` | boolean | default `false` | Heavier posts at its ends and corners; a steep rail stops at one and starts again higher. |

### Plane

A plane anywhere in the model: its origin (height above the level) and two square unit axes along it.

| Field | Type | | Description |
|---|---|---|---|
| `origin` | [x, y, z] | required |  |
| `x` | [x, y, z] | required |  |
| `y` | [x, y, z] | required |  |

### WallEdge

A wall's top or base.

- `"top"`
- `"base"`

### Part

An extrusion of a plan outline, or a sweep of a profile along a path. A void cuts what it meets (a firebox in a chimney, a hole through a slab) instead of being solid.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `level` | string | required |  |
| `outline` | array of [x, y] |  | An extrusion's outline in plan. |
| `holes` | array of array of [x, y] |  | An extrusion's holes. |
| `base` | number | default `0.0` | An extrusion's bottom above the level. |
| `height` | number |  | An extrusion's height. |
| `plane` | [Plane](#plane) |  | An extrusion drawn on a face: its outline lies on this plane, and `base` and `height` run along the plane's normal (`x` × `y`). |
| `profile` | array of [x, y] |  | A sweep's section: `x` across the path (positive to its left), `y` up from it. |
| `path` | array of [x, y, z] |  | A sweep's path through points, `z` above the level. |
| `closed` | boolean | default `false` | The path runs from its last point back to its first. |
| `face_normal` | [x, y, z] |  | A path drawn on a face: the face's outward normal, which the profile's `y` runs along instead of up. |
| `wall` | string |  | A sweep along a wall's id, at its top or base: it follows the wall (a coping, a plinth). |
| `edge` | [WallEdge](#walledge) | default `"top"` |  |
| `offset` | number | default `0.0` | Lifts a wall path off its edge. |
| `extend` | [x, y] |  | Runs a wall path on past the wall's start and end. |
| `void` | boolean | default `false` |  |
| `material` | string |  |  |
| `ifc_class` | string |  | The IFC class it exports as. |

### SiteKind

What a site element is.

- `"terrain"`
- `"area"`
- `"pad"`: A building pad: `outline`, `top`, `thickness` (its base course), `cut` and `fill` slopes.
- `"plant"`
- `"property"`: A lot: its boundary, edges and planning rules.
- `"road"`: A road along a centreline.
- `"drain"`: A drain run, set out in `drain`.
- `"tank"`: A buried tank at `at`, set out in `tank`.
- `"path"`: A path on the ground, set out in `walk`.
- `"investigation"`: A borehole, trial pit or probe at `at`, logged in `investigation`.
- `"spot"`: A spot level on a terrain: `at`, and `top` its height above the level. The ground of the terrain it lies on passes through it.
- `"ground_line"`: A contour or a break line drawn on a terrain: one entry in `lines`. The ground of the terrain it lies on passes through it.

### GroundLine

A line a terrain passes through: `[x, y, z]` points, `z` above the level. A contour has one height; a `sharp` line is a break line (a kerb, a bank top) the ground folds along.

| Field | Type | | Description |
|---|---|---|---|
| `points` | array of [x, y, z] |  |  |
| `sharp` | boolean | default `false` |  |
| `closed` | boolean | default `false` |  |

### GroundPad

A levelled platform worked into a terrain (or, with a `depth`, a pit or trench dug into it): its outline, its `level` above the terrain's level, and its banks, each the run for one unit of height (`cut` where it is dug in, `fill` where it is made up; 0 is an upright face).

| Field | Type | | Description |
|---|---|---|---|
| `outline` | array of [x, y] |  |  |
| `level` | number | required |  |
| `cut` | number | default `1.0` |  |
| `fill` | number | default `2.0` |  |
| `depth` | number | default `0.0` | An excavation: above 0, the floor is this far below the ground all over (a pit, a trench) and `level` is not used. |

### SoilLayer

A layer of soil under a terrain, from the ground as surveyed down.

| Field | Type | | Description |
|---|---|---|---|
| `name` | string | required |  |
| `thickness` | number | required |  |

### GroundShading

How a terrain is shaded in plans.

- `"none"`
- `"slope"`
- `"aspect"`
- `"height"`

### PatternKind

A surface pattern.

- `"none"`
- `"units"`: Flags, blocks, setts.
- `"lines"`: Boards, decking.
- `"stipple"`: Gravel, sand.

### SitePattern

An area's surface: units (`size`, `stagger`, `angle`), lines (`spacing`, `angle`) or stipple (`spacing`).

| Field | Type | | Description |
|---|---|---|---|
| `kind` | [PatternKind](#patternkind) | required |  |
| `size` | [x, y] |  |  |
| `stagger` | boolean | default `false` |  |
| `angle` | number | default `0.0` |  |
| `spacing` | number |  |  |

### PlantForm

A plant's form.

- `"tree"`
- `"conifer"`
- `"palm"`
- `"shrub"`

### LotEdge

What an edge of a property is: it sets the edge's setback.

- `"front"`
- `"side"`
- `"rear"`
- `"open"`: No setback.

### LotEasement

A strip `width` wide inside the lot along edge `edge` (0 is the first), kept clear of building.

| Field | Type | | Description |
|---|---|---|---|
| `name` | string | required |  |
| `edge` | integer | required |  |
| `width` | number | required |  |

### LotEdgeRule

What edge `edge` of a lot (0 is the first) has of its own: a `setback` other than its kind's, and a `note` written on its setback line below its kind's setback, `note_offset` from there where it was dragged.

| Field | Type | | Description |
|---|---|---|---|
| `edge` | integer | required |  |
| `setback` | number |  |  |
| `note` | string |  |  |
| `note_offset` | [x, y] |  |  |

### LotRules

A property's planning rules, metres; a limit of 0 is none.

| Field | Type | | Description |
|---|---|---|---|
| `front` | number | default `6.0` |  |
| `side` | number | default `3.0` |  |
| `rear` | number | default `6.0` |  |
| `height` | number | default `0.0` | The highest the building may reach above the project's zero. |
| `coverage` | number | default `0.0` | How much of the lot may be covered, 0 to 1. |
| `floor_ratio` | number | default `0.0` | Floor area allowed per unit of lot area. |
| `bearings` | boolean | default `true` | Bearings and lengths written along the edges in plans. |
| `setbacks` | boolean | default `true` | Setback lines and the buildable area drawn in plans. |
| `angle` | number | default `0.0` | Angle planes: degrees from the horizontal, rising inward from `angle_from` above each edge with a setback; 0 is none. |
| `angle_from` | number | default `2.5` |  |
| `envelope` | boolean | default `true` | The envelope shown in 3D. |
| `easements` | array of [LotEasement](#loteasement) |  | Strips along edges kept clear of building. |
| `own` | array of [LotEdgeRule](#lotedgerule) |  | Edges with a setback or a note of their own. |

### ParkingSide

Which side of a road, looking along it.

- `"left"`
- `"right"`
- `"both"`

### RoadParking

Parking bays along a road's kerb: `width` along it (square to the bay), `length` out, at `angle` degrees to it (90 square on, 0 parallel).

| Field | Type | | Description |
|---|---|---|---|
| `side` | [ParkingSide](#parkingside) | default `"left"` |  |
| `width` | number | default `2.5` |  |
| `length` | number | default `5.0` |  |
| `angle` | number | default `90.0` |  |

### SiteRoad

A road's section, metres: the carriageway kerb to kerb, its fall across (per cent) from its crown or one way to its right, kerbs, footpaths each side, its make-up's depth, its banks into the ground (run for each metre of height) and its parking bays.

| Field | Type | | Description |
|---|---|---|---|
| `width` | number | default `5.5` |  |
| `fall` | number | default `2.5` |  |
| `crowned` | boolean | default `true` |  |
| `kerb` | number | default `0.1` |  |
| `footpath` | number | default `0.0` |  |
| `thickness` | number | default `0.3` |  |
| `cut` | number | default `1.5` |  |
| `fill` | number | default `2.0` |  |
| `works` | boolean | default `true` | Worked into the terrain under it. |
| `parking` | [RoadParking](#roadparking) |  |  |
| `gullies` | number | default `0.0` | Gullies along its low edges this far apart, with a channel by the kerb; 0 is none. |
| `radius` | number | default `0.0` | Its bends rounded to this radius on the centreline; 0 leaves them sharp. |

### DrainService

What a drain carries.

- `"foul"`
- `"surface"`
- `"combined"`
- `"field"`: A drainage field: perforated pipe in a gravel trench.

### SiteDrain

A drain run, metres: `path` in plan from its head down to its outfall, a pipe `diameter` across laid at `fall` per cent (1.25 is 1 in 80), its invert `depth` under the ground at its head, with a chamber `chamber` square inside at each point, in a trench `trench` wide on a bed `bed` deep.

| Field | Type | | Description |
|---|---|---|---|
| `path` | array of [x, y] |  |  |
| `service` | [DrainService](#drainservice) | default `"foul"` |  |
| `diameter` | number | default `0.1` |  |
| `fall` | number | default `1.25` |  |
| `depth` | number | default `0.6` |  |
| `chambers` | boolean | default `true` |  |
| `chamber` | number | default `0.6` |  |
| `trench` | number | default `0.6` |  |
| `bed` | number | default `0.1` |  |

### TankService

What a tank is for.

- `"septic"`
- `"treatment"`
- `"rainwater"`
- `"attenuation"`
- `"soakaway"`

### SiteTank

A buried tank, metres: `length` by `width` inside, `depth` deep, turned `angle` degrees anticlockwise, its top `cover` under the ground, its walls `wall` thick, dug in with `working` room round it.

| Field | Type | | Description |
|---|---|---|---|
| `service` | [TankService](#tankservice) | default `"septic"` |  |
| `length` | number | required |  |
| `width` | number | required |  |
| `depth` | number | required |  |
| `angle` | number | default `0.0` |  |
| `cover` | number | default `0.3` |  |
| `wall` | number | default `0.15` |  |
| `working` | number | default `0.5` |  |

### PathSurface

What a path's surface is.

- `"gravel"`
- `"paving"`
- `"asphalt"`
- `"bark"`

### SitePath

A path on the ground, metres: along `path` in plan, `width` wide between its edging, following the ground, its surface and sub-base `thickness` deep, with `edging` this wide each side (0 for none).

| Field | Type | | Description |
|---|---|---|---|
| `path` | array of [x, y] |  |  |
| `width` | number | default `1.2` |  |
| `surface` | [PathSurface](#pathsurface) | default `"gravel"` |  |
| `edging` | number | default `0.05` |  |
| `thickness` | number | default `0.15` |  |
| `radius` | number | default `0.0` | Its bends rounded to this radius; 0 leaves them sharp. |

### ProbeType

How a spot was investigated.

- `"borehole"`
- `"trial_pit"`
- `"probe"`

### SoilType

The soil family a layer is drawn with.

- `"topsoil"`
- `"made_ground"`
- `"clay"`
- `"silt"`
- `"sand"`
- `"gravel"`
- `"peat"`
- `"rock"`

### ProbeLayer

A layer of a log: from the foot of the one above down to `to`.

| Field | Type | | Description |
|---|---|---|---|
| `to` | number | required |  |
| `soil` | [SoilType](#soiltype) | default `"clay"` |  |
| `unit` | string |  | The report's name for it across the site: "GT3". |
| `description` | string |  | The report's description, as written. |

### ProbeWater

Water struck at `depth`, rising to `rest` if the report says.

| Field | Type | | Description |
|---|---|---|---|
| `depth` | number | required |  |
| `rest` | number |  |  |
| `note` | string |  |  |

### TestType

What a test was.

- `"spt"`: Standard penetration test, N.
- `"vane"`: Hand shear vane, kPa.
- `"penetrometer"`: Pocket penetrometer, kPa.
- `"dynamic"`: Dynamic probe, blows for 100 mm.

### ProbeTest

A test at `depth` and its result; `text` as the report writes it when the number alone doesn't say it all ("50/220 mm").

| Field | Type | | Description |
|---|---|---|---|
| `depth` | number | required |  |
| `kind` | [TestType](#testtype) | default `"spt"` |  |
| `value` | number | required |  |
| `text` | string |  |  |

### SiteProbe

A borehole, trial pit or probe of a soils report, as the report logs it: depths below its own ground level, `ground` that level above the level's datum as the report gives it (left out, the existing ground's height there).

| Field | Type | | Description |
|---|---|---|---|
| `kind` | [ProbeType](#probetype) | default `"borehole"` |  |
| `ground` | number |  |  |
| `depth` | number | required | How deep it went. |
| `layers` | array of [ProbeLayer](#probelayer) |  | Its layers from the top down, each to the depth of its foot. |
| `water` | array of [ProbeWater](#probewater) |  |  |
| `tests` | array of [ProbeTest](#probetest) |  |  |
| `method` | string |  |  |
| `date` | string |  |  |
| `report` | string |  |  |
| `notes` | string |  |  |

### SiteElement

A site element: terrain, a hard landscape area, an earthwork (a pad, a pit or a trench) or a plant.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `kind` | [SiteKind](#sitekind) | required |  |
| `level` | string | required |  |
| `outline` | array of [x, y] |  | Terrain and area: the outline. |
| `holes` | array of array of [x, y] |  | An area's holes. |
| `top` | number | default `0.0` | Terrain and area: the top above the level. |
| `thickness` | number |  | Terrain and area: how deep. |
| `points` | array of [x, y, z] |  | Terrain: spot heights `[x, y, z]`, `z` above the level. |
| `contour` | number | default `0.0` | Terrain: a contour every so much height; 0 draws none. |
| `lines` | array of [GroundLine](#groundline) |  | Terrain: the contours and break lines it passes through. |
| `survey` | boolean | default `false` | Terrain: a survey, its edge takes its height from the data nearest it (else the edge stays at `top`). |
| `faceted` | boolean | default `false` | Terrain: drawn as flat triangles between its points, not smoothed through them. |
| `major` | integer | default `0` | Terrain: every so many contours one is heavier; 0 is every fifth. |
| `labels` | boolean | default `false` | Terrain: heavier contours carry their height. |
| `levels` | boolean | default `false` | Terrain: spot heights carry their height. |
| `shift` | [x, y, z] |  | Terrain: what was taken off a survey's coordinates when it was brought in, metres: a survey point is its place here plus this. |
| `pads` | array of [GroundPad](#groundpad) |  | Terrain: the pads worked into the ground, in order. |
| `under_buildings` | boolean | default `false` | Terrain: the ground is dug out under the slabs standing on it. |
| `strata` | array of [SoilLayer](#soillayer) |  | Terrain: the soil under it, layer by layer from the top. |
| `analysis` | [GroundShading](#groundshading) | default `"none"` | Terrain: shaded in plans by slope, aspect or height. |
| `flow` | boolean | default `false` | Terrain: arrows in plans for which way water runs. |
| `pattern` | [SitePattern](#sitepattern) |  | An area's surface pattern. |
| `at` | [x, y] |  | A plant's position. |
| `base` | number | default `0.0` | A plant's base above the level. |
| `plant` | [PlantForm](#plantform) | default `"tree"` |  |
| `canopy` | number |  | A plant's crown across. |
| `height` | number |  | A plant's height. |
| `cut` | number |  | A pad, pit or trench: its bank where it digs in, run over rise. |
| `fill` | number |  | A pad: its bank where it is made up, run over rise. |
| `depth` | number | default `0.0` | A pit or a trench (kind `pad`): how deep below the ground; 0 is a pad at `top`. |
| `width` | number | default `0.0` | A trench (kind `pad`): its width; its line is `path`. |
| `only_cut` | boolean | default `false` | A pad that only digs: lower ground under it stays. |
| `retained` | array of integer |  | A pad or a pit (kind `pad`): the edges cut upright whatever its banks, edge 0 from the first corner of `outline` to the second. A retaining wall standing on one is a wall element of its own, and is not tied to it here. |
| `radius` | number | default `0.0` | A trench (kind `pad`): its bends rounded to this radius. |
| `edges` | array of [LotEdge](#lotedge) |  | A property: what each edge of its outline is, edge `i` from corner `i` to the next (the first front, on four sides the third rear, the rest sides when left out). |
| `rules` | [LotRules](#lotrules) |  | A property: the planning rules it is built to. |
| `path` | array of [x, y, z] |  | A road: its centreline, `[x, y, z]` points, `z` its crown's level above the level there. |
| `road` | [SiteRoad](#siteroad) |  | A road: its section and how it meets the ground. |
| `drain` | [SiteDrain](#sitedrain) |  | A drain: its run, pipe and fall. |
| `tank` | [SiteTank](#sitetank) |  | A tank: what it is and its size; it stands at `at`. |
| `walk` | [SitePath](#sitepath) |  | A path: its line, width and surface. |
| `investigation` | [SiteProbe](#siteprobe) |  | A borehole, trial pit or probe: its log; it stands at `at`. |
| `material` | string |  |  |

### Anchor

Which point of an object `at` gives.

- `"origin"`: The middle of its back edge.
- `"centre"`: The middle of its footprint.

### LightBeam

How a fitting's light spreads.

- `"narrow"`: A spot or a downlight: a tight cone.
- `"medium"`: A wider cone, as over a worktop.
- `"wide"`: A pendant or a ceiling light: across the room.
- `"all-round"`: A bare bulb or a globe: the same every way.

### ObjectLight

The light a fitting gives in a render.

| Field | Type | | Description |
|---|---|---|---|
| `on` | boolean | default `true` | False: it is drawn, and gives no light. |
| `lumens` | number | required | Its output, lumens: about 800 for a 60 W bulb of old. |
| `kelvin` | number | default `2700.0` | Its colour, kelvin: 2700 warm, 4000 neutral, 6500 daylight. |
| `beam` | [LightBeam](#lightbeam) | default `"wide"` |  |
| `group` | string |  | The group it is dimmed with in Visuals; left out, its type's name. |

### Object

Furniture or a fixture from CreaBIM's catalogue.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `mark` | string |  |  |
| `level` | string | required |  |
| `catalogue` | string | required | The catalogue object's key: "starter/sofa-3" (`creabim catalogue` lists them; "starter/" may be left out). |
| `at` | [x, y] | required | Where it goes in plan: its origin (the middle of its back edge), or its middle with `anchor` "centre". |
| `anchor` | [Anchor](#anchor) | default `"origin"` |  |
| `rotation` | number | default `0.0` | Turn, degrees anticlockwise. At 0 its back is towards +y. |
| `offset` | number |  | Its floor above the level (for one hung from the ceiling, its top below the ceiling). Left out: the catalogue's. |
| `light` | [ObjectLight](#objectlight) |  | The light it gives in a render, when not its kind's usual one. It belongs to the object's type: every object of the same `catalogue` key shares it, so state it the same on each (or on one; the last stated is kept). |

### Room

A room: the space the walls close off round `at`, or an outline drawn for it.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  |  |
| `number` | string | default `""` |  |
| `level` | string | required |  |
| `at` | [x, y] | required | A point inside it, where its tag sits. |
| `outline` | array of [x, y] |  | An outline of its own; left out, the walls round `at` bound it. |
| `height` | number |  | Its top above its level. Left out: to the ceiling, or the level's height. |
| `floor_finish` | string | default `""` |  |
| `wall_finish` | string | default `""` |  |
| `ceiling_finish` | string | default `""` |  |
| `occupancy` | string | default `""` |  |
| `department` | string | default `""` |  |

### ViewKind

What a view is.

- `"plan"`
- `"section"`: Cut along its line.
- `"elevation"`: Seen from its line, nothing cut.
- `"3d"`: A parallel projection of the whole model.
- `"schedule"`
- `"long_section"`: A drain run unfolded along its length, with its levels.
- `"borehole_log"`: The log of a borehole, trial pit or probe.
- `"walk_section"`: A stair unrolled along its walk line, with its headroom.

### ViewStyle

How a view draws.

- `"hidden-line"`
- `"shaded"`
- `"wireframe"`

### ScheduleCategory

What a schedule lists.

- `"doors"`
- `"windows"`
- `"walls"`
- `"slabs"`
- `"structure"`: Columns and beams.
- `"furniture"`
- `"fixtures"`
- `"rooms"`
- `"materials"`: The take-off by material.
- `"drainage"`: Drain chambers and tanks with their levels.
- `"stairs"`: Stairs with their rise, going, headroom and checks.
- `"parking"`: Car parks' bays by type, against what is required.

### Callout

The box of a parent view a callout enlarges, in that view's drawing coordinates.

| Field | Type | | Description |
|---|---|---|---|
| `parent` | string | required | The parent view's id. |
| `min` | [x, y] | required |  |
| `max` | [x, y] | required |  |

### SiteGround

Which ground a view shows.

- `"after"`
- `"before"`
- `"both"`: After, with the ground as it was dashed where it changed.

### ViewSite

How a view shows the site: the ground before or after its works (or after with the ground as it was dashed), and each kind of site information on or off.

| Field | Type | | Description |
|---|---|---|---|
| `ground` | [SiteGround](#siteground) | default `"after"` |  |
| `contours` | boolean | default `true` |  |
| `levels` | boolean | default `true` |  |
| `cut_fill` | boolean | default `false` |  |
| `banks` | boolean | default `true` |  |
| `limit` | boolean | default `false` | The line round what the works change. |
| `legal` | boolean | default `true` | Property lines, setbacks and easements. |
| `roads` | boolean | default `true` |  |
| `services` | boolean | default `true` | Drains, chambers and tanks. |
| `investigation` | boolean | default `true` | Boreholes, trial pits and probes. |

### View

A view: a plan of a level, a section or elevation along a line, a 3D view, or a schedule. Which fields it uses depends on its `kind`: plan: `level`, `cut_height`, `ceiling_plan`; section and elevation: `start`, `end`, `depth`; 3d: `azimuth`, `altitude`; schedule: `category`, `by_type`.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required |  |
| `name` | string |  | Its title; the id when left out. |
| `kind` | [ViewKind](#viewkind) | required |  |
| `scale` | number | default `100.0` | The scale's denominator: 100 is 1:100. |
| `style` | [ViewStyle](#viewstyle) |  | Left out: shaded for 3D views, hidden line for the rest. |
| `level` | string |  | A plan's level. |
| `cut_height` | number | default `1.2` | A plan's cut above its level. |
| `ceiling_plan` | boolean | default `false` | A reflected ceiling plan. |
| `room_colours` | boolean | default `false` | Each room washed in a colour of its own. |
| `start` | [x, y] |  | A section or elevation's line. It looks to the left of start → end: a line drawn west to east looks north. |
| `end` | [x, y] |  |  |
| `depth` | number |  | How far a section or elevation looks. |
| `azimuth` | number | default `30.0` | A 3D view's compass bearing: 0 from the south, anticlockwise. |
| `altitude` | number | default `30.0` | A 3D view's angle above the horizon. |
| `category` | [ScheduleCategory](#schedulecategory) |  | What a schedule lists. |
| `drain` | string |  | A long section: the drain it follows. |
| `probe` | string |  | A borehole log: the borehole, trial pit or probe it logs. |
| `stair` | string |  | A walk-line section: the stair it unrolls. |
| `by_type` | boolean | default `false` | A schedule's rows grouped by type. |
| `callout` | [Callout](#callout) |  | Makes it a callout of another view: an enlarged part of it. Give it the same kind and line (or level) as its parent. |
| `site` | [ViewSite](#viewsite) |  | A plan, section, elevation or 3D view: how it shows the site. |

### PaperSize

ISO paper sizes, landscape.

- `"A0"`
- `"A1"`
- `"A2"`
- `"A3"`
- `"A4"`

### Viewport

A view placed on a sheet.

| Field | Type | | Description |
|---|---|---|---|
| `view` | string | required | A view's id. |
| `at` | [x, y] | required | Where on the sheet, in millimetres from its lower left corner. |
| `target` | [x, y] |  | The drawing point (in the file's units, as the drawing's x and y) put at `at`. Left out: the middle of the crop, or of the whole drawing. |
| `crop` | [Rect](#rect) |  | Only this part of the drawing is shown (drawing coordinates). |
| `fit` | string |  | A stair's id: the crop follows that stair, its footprint with a margin round it, as it moves or grows; `crop` is used once it is gone. |

### Sheet

A drawing sheet, with views placed on it. Its id is its number.

| Field | Type | | Description |
|---|---|---|---|
| `id` | string | required | Its number: "A101". |
| `name` | string |  | Its title; the number when left out. |
| `size` | [PaperSize](#papersize) |  | An ISO paper size, landscape. Give this or `width` and `height`. |
| `width` | number |  | In millimetres. |
| `height` | number |  |  |
| `fields` | object of strings |  | Title block fields by name. |
| `viewports` | array of [Viewport](#viewport) |  |  |

### Tag

A tag in a view, showing an element's mark (a door's, a window's) or a room's name, number and area. It moves with its element.

| Field | Type | | Description |
|---|---|---|---|
| `view` | string | required | A view's id. |
| `element` | string | required | The id of the element it tags. |
| `offset` | [x, y] | default `[0.0, 0.0]` | Where it sits, from the element's middle, in drawing units. |
| `elbow` | [x, y] |  | A bend in its leader, from the element's middle. |
| `end` | [x, y] |  | Where its leader ends, from the element's middle; left out, at the middle. |

### NoteSymbol

A symbol placed with the Symbol tool, its middle on the note's `at`.

| Field | Type | | Description |
|---|---|---|---|
| `kind` | string | required | `points`, `slopes`, `centerlines`, `materials` or `mep`. |
| `style` | string |  | A style of that kind, as `slopes.ramp_rise_arrow`; left out, the kind's pick in the project's style pack. |
| `angle` | number | default `0.0` | Its turn, degrees anticlockwise. |

### Note

Text in a view (drawing coordinates, in the file's units) or on a sheet (paper millimetres).

| Field | Type | | Description |
|---|---|---|---|
| `view` | string | required | A view's id, or a sheet's. |
| `at` | [x, y] | required | Where its first line starts: its left end, mid-height. |
| `text` | string | required | Line breaks (`\n`) start new lines. |
| `leader` | [x, y] |  | A leader from the text to this point, in the same units as `at`. |
| `elbow` | [x, y] |  | A bend in the leader. |
| `colour` | colour |  |  |
| `symbol` | [NoteSymbol](#notesymbol) |  | Drawn as a CreaBIM symbol instead of as text, `text` going in the symbol's slot when it has one. |

### Model

A whole model: the file's top level. Every list may be left out. Ids are chosen by you, are unique across the whole file, and are what elements use to refer to each other (a door names its wall's id, a plan names its level's id).

| Field | Type | | Description |
|---|---|---|---|
| `$schema` | string |  | Where an editor finds this schema; ignored by CreaBIM. |
| `version` | integer | required | The format version the file is written to. This CreaBIM reads 1. |
| `units` | [LengthIn](#lengthin) | default `"m"` | The unit every length in the file is in, except the sizes and positions on sheets, which are always paper millimetres. Angles are always degrees. |
| `display_units` | [DisplayUnits](#displayunits) |  | How the project shows and takes lengths (the model is metres whatever this says). |
| `datum` | number |  | The site datum: how high the project's zero stands above sea level, in metres whatever `units` says. Site levels are written above it. |
| `materials` | array of [Material](#material) |  |  |
| `assemblies` | array of [Assembly](#assembly) |  | Layered build-ups for walls, floors and roofs. |
| `types` | array of [Type](#type) |  | Element types: wall types, door and window types, slab, roof, column and beam types. |
| `levels` | array of [Level](#level) |  |  |
| `grid_groups` | array of [GridGroup](#gridgroup) |  |  |
| `grids` | array of [Grid](#grid) |  |  |
| `walls` | array of [Wall](#wall) |  |  |
| `openings` | array of [Opening](#opening) |  | Doors, windows and bare holes, each in a wall. |
| `slabs` | array of [Slab](#slab) |  |  |
| `roofs` | array of [Roof](#roof) |  |  |
| `columns` | array of [Column](#column) |  |  |
| `beams` | array of [Beam](#beam) |  |  |
| `stairs` | array of [Stair](#stair) |  |  |
| `railings` | array of [Railing](#railing) |  |  |
| `parts` | array of [Part](#part) |  | Extrusions and sweeps, solid or voids that cut what they meet. |
| `site` | array of [SiteElement](#siteelement) |  | Terrain, hard landscape and planting. |
| `objects` | array of [Object](#object) |  | Furniture and fixtures from the catalogue. |
| `rooms` | array of [Room](#room) |  |  |
| `views` | array of [View](#view) |  |  |
| `sheets` | array of [Sheet](#sheet) |  |  |
| `tags` | array of [Tag](#tag) |  | Tags of doors, windows, rooms and other elements, in views. |
| `notes` | array of [Note](#note) |  | Text written in a view or on a sheet. |

<!-- reference:end -->
