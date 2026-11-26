//! The format itself: every object a file may hold, field by field. The
//! doc comments here are the reference in `docs/json-format.md` and the
//! descriptions in `docs/creabim-model.schema.json`, so they are written
//! for someone writing a file, not for someone reading the code.

use crate::json::Colour;
use crate::{spec_enum, spec_struct};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

/// The newest version of the format this CreaBIM reads and writes.
pub const VERSION: u32 = 1;

spec_struct! {
    /// A whole model: the file's top level. Every list may be left out.
    /// Ids are chosen by you, are unique across the whole file, and are
    /// what elements use to refer to each other (a door names its wall's
    /// id, a plan names its level's id).
    pub struct Model {
        /// Where an editor finds this schema; ignored by CreaBIM.
        schema as "$schema": Option<String>,
        /// The format version the file is written to. This CreaBIM reads 1.
        version: u32,
        /// The unit every length in the file is in, except the sizes and
        /// positions on sheets, which are always paper millimetres.
        /// Angles are always degrees.
        units: LengthIn = LengthIn::Metres,
        /// How the project shows and takes lengths (the model is metres
        /// whatever this says).
        display_units: Option<DisplayUnits>,
        /// The site datum: how high the project's zero stands above sea
        /// level, in metres whatever `units` says. Site levels are written
        /// above it.
        datum: Option<f64>,
        materials: Vec<Material>,
        /// Layered build-ups for walls, floors and roofs.
        assemblies: Vec<Assembly>,
        /// Element types: wall types, door and window types, slab, roof,
        /// column and beam types.
        types: Vec<Type>,
        levels: Vec<Level>,
        grid_groups: Vec<GridGroup>,
        grids: Vec<Grid>,
        walls: Vec<Wall>,
        /// Doors, windows and bare holes, each in a wall.
        openings: Vec<Opening>,
        slabs: Vec<Slab>,
        roofs: Vec<Roof>,
        columns: Vec<Column>,
        beams: Vec<Beam>,
        stairs: Vec<Stair>,
        railings: Vec<Railing>,
        /// Extrusions and sweeps, solid or voids that cut what they meet.
        parts: Vec<Part>,
        /// Terrain, hard landscape and planting.
        site: Vec<SiteElement>,
        /// Furniture and fixtures from the catalogue.
        objects: Vec<Object>,
        rooms: Vec<Room>,
        views: Vec<View>,
        sheets: Vec<Sheet>,
        /// Tags of doors, windows, rooms and other elements, in views.
        tags: Vec<Tag>,
        /// Text written in a view or on a sheet.
        notes: Vec<Note>,
    }
}

spec_enum! {
    /// A unit lengths are written in.
    pub enum LengthIn {
        Metres = "m",
        Millimetres = "mm",
        Centimetres = "cm",
        Feet = "ft",
        Inches = "in",
    }
}

impl LengthIn {
    /// Metres in one of it.
    pub fn metres(self) -> f64 {
        match self {
            LengthIn::Metres => 1.0,
            LengthIn::Millimetres => 0.001,
            LengthIn::Centimetres => 0.01,
            LengthIn::Feet => 0.3048,
            LengthIn::Inches => 0.0254,
        }
    }
}

spec_struct! {
    /// The project's display units.
    pub struct DisplayUnits {
        length: DisplayLength,
        /// Decimal places; for feet and inches, the smallest fraction of an
        /// inch as a power of two (3 is 1/8").
        precision: u32 = 0,
    }
}

spec_enum! {
    /// How lengths are shown.
    pub enum DisplayLength {
        Millimetres = "mm",
        Centimetres = "cm",
        Metres = "m",
        /// Feet and fractional inches: 3'-6 1/2".
        FeetInches = "ft-in",
        /// Decimal feet.
        Feet = "ft",
        /// Fractional inches.
        Inches = "in",
    }
}

spec_struct! {
    /// What elements are made of: their colour in 3D and the hatch on
    /// their cut faces. Start from a library material by its name, or give
    /// the values yourself; any value given overrides the library's.
    pub struct Material {
        id: String,
        /// Shown in the app; the id when left out.
        name: Option<String>,
        /// A material of CreaBIM's library, by name ("Concrete, cast in
        /// place"), to start from.
        library: Option<String>,
        hatch: Option<Hatch>,
        colour: Option<Colour>,
        /// IFC's material category: concrete, brick, wood, steel, glass...
        category: Option<String>,
        description: Option<String>,
        /// 0 mirror-smooth to 1 fully matte.
        roughness: Option<f64>,
        /// 0 for almost everything, 1 for bare metal.
        metallic: Option<f64>,
        /// 1 opaque; glass is about 0.3.
        opacity: Option<f64>,
        /// Its price for the take-off, in the project's currency, for each
        /// `rate_per`.
        rate: Option<f64>,
        /// What `rate` prices: square metres (the default), cubic metres,
        /// metres or pieces.
        rate_per: Option<RatePer>,
    }
}

spec_enum! {
    /// What a material's rate is priced per.
    pub enum RatePer {
        SquareMetre = "m2",
        CubicMetre = "m3",
        Metre = "m",
        Each = "each",
    }
}

spec_enum! {
    /// The pattern on a material's cut faces.
    pub enum Hatch {
        Empty = "empty",
        Solid = "solid",
        Diagonal = "diagonal",
        Cross = "cross",
        Brick = "brick",
        Concrete = "concrete",
        Timber = "timber",
    }
}

spec_struct! {
    /// A layered build-up. Walls list layers from the exterior face in;
    /// floors and roofs from the top down. An element built of one is as
    /// thick as its layers.
    pub struct Assembly {
        id: String,
        name: Option<String>,
        usage: AssemblyUsage = AssemblyUsage::Wall,
        layers: Vec<Layer>,
        description: Option<String>,
    }
}

spec_enum! {
    /// What an assembly is for.
    pub enum AssemblyUsage {
        Wall = "wall",
        Floor = "floor",
        Roof = "roof",
    }
}

spec_struct! {
    /// One layer of an assembly.
    pub struct Layer {
        function: LayerFunction = LayerFunction::Structure,
        /// A material's id; none for an air gap.
        material: Option<String>,
        thickness: f64,
        /// Which layer wins where walls join: 1 cuts through 5. Left out,
        /// it follows the layer's function (structure 1, finish 5).
        priority: Option<u32>,
    }
}

spec_enum! {
    /// What a layer does. The core runs from the first structure layer
    /// to the last.
    pub enum LayerFunction {
        Structure = "structure",
        Substrate = "substrate",
        Insulation = "insulation",
        AirGap = "air-gap",
        Membrane = "membrane",
        Finish = "finish",
    }
}

spec_struct! {
    /// An element type: shared values its instances take, so editing the
    /// type in the app changes them all. An instance names it in its
    /// `type` field and may still set any of those values itself.
    ///
    /// Which fields a category uses: wall: `thickness` (or `assembly`),
    /// `material`, `location`, `height`, `screen`; door and window:
    /// `width`, `height`, `sill`, `frame`, `material`; slab and roof:
    /// `thickness` (or `assembly`), `material`; column: `width` and
    /// `depth`, or `diameter`, and `material`; beam: `width`, `depth`,
    /// `material`.
    pub struct Type {
        id: String,
        name: Option<String>,
        category: TypeCategory,
        description: Option<String>,
        thickness: Option<f64>,
        /// An assembly's id.
        assembly: Option<String>,
        /// A material's id.
        material: Option<String>,
        /// A wall type's location line.
        location: Option<Location>,
        /// With location `layer-boundary`: which boundary, counted from
        /// the exterior face (0) to the interior face (the layer count).
        boundary: Option<u32>,
        /// A wall type's height for new walls; a door or window's height.
        height: Option<f64>,
        width: Option<f64>,
        sill: Option<f64>,
        frame: Option<Frame>,
        depth: Option<f64>,
        diameter: Option<f64>,
        /// Makes a wall type a screen wall: a repeating unit with holes.
        screen: Option<Screen>,
    }
}

spec_enum! {
    /// What a type is for.
    pub enum TypeCategory {
        Wall = "wall",
        Door = "door",
        Window = "window",
        Slab = "slab",
        Roof = "roof",
        Column = "column",
        Beam = "beam",
    }
}

spec_struct! {
    /// The repeating unit of a screen wall (breeze block, louvres,
    /// lattice).
    pub struct Screen {
        /// The unit's size, joints included: along the wall and up.
        unit: DVec2,
        /// The open rectangles of one unit, from its lower left corner.
        holes: Vec<Rect>,
        /// A solid border this wide round the screen.
        frame: f64 = 0.0,
        /// Every other course shifted half a unit along.
        stagger: bool = false,
    }
}

spec_struct! {
    /// A rectangle by its lower left and upper right corners.
    pub struct Rect {
        min: DVec2,
        max: DVec2,
    }
}

spec_enum! {
    /// The line a wall is drawn along and kept on when it gets thicker.
    /// The exterior is on the wall's right, looking from start to end,
    /// unless the wall is flipped.
    pub enum Location {
        Centre = "centre",
        CoreCentre = "core-centre",
        ExteriorFace = "exterior-face",
        InteriorFace = "interior-face",
        CoreExterior = "core-exterior",
        CoreInterior = "core-interior",
        /// One of the boundaries between layers, named by `boundary`.
        LayerBoundary = "layer-boundary",
    }
}

spec_struct! {
    /// How a door or window is built inside its opening: a frame round
    /// its outline, and inside it a grid of cells, each a fixed light or
    /// a panel that opens.
    pub struct Frame {
        /// Face width of the frame members.
        width: f64 = 0.06,
        /// Depth of the frame through the wall.
        depth: f64 = 0.1,
        /// The opening's shape.
        outline: Outline = Outline::Rectangle,
        /// A segmental arch's rise from its springing to its crown, or a
        /// raked head's fall from its start to its end (negative: it falls
        /// towards the start), in metres. Not used by the other outlines.
        rise: f64 = 0.0,
        /// What a door's leaves are made as.
        leaf: LeafStyle = LeafStyle::Flush,
        /// How an `all-glass` leaf is held.
        fittings: GlassFittings = GlassFittings::Patch,
        /// A `french` or `slim-steel` leaf's panes across, each leaf (none: as
        /// `leaf_panes_up` says, or about 300 mm wide each, 400 for slim
        /// steel).
        leaf_panes_across: Option<u32>,
        /// Its panes up (none: about 360 mm high each, 450 for slim
        /// steel). Set both or neither.
        leaf_panes_up: Option<u32>,
        /// A lining board's thickness on the reveal, room side (0: none).
        lining: f64 = 0.0,
        /// An architrave's face width round the opening, room side (0: none).
        architrave: f64 = 0.0,
        /// A threshold's height under a door (0: none).
        threshold: f64 = 0.0,
        /// Declared fire resistance, information only: "FD30", "EI 30".
        fire_rating: Option<String>,
        /// Declared sound reduction, information only: "Rw 32 dB".
        acoustic_rating: Option<String>,
        /// Declared security class, information only: "RC2".
        security_rating: Option<String>,
        /// Declared thermal transmittance, W/m²K, information only.
        u_value: Option<f64>,
        /// Declared suitable for people with disabilities.
        accessible: bool = false,
        /// The grid's columns from the opening's start (none: one).
        columns: Vec<Track>,
        /// The grid's rows from the bottom (none: one).
        rows: Vec<Track>,
        /// The cells, row by row from the bottom row, each row from the
        /// opening's start. None: a door's one hinged leaf or a window's
        /// one fixed light, or what `operation` describes.
        cells: Vec<Cell>,
        /// Before cells: how it opens, read only when `cells` is empty.
        operation: Option<Operation>,
        /// Before cells: a pair's active leaf share of the clear width.
        active_share: Option<f64>,
        /// Before cells: a multi-panel slider's panels.
        panels: Option<u32>,
        /// Before cells: a multi-panel slider's fixed panels, from 1.
        fixed: Option<Vec<u32>>,
        /// Before cells: window panes side by side.
        panes_across: Option<u32>,
        /// Before cells: window panes one above the other.
        panes_up: Option<u32>,
    }
}

spec_enum! {
    /// What a door's leaves are made as.
    pub enum LeafStyle {
        /// A plain slab.
        Flush = "flush",
        /// Stiles and rails round four sunk panels.
        Panelled = "panelled",
        /// Stiles and rails round one light of glass.
        Glazed = "glazed",
        /// Glass above, a solid panel below.
        PartGlazed = "part-glazed",
        /// Stiles and rails round horizontal slats.
        Louvred = "louvred",
        /// A flush leaf with a narrow upright light.
        VisionPanel = "vision-panel",
        /// A frameless leaf of toughened glass held by metal fittings
        /// (`fittings`), with a long pull handle.
        AllGlass = "all-glass",
        /// Stiles and rails round small panes between glazing bars.
        French = "french",
        /// Thin black steel stiles and rails round small panes.
        SlimSteel = "slim-steel",
    }
}

spec_enum! {
    /// How an all glass leaf is held.
    pub enum GlassFittings {
        /// Type F: a patch fitting at each corner.
        Patch = "patch",
        /// Type BP: a rail along the bottom, a patch fitting at the top.
        BottomRail = "bottom-rail",
        /// Type A/P: a rail along the top and the bottom.
        Rails = "rails",
    }
}

spec_enum! {
    /// An opening's shape in its wall.
    pub enum Outline {
        Rectangle = "rectangle",
        /// A segmental arch over upright sides, rising `rise`.
        Segmental = "segmental",
        /// A semicircular arch over upright sides.
        Round = "round",
        /// A sloping head, falling `rise` along the opening.
        Raked = "raked",
    }
}

spec_struct! {
    /// A column or row of an opening's grid.
    pub struct Track {
        /// Its share of the clear width or height, relative to the
        /// others ([1, 1] splits it equally).
        share: f64 = 1.0,
        /// A mullion or transom between it and the one before.
        bar: bool = false,
    }
}

spec_struct! {
    /// One cell of an opening's grid.
    pub struct Cell {
        /// How it opens.
        mechanism: Mechanism = Mechanism::Fixed,
        /// Hinged (or sliding) towards the opening's end, not its start.
        hand: bool = false,
        /// Opens to the other side from its opening's own: from the side a
        /// door opens to, or the outside a window faces.
        flipped: bool = false,
        /// Has the handle: the leaf opened first in a pair.
        active: bool = false,
        /// Which track a sliding panel runs on (0 or 1).
        track: u32 = 0,
        /// How drawings show it.
        display: CellDisplay = CellDisplay::Convention,
        /// Glass panes side by side, split by glazing bars.
        panes_across: u32 = 1,
        /// Glass panes one above the other.
        panes_up: u32 = 1,
        /// How many columns it spans from its own, as a transom light over
        /// the whole opening; the places it covers are not cells of their own.
        across: u32 = 1,
    }
}

spec_enum! {
    /// How a cell opens.
    pub enum Mechanism {
        /// A fixed light, or a sliding door's fixed panel.
        Fixed = "fixed",
        /// A leaf on hinges at one side: a door leaf, a casement.
        Hinged = "hinged",
        /// A sash hinged at its top, opening out at the bottom: an awning.
        TopHung = "top-hung",
        /// A sash hinged at its bottom, opening in at the top: a hopper.
        BottomHung = "bottom-hung",
        /// Glass or timber blades turning together.
        Louvred = "louvred",
        /// A leaf hung from a track on the wall's face.
        SurfaceSliding = "surface-sliding",
        /// A leaf sliding into a cavity in the wall.
        Pocket = "pocket",
        /// A panel sliding on a track in the frame.
        Sliding = "sliding",
        /// A door leaf turning on a pivot set in from its hinge side.
        Pivot = "pivot",
        /// A door leaf or window sash turning on a pivot at its middle,
        /// about an upright axis.
        CentrePivot = "centre-pivot",
        /// A door leaf swinging both ways.
        DoubleAction = "double-action",
        /// A door panel or window sash folding against its hand's jamb
        /// with its neighbours.
        Folding = "folding",
        /// A sash turning on side hinges or tilting in on bottom hinges.
        TiltTurn = "tilt-turn",
        /// A sash sliding up or down: a hung (sash) window.
        VerticalSliding = "vertical-sliding",
        /// A sash turning on pivots at the middle of its sides.
        HorizontalPivot = "horizontal-pivot",
        /// A door of hinged horizontal panels running up under the ceiling.
        Sectional = "sectional",
        /// A slatted curtain rolling up into a coil box over the opening.
        Rolling = "rolling",
        /// Four wings turning in a drum.
        Revolving = "revolving",
    }
}

spec_enum! {
    /// How drawings show a cell.
    pub enum CellDisplay {
        /// As drawings usually do: a hinged door open in plan, the rest closed.
        Convention = "convention",
        Closed = "closed",
        Open = "open",
    }
}

spec_enum! {
    /// How a door or window opened before cells.
    pub enum Operation {
        /// A door's single hinged leaf; a window's fixed glazing.
        Standard = "standard",
        /// A door's pair of hinged leaves.
        DoubleSwing = "double-swing",
        /// A door's leaf sliding on a track on the wall's face.
        SurfaceSliding = "surface-sliding",
        /// A door's pair of leaves on a track on the wall's face, parting
        /// from the middle.
        SurfaceSlidingPair = "surface-sliding-pair",
        /// A door's leaf sliding into a cavity in the wall.
        Pocket = "pocket",
        /// A door's pair of leaves sliding into cavities either side.
        PocketPair = "pocket-pair",
        /// A door of two to four glazed panels on offset tracks, each
        /// fixed or sliding.
        MultiSlide = "multi-slide",
    }
}

spec_struct! {
    /// A storey. Elements hang off a level, so moving it moves them.
    pub struct Level {
        id: String,
        name: Option<String>,
        /// Height of its floor above the project's zero.
        elevation: f64,
        /// Floor-to-floor height: the height walls on it take unless
        /// told otherwise.
        height: f64 = 3.0,
    }
}

spec_struct! {
    /// A grid system: how its lines are labelled and coloured.
    pub struct GridGroup {
        id: String,
        name: Option<String>,
        /// Put before every label: "A-" gives A-1, A-2...
        prefix: String = String::new(),
        /// How lines running north-south are labelled, west to east.
        vertical: GridLabels = GridLabels::Letters,
        /// How lines running east-west are labelled, south to north.
        horizontal: GridLabels = GridLabels::Numbers,
        colour: Option<Colour>,
    }
}

spec_enum! {
    /// Grid labels.
    pub enum GridLabels {
        Numbers = "numbers",
        /// A, B, C... (I and O are skipped), then AA, AB...
        Letters = "letters",
    }
}

spec_struct! {
    /// A grid line.
    pub struct Grid {
        id: String,
        name: Option<String>,
        /// Its label after its group's prefix; the id when left out.
        label: Option<String>,
        start: DVec2,
        end: DVec2,
        /// A grid group's id.
        group: Option<String>,
    }
}

spec_struct! {
    /// A straight wall along its centreline on a level.
    pub struct Wall {
        id: String,
        name: Option<String>,
        /// Its mark, shown by tags and schedules; none numbers it
        /// automatically.
        mark: Option<String>,
        /// A level's id.
        level: String,
        start: DVec2,
        end: DVec2,
        /// A wall type's id: the wall takes its thickness, assembly,
        /// material, location line and height.
        r#type: Option<String>,
        /// Needed unless the wall has a type or an assembly.
        thickness: Option<f64>,
        /// Height above its base. Left out: up to `top_level` when given,
        /// else the type's height, else its level's height.
        height: Option<f64>,
        /// Its base above its level.
        base_offset: f64 = 0.0,
        /// A level's id: the top stays `top_offset` above that level as
        /// levels move.
        top_level: Option<String>,
        top_offset: f64 = 0.0,
        material: Option<String>,
        assembly: Option<String>,
        location: Option<Location>,
        /// With location `layer-boundary`: which boundary, counted from
        /// the exterior face (0) to the interior face (the layer count).
        boundary: Option<u32>,
        /// The exterior face on the left instead of the right.
        flip: bool = false,
        /// Roofs and slabs (ids) the top follows the underside of.
        attach_top: Vec<String>,
    }
}

spec_struct! {
    /// A door, window or bare hole in a wall, placed by its distance
    /// along the wall from the wall's start; or a roof window, placed in
    /// plan on its roof.
    pub struct Opening {
        id: String,
        name: Option<String>,
        mark: Option<String>,
        kind: OpeningKind,
        /// The host wall's id.
        wall: String,
        /// From the wall's start to the opening's near edge. Give this
        /// or `centre`.
        offset: Option<f64>,
        /// From the wall's start to the opening's middle.
        centre: Option<f64>,
        /// A door or window type's id: the opening takes its width,
        /// height, sill, frame and material.
        r#type: Option<String>,
        /// Needed unless the opening has a type.
        width: Option<f64>,
        /// Needed unless the opening has a type.
        height: Option<f64>,
        /// Its bottom above the wall's base. Left out: 0 for doors and
        /// holes, 0.9 m for windows (or the type's).
        sill: Option<f64>,
        material: Option<String>,
        frame: Option<Frame>,
        /// A door hinged on the jamb at the wall's end side.
        flip_hand: bool = false,
        /// A door opening to the wall's exterior; a window facing its
        /// interior.
        flip_facing: bool = false,
        /// A roof window's centre in plan. With it, `wall` is the id of
        /// the roof the window is set in, its width runs across the slope
        /// and its height up it, and `offset`, `centre` and `sill` are not
        /// used.
        roof_at: Option<DVec2>,
        /// Windows that make one thing together: side by side in a
        /// ribbon, round a corner, in a bay or a bow. Left out: on its own.
        arrangement: Option<Arrangement>,
    }
}

spec_struct! {
    /// How a window stands with others. Each keeps its own frame; the
    /// windows of one group share its number.
    pub struct Arrangement {
        kind: ArrangementKind,
        /// The group's number, the same for every window in it.
        group: u32,
        /// Its end at the wall's start reaches into the corner there.
        corner_start: bool = false,
        /// Its end at the wall's end reaches into the corner there.
        corner_end: bool = false,
        /// How it meets the window round the corner.
        joint: CornerJoint = CornerJoint::Post,
        /// At its start's corner the other wall runs off to this wall's
        /// left, else to its right.
        left_start: bool = false,
        /// The same at its end's corner.
        left_end: bool = false,
        /// At its start's corner, the other wall's way off as an angle
        /// (radians) from this wall's direction towards its left.
        angle_start: Option<f64>,
        /// The same at its end's corner.
        angle_end: Option<f64>,
    }
}

spec_enum! {
    /// What a group of windows makes.
    pub enum ArrangementKind {
        /// Side by side in one wall, coupled.
        Ribbon = "ribbon",
        /// Round a corner, one in each wall.
        Corner = "corner",
        /// In a bay: a projecting run of short walls, a window in each.
        Bay = "bay",
        /// In a bow: a projecting curve of short facets.
        Bow = "bow",
    }
}

spec_enum! {
    /// Where two windows meet at a corner.
    pub enum CornerJoint {
        /// A corner post between the two frames.
        Post = "post",
        /// Glass to glass, no post: the panes meet at a sealed joint.
        Glass = "glass",
    }
}

spec_enum! {
    /// What an opening is.
    pub enum OpeningKind {
        Door = "door",
        Window = "window",
        Hole = "hole",
    }
}

spec_struct! {
    /// A floor slab: its outline pushed down from its top face.
    pub struct Slab {
        id: String,
        name: Option<String>,
        level: String,
        /// The corners in plan, either way round.
        outline: Vec<DVec2>,
        /// How each edge bends (corner i to i + 1): the tangent of a
        /// quarter of the arc's angle, positive bulging left. 0 or left
        /// out is straight.
        bulges: Vec<f64>,
        /// Holes through it, each an outline inside it.
        holes: Vec<Vec<DVec2>>,
        /// Needed unless the slab has a type or an assembly.
        thickness: Option<f64>,
        /// Its top face above its level; negative for below.
        top_offset: f64 = 0.0,
        r#type: Option<String>,
        material: Option<String>,
        assembly: Option<String>,
    }
}

spec_struct! {
    /// A roof over a wall-plate outline.
    pub struct Roof {
        id: String,
        name: Option<String>,
        level: String,
        /// Where the slopes start, in plan.
        outline: Vec<DVec2>,
        shape: RoofShape = RoofShape::Hip,
        /// Slope in degrees. Needed unless the roof is flat.
        pitch: Option<f64>,
        /// How far it reaches past the outline, in plan.
        overhang: f64 = 0.0,
        /// Square to the slope. Needed unless the roof has a type or an
        /// assembly.
        thickness: Option<f64>,
        base_offset: f64 = 0.0,
        /// The outline edge a shed roof rises from, or a gable roof's
        /// ridge runs along (edge i runs from corner i to i + 1).
        edge: u32 = 0,
        r#type: Option<String>,
        material: Option<String>,
        assembly: Option<String>,
        /// Holes cut upright through it (roof lights, a chimney).
        holes: Vec<Vec<DVec2>>,
    }
}

spec_enum! {
    /// A roof's shape.
    pub enum RoofShape {
        Flat = "flat",
        /// One slope, rising from the `edge`.
        Shed = "shed",
        /// Two slopes meeting at a ridge, with upright gable ends.
        Gable = "gable",
        /// A slope from every edge.
        Hip = "hip",
    }
}

spec_struct! {
    /// A column: a rectangular (`width` × `depth`) or round (`diameter`)
    /// profile standing on a level.
    pub struct Column {
        id: String,
        name: Option<String>,
        mark: Option<String>,
        level: String,
        /// The profile's centre in plan.
        at: DVec2,
        r#type: Option<String>,
        width: Option<f64>,
        depth: Option<f64>,
        diameter: Option<f64>,
        /// Turn of the profile, degrees anticlockwise.
        rotation: f64 = 0.0,
        base_offset: f64 = 0.0,
        /// A fixed height. Left out: up to `top_level`, else to the next
        /// level up.
        height: Option<f64>,
        top_level: Option<String>,
        top_offset: f64 = 0.0,
        material: Option<String>,
    }
}

spec_struct! {
    /// A beam, its top `top_offset` below the top of its storey.
    pub struct Beam {
        id: String,
        name: Option<String>,
        mark: Option<String>,
        level: String,
        start: DVec2,
        end: DVec2,
        r#type: Option<String>,
        width: Option<f64>,
        /// Height of the section.
        depth: Option<f64>,
        top_offset: f64 = 0.0,
        material: Option<String>,
    }
}

spec_struct! {
    /// A stair: drawn by a line (its walk line or a side), with a landing
    /// at each corner; or, without a line, one flight or two with a
    /// landing between.
    pub struct Stair {
        id: String,
        name: Option<String>,
        level: String,
        /// The middle of the first riser, in plan.
        start: DVec2,
        /// Which way the first flight climbs, degrees anticlockwise from x.
        direction: f64 = 0.0,
        width: f64,
        /// The tallest riser allowed; the risers share the height equally.
        max_riser: f64 = 0.18,
        /// The depth of each tread.
        tread: f64 = 0.28,
        /// A fixed rise. Left out: up to the next level.
        height: Option<f64>,
        base_offset: f64 = 0.0,
        /// Thickness of the sloping slab under the steps.
        waist: f64 = 0.15,
        landing: Option<Landing>,
        material: Option<String>,
        /// The line it was drawn by, in plan, from `start`. Its corners
        /// are landings.
        line: Vec<DVec2>,
        /// How far each straight of `line` bends into an arc: degrees its
        /// direction turns through, to the left when positive.
        bends: Vec<f64>,
        /// Which line of the stair `line` is.
        drawn: StairLine = StairLine::Walk,
        /// Flights given their own number of treads; the others take the
        /// rest.
        flights: Vec<StairFlight>,
        /// Landings made deeper than the line leaves them.
        landings: Vec<StairLanding>,
        /// What its turns become.
        turns: StairTurnKind = StairTurnKind::Landing,
        /// How many winders at each turn (0: one for each 30° it turns).
        winders: u32 = 0,
        /// Turns given their own kind.
        turn_kinds: Vec<StairTurn>,
        /// How it is built: what carries its treads.
        build: StairBuild = StairBuild::Concrete,
        /// What its treads, risers and landings are finished in.
        finish: Option<StairFinish>,
        /// The rule set it is checked against: `comfort`, `ontario`, `us`,
        /// `uk`, or the name of one of the project's own.
        rules: String = String::from("ontario"),
        /// A ramp, not steps: its steepest slope, rise over run (1:12 is
        /// 0.0833). A ramp's `tread` and `max_riser` follow from it.
        ramp: Option<f64>,
        /// Stacked: the id of the stair at the foot of the stack this one
        /// repeats on a typical floor. It is kept the same as that stair.
        typical: Option<String>,
        /// A split stair: its line is one arm; the stair is that arm and
        /// its mirror image, sharing one flight twice as wide up to the
        /// first landing.
        split: bool = false,
        /// Where its UP label stands, moved from before its first flight:
        /// the offset in plan. Left out: before the flight.
        up_label: Option<DVec2>,
    }
}

spec_struct! {
    /// The finishes over a stair's structure. Its risers are measured
    /// finish to finish; the structure sits the tread finish lower.
    pub struct StairFinish {
        /// The finish on each tread, landing and winder: its thickness.
        tread: f64 = 0.0,
        /// A material id.
        tread_material: Option<String>,
        /// The finish on each riser's face (none where risers are open).
        riser: f64 = 0.0,
        /// A material id.
        riser_material: Option<String>,
        /// How far each tread's finish runs out past the riser below.
        nosing: f64 = 0.0,
    }
}

spec_enum! {
    /// How a stair is built.
    pub enum StairBuild {
        Concrete = "concrete",
        Stepped = "stepped",
        FoldedPlate = "folded_plate",
        Stringers = "stringers",
        CutStringers = "cut_stringers",
        Spine = "spine",
        Cantilever = "cantilever",
        Post = "post",
    }
}

spec_enum! {
    /// What a turn of a stair becomes.
    pub enum StairTurnKind {
        Landing = "landing",
        Winders = "winders",
    }
}

spec_struct! {
    /// A turn of a line stair given its own kind.
    pub struct StairTurn {
        /// The turn of the line, from 0.
        turn: u32,
        kind: StairTurnKind,
        /// How many winders (0: one for each 30° it turns).
        winders: u32 = 0,
    }
}

spec_struct! {
    /// A flight of a line stair given its own number of treads.
    pub struct StairFlight {
        /// The straight of the line it runs along, from 0.
        straight: u32,
        treads: u32,
    }
}

spec_struct! {
    /// A landing of a line stair made deeper: the flight before it stops
    /// this much sooner.
    pub struct StairLanding {
        /// The turn of the line it sits at, from 0.
        turn: u32,
        deeper: f64,
    }
}

spec_enum! {
    /// Which line of a stair it was drawn by.
    pub enum StairLine {
        Walk = "walk",
        Left = "left",
        Right = "right",
    }
}

spec_struct! {
    /// A landing that turns a stair.
    pub struct Landing {
        /// The number of risers before it.
        after: u32,
        turn: Side,
    }
}

spec_enum! {
    /// Left or right.
    pub enum Side {
        Left = "left",
        Right = "right",
    }
}

spec_struct! {
    /// A railing along a path, or along one side of a stair.
    pub struct Railing {
        id: String,
        name: Option<String>,
        level: String,
        /// Points of its path, heights above the level. Not used when it
        /// follows a stair.
        path: Vec<DVec3>,
        height: f64 = 1.0,
        post_spacing: f64 = 1.0,
        /// A stair's id: the railing follows its `side`.
        stair: Option<String>,
        side: Option<Side>,
        material: Option<String>,
        /// On a stair: how far the handrail runs on past its ends.
        extend: Option<RailExtend>,
        /// A handrail on a wall: brackets, ends back to the wall.
        on_wall: bool = false,
        /// What fills a guard below its top rail.
        infill: RailInfill = RailInfill::Rail,
        /// A guard taller than its handrail: its top rail's height; the
        /// handrail stays at `height` on its posts.
        guard_height: Option<f64>,
        /// Heavier posts at its ends and corners; a steep rail stops at one
        /// and starts again higher.
        newels: bool = false,
    }
}

spec_enum! {
    /// What fills a guard below its top rail.
    pub enum RailInfill {
        Rail = "rail",
        Balusters = "balusters",
        Glass = "glass",
    }
}

spec_struct! {
    /// How a stair's handrail runs on past its bottom and top nosings.
    pub struct RailExtend {
        bottom: f64 = 0.3,
        top: f64 = 0.3,
        /// The rail keeps its slope for one going before running level.
        slope_bottom: bool = true,
    }
}

spec_struct! {
    /// An extrusion of a plan outline, or a sweep of a profile along a
    /// path. A void cuts what it meets (a firebox in a chimney, a hole
    /// through a slab) instead of being solid.
    pub struct Part {
        id: String,
        name: Option<String>,
        level: String,
        /// An extrusion's outline in plan.
        outline: Vec<DVec2>,
        /// An extrusion's holes.
        holes: Vec<Vec<DVec2>>,
        /// An extrusion's bottom above the level.
        base: f64 = 0.0,
        /// An extrusion's height.
        height: Option<f64>,
        /// An extrusion drawn on a face: its outline lies on this plane, and
        /// `base` and `height` run along the plane's normal (`x` × `y`).
        plane: Option<Plane>,
        /// A sweep's section: `x` across the path (positive to its left),
        /// `y` up from it.
        profile: Vec<DVec2>,
        /// A sweep's path through points, `z` above the level.
        path: Vec<DVec3>,
        /// The path runs from its last point back to its first.
        closed: bool = false,
        /// A path drawn on a face: the face's outward normal, which the
        /// profile's `y` runs along instead of up.
        face_normal: Option<DVec3>,
        /// A sweep along a wall's id, at its top or base: it follows the
        /// wall (a coping, a plinth).
        wall: Option<String>,
        edge: WallEdge = WallEdge::Top,
        /// Lifts a wall path off its edge.
        offset: f64 = 0.0,
        /// Runs a wall path on past the wall's start and end.
        extend: Option<DVec2>,
        void: bool = false,
        material: Option<String>,
        /// The IFC class it exports as.
        ifc_class: Option<String>,
    }
}

spec_struct! {
    /// A plane anywhere in the model: its origin (height above the level)
    /// and two square unit axes along it.
    pub struct Plane {
        origin: DVec3,
        x: DVec3,
        y: DVec3,
    }
}

spec_enum! {
    /// A wall's top or base.
    pub enum WallEdge {
        Top = "top",
        Base = "base",
    }
}

spec_struct! {
    /// A site element: terrain, a hard landscape area, an earthwork (a
    /// pad, a pit or a trench) or a plant.
    pub struct SiteElement {
        id: String,
        name: Option<String>,
        kind: SiteKind,
        level: String,
        /// Terrain and area: the outline.
        outline: Vec<DVec2>,
        /// An area's holes.
        holes: Vec<Vec<DVec2>>,
        /// Terrain and area: the top above the level.
        top: f64 = 0.0,
        /// Terrain and area: how deep.
        thickness: Option<f64>,
        /// Terrain: spot heights `[x, y, z]`, `z` above the level.
        points: Vec<DVec3>,
        /// Terrain: a contour every so much height; 0 draws none.
        contour: f64 = 0.0,
        /// Terrain: the contours and break lines it passes through.
        lines: Vec<GroundLine>,
        /// Terrain: a survey, its edge takes its height from the data
        /// nearest it (else the edge stays at `top`).
        survey: bool = false,
        /// Terrain: drawn as flat triangles between its points, not
        /// smoothed through them.
        faceted: bool = false,
        /// Terrain: every so many contours one is heavier; 0 is every fifth.
        major: u32 = 0,
        /// Terrain: heavier contours carry their height.
        labels: bool = false,
        /// Terrain: spot heights carry their height.
        levels: bool = false,
        /// Terrain: what was taken off a survey's coordinates when it was
        /// brought in, metres: a survey point is its place here plus this.
        shift: Option<DVec3>,
        /// Terrain: the pads worked into the ground, in order.
        pads: Vec<GroundPad>,
        /// Terrain: the ground is dug out under the slabs standing on it.
        under_buildings: bool = false,
        /// Terrain: the soil under it, layer by layer from the top.
        strata: Vec<SoilLayer>,
        /// Terrain: shaded in plans by slope, aspect or height.
        analysis: GroundShading = GroundShading::None,
        /// Terrain: arrows in plans for which way water runs.
        flow: bool = false,
        /// An area's surface pattern.
        pattern: Option<SitePattern>,
        /// A plant's position.
        at: Option<DVec2>,
        /// A plant's base above the level.
        base: f64 = 0.0,
        plant: PlantForm = PlantForm::Tree,
        /// A plant's crown across.
        canopy: Option<f64>,
        /// A plant's height.
        height: Option<f64>,
        /// A pad, pit or trench: its bank where it digs in, run over rise.
        cut: Option<f64>,
        /// A pad: its bank where it is made up, run over rise.
        fill: Option<f64>,
        /// A pit or a trench (kind `pad`): how deep below the ground; 0
        /// is a pad at `top`.
        depth: f64 = 0.0,
        /// A trench (kind `pad`): its width; its line is `path`.
        width: f64 = 0.0,
        /// A pad that only digs: lower ground under it stays.
        only_cut: bool = false,
        /// A pad or a pit (kind `pad`): the edges cut upright whatever
        /// its banks, edge 0 from the first corner of `outline` to the
        /// second. A retaining wall standing on one is a wall element of
        /// its own, and is not tied to it here.
        retained: Vec<u32>,
        /// A trench (kind `pad`): its bends rounded to this radius.
        radius: f64 = 0.0,
        /// A property: what each edge of its outline is, edge `i` from
        /// corner `i` to the next (the first front, on four sides the
        /// third rear, the rest sides when left out).
        edges: Vec<LotEdge>,
        /// A property: the planning rules it is built to.
        rules: Option<LotRules>,
        /// A road: its centreline, `[x, y, z]` points, `z` its crown's
        /// level above the level there.
        path: Vec<DVec3>,
        /// A road: its section and how it meets the ground.
        road: Option<SiteRoad>,
        /// A drain: its run, pipe and fall.
        drain: Option<SiteDrain>,
        /// A tank: what it is and its size; it stands at `at`.
        tank: Option<SiteTank>,
        /// A path: its line, width and surface.
        walk: Option<SitePath>,
        /// A borehole, trial pit or probe: its log; it stands at `at`.
        investigation: Option<SiteProbe>,
        material: Option<String>,
    }
}

spec_struct! {
    /// A borehole, trial pit or probe of a soils report, as the report
    /// logs it: depths below its own ground level, `ground` that level
    /// above the level's datum as the report gives it (left out, the
    /// existing ground's height there).
    pub struct SiteProbe {
        kind: ProbeType = ProbeType::Borehole,
        ground: Option<f64>,
        /// How deep it went.
        depth: f64,
        /// Its layers from the top down, each to the depth of its foot.
        layers: Vec<ProbeLayer>,
        water: Vec<ProbeWater>,
        tests: Vec<ProbeTest>,
        method: Option<String>,
        date: Option<String>,
        report: Option<String>,
        notes: Option<String>,
    }
}

spec_struct! {
    /// A layer of a log: from the foot of the one above down to `to`.
    pub struct ProbeLayer {
        to: f64,
        soil: SoilType = SoilType::Clay,
        /// The report's name for it across the site: "GT3".
        unit: Option<String>,
        /// The report's description, as written.
        description: Option<String>,
    }
}

spec_struct! {
    /// Water struck at `depth`, rising to `rest` if the report says.
    pub struct ProbeWater {
        depth: f64,
        rest: Option<f64>,
        note: Option<String>,
    }
}

spec_struct! {
    /// A test at `depth` and its result; `text` as the report writes it
    /// when the number alone doesn't say it all ("50/220 mm").
    pub struct ProbeTest {
        depth: f64,
        kind: TestType = TestType::Spt,
        value: f64,
        text: Option<String>,
    }
}

spec_enum! {
    /// How a spot was investigated.
    pub enum ProbeType {
        Borehole = "borehole",
        TrialPit = "trial_pit",
        Probe = "probe",
    }
}

spec_enum! {
    /// The soil family a layer is drawn with.
    pub enum SoilType {
        Topsoil = "topsoil",
        MadeGround = "made_ground",
        Clay = "clay",
        Silt = "silt",
        Sand = "sand",
        Gravel = "gravel",
        Peat = "peat",
        Rock = "rock",
    }
}

spec_enum! {
    /// What a test was.
    pub enum TestType {
        /// Standard penetration test, N.
        Spt = "spt",
        /// Hand shear vane, kPa.
        Vane = "vane",
        /// Pocket penetrometer, kPa.
        Penetrometer = "penetrometer",
        /// Dynamic probe, blows for 100 mm.
        Dynamic = "dynamic",
    }
}

spec_struct! {
    /// A road's section, metres: the carriageway kerb to kerb, its fall
    /// across (per cent) from its crown or one way to its right, kerbs,
    /// footpaths each side, its make-up's depth, its banks into the
    /// ground (run for each metre of height) and its parking bays.
    pub struct SiteRoad {
        width: f64 = 5.5,
        fall: f64 = 2.5,
        crowned: bool = true,
        kerb: f64 = 0.1,
        footpath: f64 = 0.0,
        thickness: f64 = 0.3,
        cut: f64 = 1.5,
        fill: f64 = 2.0,
        /// Worked into the terrain under it.
        works: bool = true,
        parking: Option<RoadParking>,
        /// Gullies along its low edges this far apart, with a channel by
        /// the kerb; 0 is none.
        gullies: f64 = 0.0,
        /// Its bends rounded to this radius on the centreline; 0 leaves
        /// them sharp.
        radius: f64 = 0.0,
    }
}

spec_struct! {
    /// A path on the ground, metres: along `path` in plan, `width` wide
    /// between its edging, following the ground, its surface and sub-base
    /// `thickness` deep, with `edging` this wide each side (0 for none).
    pub struct SitePath {
        path: Vec<DVec2>,
        width: f64 = 1.2,
        surface: PathSurface = PathSurface::Gravel,
        edging: f64 = 0.05,
        thickness: f64 = 0.15,
        /// Its bends rounded to this radius; 0 leaves them sharp.
        radius: f64 = 0.0,
    }
}

spec_enum! {
    /// What a path's surface is.
    pub enum PathSurface {
        Gravel = "gravel",
        Paving = "paving",
        Asphalt = "asphalt",
        Bark = "bark",
    }
}

spec_struct! {
    /// A drain run, metres: `path` in plan from its head down to its
    /// outfall, a pipe `diameter` across laid at `fall` per cent (1.25 is
    /// 1 in 80), its invert `depth` under the ground at its head, with a
    /// chamber `chamber` square inside at each point, in a trench
    /// `trench` wide on a bed `bed` deep.
    pub struct SiteDrain {
        path: Vec<DVec2>,
        service: DrainService = DrainService::Foul,
        diameter: f64 = 0.1,
        fall: f64 = 1.25,
        depth: f64 = 0.6,
        chambers: bool = true,
        chamber: f64 = 0.6,
        trench: f64 = 0.6,
        bed: f64 = 0.1,
    }
}

spec_enum! {
    /// What a drain carries.
    pub enum DrainService {
        Foul = "foul",
        Surface = "surface",
        Combined = "combined",
        /// A drainage field: perforated pipe in a gravel trench.
        Field = "field",
    }
}

spec_struct! {
    /// A buried tank, metres: `length` by `width` inside, `depth` deep,
    /// turned `angle` degrees anticlockwise, its top `cover` under the
    /// ground, its walls `wall` thick, dug in with `working` room round it.
    pub struct SiteTank {
        service: TankService = TankService::Septic,
        length: f64,
        width: f64,
        depth: f64,
        angle: f64 = 0.0,
        cover: f64 = 0.3,
        wall: f64 = 0.15,
        working: f64 = 0.5,
    }
}

spec_enum! {
    /// What a tank is for.
    pub enum TankService {
        Septic = "septic",
        Treatment = "treatment",
        Rainwater = "rainwater",
        Attenuation = "attenuation",
        Soakaway = "soakaway",
    }
}

impl Default for SiteRoad {
    fn default() -> Self {
        Self {
            width: 5.5,
            fall: 2.5,
            crowned: true,
            kerb: 0.1,
            footpath: 0.0,
            thickness: 0.3,
            cut: 1.5,
            fill: 2.0,
            works: true,
            parking: None,
            gullies: 0.0,
            radius: 0.0,
        }
    }
}

spec_struct! {
    /// Parking bays along a road's kerb: `width` along it (square to
    /// the bay), `length` out, at `angle` degrees to it (90 square on, 0
    /// parallel).
    pub struct RoadParking {
        side: ParkingSide = ParkingSide::Left,
        width: f64 = 2.5,
        length: f64 = 5.0,
        angle: f64 = 90.0,
    }
}

spec_enum! {
    /// Which side of a road, looking along it.
    pub enum ParkingSide {
        Left = "left",
        Right = "right",
        Both = "both",
    }
}

spec_enum! {
    /// What an edge of a property is: it sets the edge's setback.
    pub enum LotEdge {
        Front = "front",
        Side = "side",
        Rear = "rear",
        /// No setback.
        Open = "open",
    }
}

spec_struct! {
    /// A property's planning rules, metres; a limit of 0 is none.
    pub struct LotRules {
        front: f64 = 6.0,
        side: f64 = 3.0,
        rear: f64 = 6.0,
        /// The highest the building may reach above the project's zero.
        height: f64 = 0.0,
        /// How much of the lot may be covered, 0 to 1.
        coverage: f64 = 0.0,
        /// Floor area allowed per unit of lot area.
        floor_ratio: f64 = 0.0,
        /// Bearings and lengths written along the edges in plans.
        bearings: bool = true,
        /// Setback lines and the buildable area drawn in plans.
        setbacks: bool = true,
        /// Angle planes: degrees from the horizontal, rising inward from
        /// `angle_from` above each edge with a setback; 0 is none.
        angle: f64 = 0.0,
        angle_from: f64 = 2.5,
        /// The envelope shown in 3D.
        envelope: bool = true,
        /// Strips along edges kept clear of building.
        easements: Vec<LotEasement>,
        /// Edges with a setback or a note of their own.
        own: Vec<LotEdgeRule>,
    }
}

spec_struct! {
    /// What edge `edge` of a lot (0 is the first) has of its own: a
    /// `setback` other than its kind's, and a `note` written on its
    /// setback line below its kind's setback, `note_offset` from there
    /// where it was dragged.
    pub struct LotEdgeRule {
        edge: u32,
        setback: Option<f64>,
        note: Option<String>,
        note_offset: Option<DVec2>,
    }
}

spec_struct! {
    /// A strip `width` wide inside the lot along edge `edge` (0 is the
    /// first), kept clear of building.
    pub struct LotEasement {
        name: String,
        edge: u32,
        width: f64,
    }
}

spec_enum! {
    /// How a terrain is shaded in plans.
    pub enum GroundShading {
        None = "none",
        Slope = "slope",
        Aspect = "aspect",
        Height = "height",
    }
}

spec_enum! {
    /// What a site element is.
    pub enum SiteKind {
        Terrain = "terrain",
        Area = "area",
        /// A building pad: `outline`, `top`, `thickness` (its base
        /// course), `cut` and `fill` slopes.
        Pad = "pad",
        Plant = "plant",
        /// A lot: its boundary, edges and planning rules.
        Property = "property",
        /// A road along a centreline.
        Road = "road",
        /// A drain run, set out in `drain`.
        Drain = "drain",
        /// A buried tank at `at`, set out in `tank`.
        Tank = "tank",
        /// A path on the ground, set out in `walk`.
        Path = "path",
        /// A borehole, trial pit or probe at `at`, logged in
        /// `investigation`.
        Investigation = "investigation",
        /// A spot level on a terrain: `at`, and `top` its height above
        /// the level. The ground of the terrain it lies on passes
        /// through it.
        Spot = "spot",
        /// A contour or a break line drawn on a terrain: one entry in
        /// `lines`. The ground of the terrain it lies on passes through
        /// it.
        GroundLine = "ground_line",
    }
}

spec_struct! {
    /// A line a terrain passes through: `[x, y, z]` points, `z` above
    /// the level. A contour has one height; a `sharp` line is a break
    /// line (a kerb, a bank top) the ground folds along.
    pub struct GroundLine {
        points: Vec<DVec3>,
        sharp: bool = false,
        closed: bool = false,
    }
}

spec_struct! {
    /// A levelled platform worked into a terrain (or, with a `depth`, a
    /// pit or trench dug into it): its outline, its
    /// `level` above the terrain's level, and its banks, each the run for
    /// one unit of height (`cut` where it is dug in, `fill` where it is
    /// made up; 0 is an upright face).
    pub struct GroundPad {
        outline: Vec<DVec2>,
        level: f64,
        cut: f64 = 1.0,
        fill: f64 = 2.0,
        /// An excavation: above 0, the floor is this far below the
        /// ground all over (a pit, a trench) and `level` is not used.
        depth: f64 = 0.0,
    }
}

spec_struct! {
    /// A layer of soil under a terrain, from the ground as surveyed down.
    pub struct SoilLayer {
        name: String,
        thickness: f64,
    }
}

spec_struct! {
    /// An area's surface: units (`size`, `stagger`, `angle`), lines
    /// (`spacing`, `angle`) or stipple (`spacing`).
    pub struct SitePattern {
        kind: PatternKind,
        size: Option<DVec2>,
        stagger: bool = false,
        angle: f64 = 0.0,
        spacing: Option<f64>,
    }
}

spec_enum! {
    /// A surface pattern.
    pub enum PatternKind {
        None = "none",
        /// Flags, blocks, setts.
        Units = "units",
        /// Boards, decking.
        Lines = "lines",
        /// Gravel, sand.
        Stipple = "stipple",
    }
}

spec_enum! {
    /// A plant's form.
    pub enum PlantForm {
        Tree = "tree",
        Conifer = "conifer",
        Palm = "palm",
        Shrub = "shrub",
    }
}

spec_struct! {
    /// Furniture or a fixture from CreaBIM's catalogue.
    pub struct Object {
        id: String,
        name: Option<String>,
        mark: Option<String>,
        level: String,
        /// The catalogue object's key: "starter/sofa-3" (`creabim
        /// catalogue` lists them; "starter/" may be left out).
        catalogue: String,
        /// Where it goes in plan: its origin (the middle of its back
        /// edge), or its middle with `anchor` "centre".
        at: DVec2,
        anchor: Anchor = Anchor::Origin,
        /// Turn, degrees anticlockwise. At 0 its back is towards +y.
        rotation: f64 = 0.0,
        /// Its floor above the level (for one hung from the ceiling, its
        /// top below the ceiling). Left out: the catalogue's.
        offset: Option<f64>,
        /// The light it gives in a render, when not its kind's usual one.
        /// It belongs to the object's type: every object of the same
        /// `catalogue` key shares it, so state it the same on each (or on
        /// one; the last stated is kept).
        light: Option<ObjectLight>,
    }
}

spec_struct! {
    /// The light a fitting gives in a render.
    pub struct ObjectLight {
        /// False: it is drawn, and gives no light.
        on: bool = true,
        /// Its output, lumens: about 800 for a 60 W bulb of old.
        lumens: f64,
        /// Its colour, kelvin: 2700 warm, 4000 neutral, 6500 daylight.
        kelvin: f64 = 2700.0,
        beam: LightBeam = LightBeam::Wide,
        /// The group it is dimmed with in Visuals; left out, its type's name.
        group: Option<String>,
    }
}

spec_enum! {
    /// How a fitting's light spreads.
    pub enum LightBeam {
        /// A spot or a downlight: a tight cone.
        Narrow = "narrow",
        /// A wider cone, as over a worktop.
        Medium = "medium",
        /// A pendant or a ceiling light: across the room.
        Wide = "wide",
        /// A bare bulb or a globe: the same every way.
        AllRound = "all-round",
    }
}

spec_enum! {
    /// Which point of an object `at` gives.
    pub enum Anchor {
        /// The middle of its back edge.
        Origin = "origin",
        /// The middle of its footprint.
        Centre = "centre",
    }
}

spec_struct! {
    /// A room: the space the walls close off round `at`, or an outline
    /// drawn for it.
    pub struct Room {
        id: String,
        name: Option<String>,
        number: String = String::new(),
        level: String,
        /// A point inside it, where its tag sits.
        at: DVec2,
        /// An outline of its own; left out, the walls round `at` bound it.
        outline: Vec<DVec2>,
        /// Its top above its level. Left out: to the ceiling, or the
        /// level's height.
        height: Option<f64>,
        floor_finish: String = String::new(),
        wall_finish: String = String::new(),
        ceiling_finish: String = String::new(),
        occupancy: String = String::new(),
        department: String = String::new(),
    }
}

spec_struct! {
    /// A view: a plan of a level, a section or elevation along a line, a
    /// 3D view, or a schedule. Which fields it uses depends on its
    /// `kind`: plan: `level`, `cut_height`, `ceiling_plan`; section and
    /// elevation: `start`, `end`, `depth`; 3d: `azimuth`, `altitude`;
    /// schedule: `category`, `by_type`.
    pub struct View {
        id: String,
        /// Its title; the id when left out.
        name: Option<String>,
        kind: ViewKind,
        /// The scale's denominator: 100 is 1:100.
        scale: f64 = 100.0,
        /// Left out: shaded for 3D views, hidden line for the rest.
        style: Option<ViewStyle>,
        /// A plan's level.
        level: Option<String>,
        /// A plan's cut above its level.
        cut_height: f64 = 1.2,
        /// A reflected ceiling plan.
        ceiling_plan: bool = false,
        /// Each room washed in a colour of its own.
        room_colours: bool = false,
        /// A section or elevation's line. It looks to the left of start →
        /// end: a line drawn west to east looks north.
        start: Option<DVec2>,
        end: Option<DVec2>,
        /// How far a section or elevation looks.
        depth: Option<f64>,
        /// A 3D view's compass bearing: 0 from the south, anticlockwise.
        azimuth: f64 = 30.0,
        /// A 3D view's angle above the horizon.
        altitude: f64 = 30.0,
        /// What a schedule lists.
        category: Option<ScheduleCategory>,
        /// A long section: the drain it follows.
        drain: Option<String>,
        /// A borehole log: the borehole, trial pit or probe it logs.
        probe: Option<String>,
        /// A walk-line section: the stair it unrolls.
        stair: Option<String>,
        /// A schedule's rows grouped by type.
        by_type: bool = false,
        /// Makes it a callout of another view: an enlarged part of it.
        /// Give it the same kind and line (or level) as its parent.
        callout: Option<Callout>,
        /// A plan, section, elevation or 3D view: how it shows the site.
        site: Option<ViewSite>,
    }
}

spec_struct! {
    /// How a view shows the site: the ground before or after its works
    /// (or after with the ground as it was dashed), and each kind of site
    /// information on or off.
    pub struct ViewSite {
        ground: SiteGround = SiteGround::After,
        contours: bool = true,
        levels: bool = true,
        cut_fill: bool = false,
        banks: bool = true,
        /// The line round what the works change.
        limit: bool = false,
        /// Property lines, setbacks and easements.
        legal: bool = true,
        roads: bool = true,
        /// Drains, chambers and tanks.
        services: bool = true,
        /// Boreholes, trial pits and probes.
        investigation: bool = true,
    }
}

spec_enum! {
    /// Which ground a view shows.
    pub enum SiteGround {
        After = "after",
        Before = "before",
        /// After, with the ground as it was dashed where it changed.
        Both = "both",
    }
}

spec_struct! {
    /// The box of a parent view a callout enlarges, in that view's
    /// drawing coordinates.
    pub struct Callout {
        /// The parent view's id.
        parent: String,
        min: DVec2,
        max: DVec2,
    }
}

spec_enum! {
    /// What a view is.
    pub enum ViewKind {
        Plan = "plan",
        /// Cut along its line.
        Section = "section",
        /// Seen from its line, nothing cut.
        Elevation = "elevation",
        /// A parallel projection of the whole model.
        ThreeD = "3d",
        Schedule = "schedule",
        /// A drain run unfolded along its length, with its levels.
        LongSection = "long_section",
        /// The log of a borehole, trial pit or probe.
        BoreholeLog = "borehole_log",
        /// A stair unrolled along its walk line, with its headroom.
        WalkSection = "walk_section",
    }
}

spec_enum! {
    /// How a view draws.
    pub enum ViewStyle {
        HiddenLine = "hidden-line",
        Shaded = "shaded",
        Wireframe = "wireframe",
    }
}

spec_enum! {
    /// What a schedule lists.
    pub enum ScheduleCategory {
        Doors = "doors",
        Windows = "windows",
        Walls = "walls",
        Slabs = "slabs",
        /// Columns and beams.
        Structure = "structure",
        Furniture = "furniture",
        Fixtures = "fixtures",
        Rooms = "rooms",
        /// The take-off by material.
        Materials = "materials",
        /// Drain chambers and tanks with their levels.
        Drainage = "drainage",
        /// Stairs with their rise, going, headroom and checks.
        Stairs = "stairs",
        /// Car parks' bays by type, against what is required.
        Parking = "parking",
    }
}

spec_struct! {
    /// A drawing sheet, with views placed on it. Its id is its number.
    pub struct Sheet {
        /// Its number: "A101".
        id: String,
        /// Its title; the number when left out.
        name: Option<String>,
        /// An ISO paper size, landscape. Give this or `width` and `height`.
        size: Option<PaperSize>,
        /// In millimetres.
        width: Option<f64>,
        height: Option<f64>,
        /// Title block fields by name.
        fields: BTreeMap<String, String>,
        viewports: Vec<Viewport>,
    }
}

spec_enum! {
    /// ISO paper sizes, landscape.
    pub enum PaperSize {
        A0 = "A0",
        A1 = "A1",
        A2 = "A2",
        A3 = "A3",
        A4 = "A4",
    }
}

spec_struct! {
    /// A tag in a view, showing an element's mark (a door's, a window's)
    /// or a room's name, number and area. It moves with its element.
    pub struct Tag {
        /// A view's id.
        view: String,
        /// The id of the element it tags.
        element: String,
        /// Where it sits, from the element's middle, in drawing units.
        offset: DVec2 = DVec2::ZERO,
        /// A bend in its leader, from the element's middle.
        elbow: Option<DVec2>,
        /// Where its leader ends, from the element's middle; left out, at
        /// the middle.
        end: Option<DVec2>,
    }
}

spec_struct! {
    /// Text in a view (drawing coordinates, in the file's units) or on a
    /// sheet (paper millimetres).
    pub struct Note {
        /// A view's id, or a sheet's.
        view: String,
        /// Where its first line starts: its left end, mid-height.
        at: DVec2,
        /// Line breaks (`\n`) start new lines.
        text: String,
        /// A leader from the text to this point, in the same units as `at`.
        leader: Option<DVec2>,
        /// A bend in the leader.
        elbow: Option<DVec2>,
        colour: Option<Colour>,
        /// Drawn as a CreaBIM symbol instead of as text, `text` going in
        /// the symbol's slot when it has one.
        symbol: Option<NoteSymbol>,
    }
}

spec_struct! {
    /// A symbol placed with the Symbol tool, its middle on the note's `at`.
    pub struct NoteSymbol {
        /// `points`, `slopes`, `centerlines`, `materials` or `mep`.
        kind: String,
        /// A style of that kind, as `slopes.ramp_rise_arrow`; left out,
        /// the kind's pick in the project's style pack.
        style: Option<String>,
        /// Its turn, degrees anticlockwise.
        angle: f64 = 0.0,
    }
}

spec_struct! {
    /// A view placed on a sheet.
    pub struct Viewport {
        /// A view's id.
        view: String,
        /// Where on the sheet, in millimetres from its lower left corner.
        at: DVec2,
        /// The drawing point (in the file's units, as the drawing's x
        /// and y) put at `at`. Left out: the middle of the crop, or of the
        /// whole drawing.
        target: Option<DVec2>,
        /// Only this part of the drawing is shown (drawing coordinates).
        crop: Option<Rect>,
        /// A stair's id: the crop follows that stair, its footprint with a
        /// margin round it, as it moves or grows; `crop` is used once it
        /// is gone.
        fit: Option<String>,
    }
}
