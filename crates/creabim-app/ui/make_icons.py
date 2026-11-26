"""Make icons.js from the lucide-static icons folder: make_icons.py ICONS_DIR > icons.js"""
import re, sys, json
src=sys.argv[1].rstrip('/')+'/'
names="""mouse-pointer-2 brick-wall door-open app-window layers layers-2 box scissors-line-dashed building building-2 ruler tag pen-tool slash undo-2 redo-2 move rotate-cw scaling copy-plus copy scissors clipboard-paste trash-2 eye-off focus scan-eye eye folder chevron-right chevron-down search file magnet grid-3x3 square-dashed crosshair check arrow-left layers-3 columns-2 scan sticky-note frame panel-left hash axis-3d circle-dot move-3d spline file-plus folder-open save download upload link sheet table-2 refresh-cw cuboid house map-pin layout-dashboard square-dashed-mouse-pointer git-commit-horizontal ellipsis sun moon panel-right panel-bottom maximize-2 minus diamond palette map file-text""".split()
# for the 3D view's buttons and the command palette
names+="""zoom-in zoom-out rotate-3d""".split()
# for the single view layout
names+="""square""".split()
# for the annotation tools
names+="""drafting-compass radius diameter message-square-text arrow-up-right""".split()
# for the building element tools
names+="""rectangle-vertical rectangle-horizontal tent fence corner-down-right""".split()
# a property: a parcel and its flag (Site polish 4, owner's pick E)
names+="""land-plot""".split()
# flipping a section or elevation round
names+="""flip-horizontal-2""".split()
# the Rotate bar's quarter turn anticlockwise (MU142)
names+="""rotate-ccw""".split()
# the Help menu and the Help panel
names+="""circle-help book-open keyboard rocket x arrow-right info""".split()
# folding the project browser's groups
names+="""chevrons-down-up chevrons-up-down""".split()
# pinning project time at the browser's foot (MU101)
names+="""pin""".split()
# snap markers, drawn like the markers in the views (not from Lucide)
CUSTOM={
 # Mirror: two triangles either side of a dashed line, as drawings show it (MU142)
 "mirror": '<path d="M9 6 3 18h6z" /> <path d="M15 6l6 12h-6z" /> <line x1="12" x2="12" y1="3" y2="5.5" /> <line x1="12" x2="12" y1="8.5" y2="10.5" /> <line x1="12" x2="12" y1="13.5" y2="15.5" /> <line x1="12" x2="12" y1="18.5" y2="21" />',
 "snap-endpoint": '<rect x="6" y="6" width="12" height="12" />',
 "snap-midpoint": '<path d="M12 5.5 19.5 18.5h-15z" />',
 "snap-intersection": '<path d="m6 6 12 12M18 6 6 18" />',
 "snap-perpendicular": '<path d="M4 19h16M10 19V5M10 14h5v5" />',
 "snap-nearest": '<path d="M6 5h12L6 19h12z" />',
 "snap-centre": '<circle cx="12" cy="12" r="7" /><circle cx="12" cy="12" r="1" />',
 "snap-quadrant": '<path d="M12 4.5 19.5 12 12 19.5 4.5 12z" />',
 "snap-axis": '<path d="M3 12h18M12 3v18" stroke-dasharray="2.5 2.5" />',
 # the Level tool and the project browser's group buttons
 # the site (2026-11-18): the ground tools that stood in with general icons
 "contour": '<path d="M3 5c5 0 8 4 8 9s3 7 10 7" /> <path d="M3 10c3 0 5 2.5 5 5.5S10.5 21 15 21" /> <path d="M3 15c1.5 0 2.5 1 2.5 2.5S7 21 9 21" />',
 "pit": '<path d="M2 8h3l3.5 9h7L19 8h3" />',
 "trench": '<path d="M3 7h5v9h8V7h5" />',
 "road": '<path d="M8 3 5 21M16 3l3 18M12 5v3M12 11v3M12 17v3" />',
 "site-path": '<path d="M5 20c1-6 13-5 14-16" /> <path d="M9 20c1-4 11-3 12-14" />',
 "drain": '<circle cx="5" cy="6" r="2.5" /> <circle cx="19" cy="18" r="2.5" /> <path d="M7 7.5l10 9" /> <path d="M11 14.5h3.5V11" />',
 "tank": '<path d="M2 6h8M14 6h8" /> <path d="M10 6v3H7a3 3 0 0 0-3 3v5a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-5a3 3 0 0 0-3-3h-3V6" />',
 "spot-level": '<path d="M8 16l-3.5-5h7z" fill="currentColor" stroke="none" /> <path d="M2 16h20" /> <path d="M8 11L13 6h9" /> <path d="M15 3.5h2" />',
 "level": '<path d="M2 14h12" stroke-dasharray="3 2" /> <circle cx="18" cy="14" r="3.5" /> <path d="M18 10.5V14h3.5M18 14v3.5h-3.5" /> <path d="M3 7h9" />',
 "plus": '<path d="M5 12h14M12 5v14" />',
 # the project browser's sort button, and alignment tracking in the snap list
 "sort-by-level": '<path d="M3 6h8M3 12h6M3 18h4" /> <path d="M17 4v16M13 16l4 4 4-4" />',
 "snap-track": '<path d="M3 8h18M16 3v18" stroke-dasharray="2.5 2.5" /> <circle cx="16" cy="8" r="2" />',
 # the Align tool, temporary guides, annotation styles and sheet templates
 "align-tool": '<path d="M4 3v18" stroke-dasharray="2.5 2" /> <rect x="8" y="5" width="12" height="5" /> <rect x="8" y="14" width="7" height="5" />',
 "snap-guides": '<path d="M2 16h20M7 21 19 3" stroke-dasharray="2.5 2" /> <circle cx="13.7" cy="12" r="2.5" />',
 "anno-styles": '<path d="M3 17h12" /> <path d="m3 14 3 6M12 14l3 6" /> <circle cx="18" cy="17" r="2" /> <path d="M5 4h14M12 4v7" />',
 "sheet-template": '<rect x="3" y="4" width="18" height="16" /> <path d="M11 14h10M11 14v6M16 14v6" />',
 # the Stair tool: a flight seen from the side, with its handrail
 "stair": '<path d="M3 21h4v-4h4v-4h4V9h4V5h2v16z" /> <path d="M3 14 12 5" />',
 # X-ray and hidden elements: a see-through (dashed) wall with the solid
 # beam inside it showing, unlike the Wall opening tool's solid frame
 "xray": '<rect x="7" y="2.5" width="10" height="19" rx="1" stroke-dasharray="2.5 2" /> <path d="M2 10h20v4H2z" fill="currentColor" fill-opacity=".35" />',
 # how the 3D view draws: shaded with edges, hidden line, wireframe
 "render-edges": '<path d="M3 9h12v12H3z" fill="currentColor" fill-opacity=".3" /> <path d="m3 9 6-6h12v12l-6 6M15 9l6-6" fill="currentColor" fill-opacity=".15" />',
 "render-hidden-line": '<rect x="3" y="9" width="12" height="12" /> <path d="m3 9 6-6h12v12l-6 6M15 9l6-6" />',
 "render-wireframe": '<rect x="3" y="9" width="12" height="12" /> <path d="m3 9 6-6h12v12l-6 6M15 9l6-6" /> <path d="M9 3v12h12M9 15l-6 6" stroke-dasharray="2 1.5" />',
 # editing an element's outline: a shape with corner grips and an added one
 "edit-outline": '<path d="M5 19 4 6l12-2 4 13z" /> <rect x="2.5" y="4.5" width="3" height="3" /> <rect x="14.5" y="2.5" width="3" height="3" /> <rect x="18.5" y="15.5" width="3" height="3" /> <path d="m12 16.5 2 2-2 2-2-2z" />',
 # crop boxes (section boxes) of 3D views: a dashed box with grips
 "crop-box": '<path d="M4 7.5 12 3l8 4.5v9L12 21l-8-4.5z" stroke-dasharray="2.5 2" /> <path d="M4 7.5 12 12l8-4.5M12 12v9" /> <rect x="10.5" y="1.5" width="3" height="3" /> <rect x="10.5" y="19.5" width="3" height="3" />',
 # element types, curtain walls, wall openings and shafts
 "curtain-wall": '<rect x="3" y="3" width="18" height="18" /> <path d="M9 3v18M15 3v18M3 12h18" />',
 "wall-opening": '<rect x="3" y="3" width="18" height="18" rx="2" /> <rect x="8" y="8" width="8" height="8" stroke-dasharray="2 2" />',
 "shaft": '<path d="M3 8h18M3 16h18" /> <rect x="9" y="3" width="6" height="18" stroke-dasharray="2 2" />',
 "element-types": '<rect x="3" y="3" width="7" height="7" /> <rect x="14" y="3" width="7" height="7" /> <rect x="3" y="14" width="7" height="7" /> <path d="M14 17.5h7M17.5 14v7" />',
 # furniture and fixtures: the Object tool and the Catalogue
 "sofa": '<path d="M5 11V8a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v3" /> <path d="M3 13a2 2 0 0 1 4 0v2h10v-2a2 2 0 0 1 4 0v4H3z" /> <path d="M5 17v2M19 17v2" />',
 # building services fixtures: a lighting point, for the fixture schedule
 "fixtures": '<circle cx="12" cy="10" r="6.5" /> <path d="m7.4 5.4 9.2 9.2M16.6 5.4l-9.2 9.2M9 20h6" />',
 # footings, and copying or moving elements to other levels
 "footing": '<path d="M9 3v11M15 3v11" /> <rect x="3" y="14" width="18" height="6" /> <path d="M1 10h4M19 10h4" stroke-dasharray="2 1.5" />',
 "copy-levels": '<path d="M2 7h11M2 19h11" stroke-dasharray="3 2" /> <rect x="15" y="2" width="6" height="5" /> <rect x="15" y="14" width="6" height="5" stroke-dasharray="2 1.5" /> <path d="M18 8.5v4M16.5 11l1.5 1.5 1.5-1.5" />',
 "move-level": '<path d="M2 7h11M2 19h11" stroke-dasharray="3 2" /> <rect x="15" y="14" width="6" height="5" /> <path d="M18 2v10M15.5 9.5 18 12l2.5-2.5" />',
 # extrusions, sweeps and voids: an outline pulled up, a profile along a
 # path, a dashed solid cutting a block
 "extrusion": '<path d="M4 17 9 20l11-4-5-3z" /> <path d="M4 17V7l5 3v10M9 10l11-4v10M4 7l11-4 5 3" />',
 "sweep": '<path d="M3 15h18v4H3z" /> <path d="M3 15c3-8 15-8 18 0" stroke-dasharray="2.5 2" /> <path d="M6 11V7h4v4" />',
 "void": '<rect x="3" y="3" width="18" height="18" /> <rect x="7" y="9" width="10" height="12" stroke-dasharray="2 2" /> <path d="M7 9a5 5 0 0 1 10 0" stroke-dasharray="2 2" />',
 # screen walls: a wall of pierced blocks
 "screen-wall": '<rect x="2" y="4" width="20" height="16" /> <rect x="5" y="7" width="4" height="4" /> <rect x="10" y="7" width="4" height="4" /> <rect x="15" y="7" width="4" height="4" /> <rect x="5" y="13" width="4" height="4" /> <rect x="10" y="13" width="4" height="4" /> <rect x="15" y="13" width="4" height="4" />',
 # the site: ground with a contour, paving units, a tree, a car
 "terrain": '<path d="M2 18c4-6 8-2 11-6s6-4 9-2v10H2z" /> <path d="M2 14c3-3 6-1 9-4" stroke-dasharray="2 2" />',
 "area-driveway": '<rect x="14" y="3" width="7" height="5.5" rx="1" /> <path d="M4 21C4 14.5 8 12 15 12h6" /> <path d="M10 21c0-2.5 1.5-4 5-4h6" />',
 "area-gravel": '<circle cx="7" cy="7.5" r="2.2" /> <circle cx="15" cy="5.5" r="1.5" /> <circle cx="17.5" cy="12" r="2.3" /> <circle cx="10" cy="14" r="1.7" /> <circle cx="15" cy="18.5" r="1.5" /> <circle cx="5.5" cy="18.5" r="1.4" /> <path d="M12 9.5h.01M20 18h.01M4 13h.01" />',
 "plant-tree": '<path d="M12 22v-7" /> <path d="M12 19l2.5-1.7M12 17.5l-2-1.2" /> <path d="M8 15a4.5 4.5 0 0 1-1.8-8.2 6 6 0 0 1 11.6 0A4.5 4.5 0 0 1 16 15z" />',
 "plant-conifer": '<path d="M12 2l4 5.800h-2.400l4 5.700H14l4.500 5.500h-13L10 13.500H6.400l4-5.700H8z" /> <path d="M12 19v3" />',
 "plant-palm": '<path d="M10 22c0-4.5 0.6-8.5 2.5-12" /> <path d="M12.500 10C11 7 8 6 4 7.500M12.500 10c-2.2-0.8-5-0.3-7 2.8M12.500 10C14 7 17 6 20.500 7.500M12.500 10c2.2-0.8 5-0.3 6.5 3.2M12.500 10c0-3 0.5-5 2-7" />',
 "plant-shrub": '<path d="M3 20h18" /> <path d="M5.500 20a4 4 0 0 1 0.8-7.6 5 5 0 0 1 9.600-1.300A4.500 4.500 0 0 1 18.500 20" /> <path d="M12 20v-4.500M9.500 20L8 17M14.500 20l1.500-3" />',
 "area-paving": '<rect x="3" y="3" width="18" height="18" rx="1.5" /> <rect x="5.5" y="5.5" width="8" height="3.6" rx="0.6" /> <rect x="10.5" y="10.2" width="8" height="3.6" rx="0.6" /> <rect x="5.5" y="14.9" width="7" height="3.6" rx="0.6" />',
 "area-asphalt": '<rect x="3" y="3" width="18" height="18" rx="2" /> <path d="M7.5 7.5h.01M12 7.5h.01M16.5 7.5h.01M7.5 12h.01M12 12h.01M16.5 12h.01M7.5 16.5h.01M12 16.5h.01M16.5 16.5h.01" />',
 "area-decking": '<rect x="3" y="3" width="18" height="18" rx="1.5" /> <path d="M3 8l9 5 9-5M3 13.5l9 5 9-5M12 3v18" />',
 "parking": '<rect x="3" y="3" width="18" height="18" rx="2" /> <path d="M9 17V7h4a3 3 0 0 1 0 6H9" />',
 "paving": '<rect x="3" y="3" width="18" height="18" /> <path d="M3 9h18M3 15h18M9 3v6M15 3v6M6 9v6M12 9v6M18 9v6M9 15v6M15 15v6" />',
 "tree": '<circle cx="12" cy="10" r="7" /> <path d="M12 17v5M9 22h6" /> <path d="M9 10h6M12 7v6" />',
 "car": '<path d="M3 16v-4l2-4h10l3 4h3v4z" /> <circle cx="7" cy="17" r="2" /> <circle cx="17" cy="17" r="2" /> <path d="M6 12h11" />',
 # rooms, room separators and ceilings
 "room": '<path d="M3 3h18v18H3z" /> <path d="M3 13h6M14 3v6" stroke-dasharray="2 1.5" /> <path d="M10 15.5h7M10 18.5h4" />',
 "room-separator": '<path d="M3 3h18v18H3z" /> <path d="M12 3v18" stroke-dasharray="3 2" /> <circle cx="12" cy="3" r="1.5" /> <circle cx="12" cy="21" r="1.5" />',
 "ceiling": '<path d="M2 4h20" /> <path d="M3 8h18" /> <path d="M7 8v3M17 8v3" /> <path d="M9.5 15a2.5 2.5 0 0 0 5 0z" /> <path d="M12 8v7M4 20h16" stroke-dasharray="2 1.5" />',
 # detailing: callouts, detail components and junction rules
 "callout": '<rect x="3" y="3" width="11" height="11" stroke-dasharray="2.5 2" /> <path d="m14 14 3 3" /> <circle cx="19" cy="19" r="3" />',
 "detail-component": '<path d="M3 20h18" /> <path d="M3 8c2-3 3 3 5 0s3 3 5 0 3 3 5 0 2 2 3 0" /> <path d="M3 14h18" stroke-width="3" />',
 "join": '<rect x="3" y="3" width="8" height="18" /> <path d="M11 9h10v6H11" /> <path d="M7 3v18M11 12h10" stroke-dasharray="2 1.5" />',
}
# Brief 02's final icon set (boards/IconSetFinal.png): Lucide's grammar, a
# 24 grid, 2.0 stroke, round caps and joins. Lucide icons kept as they are:
names+="""align-start-vertical lamp-ceiling tree-deciduous circle-dot-dashed square-arrow-out-up-right layout-panel-left arrow-up-to-line align-left settings log-out""".split()
FINAL={
 # model
 "wall": '<rect x="3" y="3" width="18" height="18" rx="2" /> <path d="M3 9h18M3 15h18M9 3v6M15 3v6M12 9v6M7 15v6M17 15v6" />',
 "door": '<path d="M3 21h18" /> <path d="M6 21V4h11v17" /> <path d="M6 4l6 2v15" /> <path d="M10 12.5v1" />',
 "shaft": '<path d="M9 3v18M15 3v18" stroke-dasharray="2.5 2" /> <path d="M3 8h6M15 8h6M3 16h6M15 16h6M9 12h6" />',
 "column": '<rect x="7" y="2" width="10" height="20" rx="2" />',
 "beam": '<rect x="2" y="7" width="20" height="10" rx="2" />',
 "slab": '<path d="M2 10 12 5l10 5-10 5z" /> <path d="M2 10v3l10 5 10-5v-3" />',
 "stair": '<path d="M4 20h3v-3h3v-3h3v-3h3V8h3V5" />',
 "footing": '<path d="M9 3h6v9H9z" /> <path d="M4 12h16v6H4z" /> <path d="M3 21h18" />',
 "object": '<path d="M5 11V8a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v3" /> <path d="M3 13a2 2 0 0 1 4 0v2h10v-2a2 2 0 0 1 4 0v4H3z" /> <path d="M5 17v2M19 17v2" />',
 "extrusion": '<path d="M3 17l9-4.5 9 4.5-9 4.5z" /> <path d="M12 15V3M8.5 6.5 12 3l3.5 3.5" />',
 "sweep": '<path d="M3 21c5 0 6-10 12-12" /> <rect x="15" y="4" width="5" height="6" rx="1" />',
 "void": '<path d="M12 3 20 7.5v9L12 21l-8-4.5v-9z" /> <path d="M4 7.5 12 12l8-4.5M12 12v9" /> <path d="M8.5 11v4l3.5 2" stroke-dasharray="1.5 1.5" />',
 "room": '<rect x="3" y="3" width="18" height="18" rx="1" /> <path d="M3 12h5M14 3v5" /> <path d="M10 16h6M12 19h2" />',
 "ceiling": '<path d="M3 5h18" /> <path d="M6 5v2M10 5v2M14 5v2M18 5v2" /> <path d="M3 19h18" />',
 "window": '<rect x="4" y="3" width="16" height="16" rx="1" /> <path d="M12 3v16M4 11h16" /> <path d="M2 21h20" />',
 "opening": '<path d="M4 21V3h16v18h-5v-7a3 3 0 0 0-6 0v7z" />',
 "railing": '<path d="M3 5h18M3 19h18" /> <path d="M5 5v14M9 5v14M12 5v14M15 5v14M19 5v14" />',
 "room-separator": '<rect x="3" y="3" width="18" height="18" rx="1" /> <path d="M12 3v18" stroke-dasharray="2.5 2.5" /> <path d="M6 7h3M15 17h3" />',
 "curtain-wall": '<rect x="3" y="3" width="18" height="18" rx="1" /> <path d="M12 3v18M3 12h18" /> <path d="m6 9 3-3M15 18l3-3" />',
 "screen-wall": '<rect x="3" y="3" width="18" height="18" rx="1" /> <path d="m7.5 5 2.5 2.5-2.5 2.5L5 7.5zM16.5 5 19 7.5 16.5 10 14 7.5zM7.5 14l2.5 2.5L7.5 19 5 16.5zM16.5 14l2.5 2.5-2.5 2.5-2.5-2.5z" />',
 "fixtures": '<path d="M12 3v4" /> <path d="M10 3h4" /> <path d="M4 10h16a8 8 0 0 1-16 0z" />',
 # site
 "terrain": '<path d="M2 16c2.5-2 5-2 7.5 0s5 2 7.5 0" /> <path d="M2 20c2.5-2 5-2 7.5 0s5 2 7.5 0s3-1.5 5-1" /> <circle cx="17" cy="6" r="2.5" /> <path d="M17 8.5V13" />',
 "paving": '<path d="M9 3h6M8.5 3 6 21M15.5 3 18 21" /> <path d="M8 7h8M7.5 11h9M7 15h10" /> <path d="M12 7v4M10 11v4M14 11v4M12 15v6" />',
 "planting": '<path d="M12 21v-7" /> <path d="M8 21h8" /> <path d="M8 14a4 4 0 0 1-1.5-7.7A5 5 0 0 1 16 5a4 4 0 0 1 0 9z" />',
 # views
 "section": '<circle cx="5" cy="6" r="2.5" /> <circle cx="5" cy="18" r="2.5" /> <path d="M7 7.5 14 12M7 16.5l4.5-3" /> <path d="M15 12h2M19.5 12h2" />',
 "callout": '<rect x="3" y="3" width="11" height="11" rx="1" /> <path d="m14 14 6 6" /> <path d="M20 15v5h-5" />',
 "elevation": '<path d="M3 21V10l6-6 6 6v11z" /> <path d="M7 21v-5h4v5" /> <path d="M18 12h4M20 10l2 2-2 2" />',
 "crop-box": '<path d="M12 4.5 18.5 8v8L12 19.5 5.5 16V8z" stroke-dasharray="2 2" /> <path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4" />',
 "frame-all": '<path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4" /> <rect x="8" y="8" width="8" height="8" rx="1" />',
 "hidden-elements": '<rect x="4" y="4" width="16" height="16" stroke-dasharray="2 2" /> <path d="m3 3 18 18" />',
 "hidden-colour": '<rect x="3" y="3" width="18" height="18" rx="1" stroke-dasharray="2.5 2" /> <rect x="8" y="8" width="8" height="8" fill="currentColor" fill-opacity=".4" />',
 "xray": '<rect x="8" y="3" width="8" height="18" rx="1" stroke-dasharray="2.5 2" /> <path d="M3 10h18v4H3z" />',
 # annotate
 "dimension": '<path d="M3 7v10M21 7v10M6 12h12" /> <path d="M8 9.5 5.5 12 8 14.5M16 9.5l2.5 2.5-2.5 2.5" />',
 "angle": '<circle cx="12" cy="5" r="2" /> <path d="M11 7 5 20M13 7l6 13" /> <path d="M7.5 15a6 6 0 0 0 9 0" />',
 "arc-length": '<path d="M5 19C5 11 11 5 19 5" /> <circle cx="5" cy="19" r="2" /> <circle cx="19" cy="5" r="2" />',
 "detail-component": '<path d="M2 12h2l2-4 3 8 3-8 3 8 3-8 2 4h2" />',
 # drafting and datums
 "guide": '<path d="m3 21 18-18" stroke-dasharray="2.5 2.5" /> <circle cx="12" cy="12" r="2.5" />',
 "guide-lines": '<path d="M3 10h9M12 2v9" stroke-dasharray="2 2" /> <path d="m13 13 8 3-3.5 1.5L16 21z" />',
 "level": '<path d="M3 9h13M3 15h13" /> <path d="M17 6.5 21 9l-4 2.5zM17 12.5l4 2.5-4 2.5z" />',
 "copy-levels": '<path d="M3 7h9M3 17h9" /> <path d="M7.5 9.5v5M5.5 12.5l2 2 2-2" /> <rect x="15" y="3" width="6" height="6" rx="1" /> <rect x="15" y="14" width="6" height="6" rx="1" stroke-dasharray="2 1.5" />',
 "switch-join": '<path d="M5 3v18M5 7h12M5 13h8" /> <path d="M9 7v6" /> <path d="M15 17a3 3 0 0 0 5-2l1 1" /> <path d="M20 15v2h-2" />',
 # view controls
 "units": '<path d="M21.3 15.3a2.4 2.4 0 0 1 0 3.4l-2.6 2.6a2.4 2.4 0 0 1-3.4 0L2.7 8.7a2.41 2.41 0 0 1 0-3.4l2.6-2.6a2.41 2.41 0 0 1 3.4 0Z" /> <path d="m14.5 12.5 2-2M11.5 9.5l2-2M8.5 6.5l2-2M17.5 15.5l2-2" />',
 "colour-scheme": '<rect x="3" y="4" width="5" height="5" rx="1" /> <rect x="3" y="15" width="5" height="5" rx="1" /> <path d="M11 6.5h10M11 17.5h10M5.5 9v6" />',
 "underlay": '<path d="M12 4 21 9l-9 5-9-5z" /> <path d="m3 14 9 5 9-5" stroke-dasharray="2.5 2" />',
 "frame-selection": '<path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4" /> <circle cx="12" cy="12" r="1.5" />',
 "maximise": '<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />',
 # element actions
 "edit-outline": '<path d="M4 20 3 8l8-5 5 5" /> <path d="m13 19 7-7-3-3-7 7v3z" />',
 "duplicate": '<rect x="8" y="8" width="13" height="13" rx="2" /> <path d="M4 16V5a1 1 0 0 1 1-1h11" /> <path d="M14.5 11.5v6M11.5 14.5h6" />',
 "select-similar": '<rect x="3" y="3" width="7" height="7" rx="1" /> <rect x="14" y="3" width="7" height="7" rx="1" /> <rect x="3" y="14" width="7" height="7" rx="1" /> <path d="m14 14 7 3-3 1-1 3z" />',
 "frame": '<path d="M3 7V3h4M17 3h4v4M21 17v4h-4M7 21H3v-4" /> <circle cx="12" cy="12" r="1.5" />',
 "flip": '<path d="M12 3v18" /> <path d="M8 7 3 12l5 5zM16 7l5 5-5 5z" />',
 "make-type": '<rect x="3" y="3" width="8" height="8" rx="1" /> <rect x="13" y="3" width="8" height="8" rx="1" /> <rect x="3" y="13" width="8" height="8" rx="1" /> <rect x="13" y="13" width="8" height="8" rx="1" />',
 "attach-top": '<path d="M4 3h16" /> <path d="M7 21V7h10v14" /> <path d="M12 16V8M9.5 10.5 12 8l2.5 2.5" />',
 "select-tagged": '<path d="M12.6 2.6A2 2 0 0 0 11.2 2H4a2 2 0 0 0-2 2v7.2a2 2 0 0 0 .6 1.4l7 7" /> <circle cx="7.5" cy="7.5" r="1" /> <path d="m13 13 8 3-3.5 1.5L16 21z" />',
 "straighten-leader": '<path d="M4 20 20 4" /> <path d="M4 14c3-1 4-3 5-6" stroke-dasharray="2 2" /> <path d="M15 4h5v5" />',
 # files
 "freecad": '<path d="M5 21V3h10M5 11h7" /> <circle cx="17.5" cy="16.5" r="2" /> <path d="M17.5 12v1.5M17.5 19.5V21M13 16.5h1.5M20.5 16.5H22M14.3 13.3l1 1M19.7 18.7l1 1M14.3 19.7l1-1M19.7 14.3l1-1" />',
 # roof shapes in the Shape list
 "roof-flat": '<path d="M3 8h18v3H3z" /> <path d="M5 11v9M19 11v9" />',
 "roof-mono": '<path d="M4 20v-9l16-6v15" /> <path d="M3 20h18" />',
 "roof-gable": '<path d="M3 11 12 4l9 7" /> <path d="M5 9.5V20h14V9.5" />',
 "roof-hip": '<path d="M3 13 7 7h10l4 6z" /> <path d="M5 13v7h14v-7" />',
 # the modifiers (MU38)
 "mod-array": '<rect x="2" y="8" width="5" height="8" /><rect x="9.5" y="8" width="5" height="8" /><rect x="17" y="8" width="5" height="8" />',
 "mod-radial": '<circle cx="12" cy="12" r="2" /><rect x="10.5" y="2" width="3" height="4" /><rect x="10.5" y="18" width="3" height="4" /><rect x="2" y="10.5" width="4" height="3" /><rect x="18" y="10.5" width="4" height="3" />',
 "mod-path": '<path d="M3 20C7 10 17 15 21 4" /><circle cx="5.5" cy="15" r="1.5" /><circle cx="12" cy="12.5" r="1.5" /><circle cx="18" cy="9" r="1.5" />',
 "mod-panels": '<rect x="3" y="3" width="8" height="8" /><rect x="13" y="3" width="8" height="8" /><rect x="3" y="13" width="8" height="8" /><rect x="13" y="13" width="8" height="8" />',
 "mod-louvres": '<path d="M4 3v18" /><path d="M4 5l10 3M4 11l10 3M4 17l10 3" />',
 "mod-bevel": '<path d="M4 21V11l7-7h9" />',
 "mod-shell": '<path d="M3 4h18v16H3z" /><path d="M7 4v12h10V4" />',
 "offset": '<rect x="8" y="8" width="8" height="8" /><path d="M3 8V3h5M16 3h5v5M21 16v5h-5M8 21H3v-5" />',
 # Pull (MU43): a face lifted off where it lay, the one tool that grows a
 # line into a surface, a face into a solid and a solid into a bigger one
 "pull": '<path d="M3 17.5 12 13l9 4.5-9 4.5z" /><path d="M3 17.5V12M21 17.5V12" stroke-dasharray="1.5 2" /><path d="M12 15.5V2.5M8.5 6 12 2.5 15.5 6" />',
 # drawing a shape (MU43): a line by its two ends, a rectangle by two corners
 "shape-line": '<path d="M5.5 18.5 18.5 5.5" /><rect x="2.5" y="17.5" width="4" height="4" /><rect x="17.5" y="2.5" width="4" height="4" />',
 "shape-rect": '<rect x="4" y="6.5" width="16" height="11" /><rect x="2.5" y="5" width="3" height="3" /><rect x="18.5" y="16" width="3" height="3" />',
 # the strip beside a selected shape (MU43)
 "shape-join": '<rect x="3" y="8" width="9" height="9" /><rect x="12" y="8" width="9" height="9" /><path d="M9 4.5h6M12 1.5v6" />',
 "shape-separate": '<rect x="2.5" y="8" width="7" height="9" /><rect x="14.5" y="8" width="7" height="9" /><path d="M10.5 12.5h3" stroke-dasharray="1 1.5" /><path d="M7 4.5 4.5 2M17 4.5 19.5 2" />',
 "shape-close": '<path d="M5 18 4 7l12-2 4 12" /><path d="M5 18l15-1" stroke-dasharray="2.5 2" /><rect x="3" y="16.5" width="3" height="3" /><rect x="18.5" y="15.5" width="3" height="3" />',
 "shape-wire": '<path d="M4 9 12 4l8 5-2 10-11 1z" stroke-dasharray="3 2.2" />',
 "shape-face": '<path d="M4 9 12 4l8 5-2 10-11 1z" /><path d="M8 16.5l8.5-8.5M6.5 12l5-5M12 18.5l6-6" />',
 "shape-inset": '<rect x="3" y="3" width="18" height="18" /><rect x="8" y="8" width="8" height="8" /><path d="M3 3l5 5M21 3l-5 5M21 21l-5-5M3 21l5-5" />',
 "shape-piece": '<path d="M3 9.5 9 6.5l6 3v6l-6 3-6-3z" /><path d="M3 9.5l6 3 6-3M9 12.5v6" /><rect x="16.5" y="4" width="5" height="5" stroke-dasharray="1.5 1.5" />',
 "sel-box": '<rect x="4" y="5" width="16" height="14" stroke-dasharray="3 2" />',
 "sel-circle": '<circle cx="12" cy="12" r="8" stroke-dasharray="3 2" />',
 "sel-lasso": '<path d="M7 18c-3-2-4-6-1-9s9-4 12-1 2 8-3 9c-3 1-6 0-6-2" stroke-dasharray="3 2" /><path d="M8 18l-2 4" />',
 "sel-new": '<rect x="5" y="5" width="14" height="14" />',
 "sel-add": '<rect x="3" y="3" width="11" height="11" /><rect x="10" y="10" width="11" height="11" /><path d="M15.5 13v6M12.5 16h6" />',
 "sel-sub": '<rect x="3" y="3" width="11" height="11" /><rect x="10" y="10" width="11" height="11" /><path d="M12.5 16h6" />',
 "sel-invert": '<rect x="3" y="3" width="11" height="11" /><rect x="10" y="10" width="11" height="11" /><path d="M10 10h4v4h-4z" />',
 "sel-intersect": '<rect x="3" y="3" width="11" height="11" stroke-dasharray="2 2" /><rect x="10" y="10" width="11" height="11" stroke-dasharray="2 2" /><path d="M10 10h4v4h-4z" />',
 "split": '<rect x="3" y="5" width="18" height="14" rx="1" /><path d="M14 2 10 22" />',
 "field": '<path d="M9 3H7v6l-2 3 2 3v6h2M15 3h2v6l2 3-2 3v6h-2" />',
 "text-tool": '<path d="M4 7V4h16v3M9 20h6M12 4v16" />',
 "image": '<rect x="3" y="3" width="18" height="18" rx="2" /><circle cx="9" cy="9" r="2" /><path d="M21 15 16 10 5 21" />',
 "log": '<path d="m8 4-4 4 4 4M16 4l4 4-4 4" /><path d="M4 16h16M4 20h11" />',
 # the Data (IFC) tab: lines of what the file says (Properties order REV04, MU58)
 "ifc-data": '<path d="M4 6h16M4 12h16M4 18h10" />',
 "problem": '<path d="M12 3 2 20h20z" /><path d="M12 10v4M12 17h.01" />',
 "corner-mitre": '<path d="M4 20V9l5-5h11v6h-7l-3 3v7z" /><path d="M10 13 4 9" />',
 "corner-butt": '<path d="M4 20V4h16v6H10v10z" /><path d="M10 10V4" />',
 "corner-apart": '<path d="M4 20V11h5v9z" /><path d="M11 4h9v6h-9z" />',
 "circle": '<circle cx="12" cy="12" r="9" />',
 "ellipse": '<ellipse cx="12" cy="12" rx="10" ry="6" />',
 "shell-closed": '<rect x="3" y="4" width="18" height="16" /><rect x="7" y="8" width="10" height="8" />',
 "shell-top": '<path d="M3 4v16h18V4" /><path d="M7 4v12h10V4" />',
 "shell-bottom": '<path d="M3 20V4h18v16" /><path d="M7 20V8h10v12" />',
 "shell-both": '<path d="M3 4v16M7 4v16M17 4v16M21 4v16" />',
 "mod-slice": '<rect x="5" y="3" width="14" height="5" /><rect x="5" y="10" width="14" height="5" /><rect x="5" y="17" width="14" height="4" />',
 "mod-taper": '<path d="M5 21h14L15 3H9z" />',
 "mod-twist": '<path d="M6 21h12M8 3h8" /><path d="M6 21 16 3M18 21 8 3" />',
 "mod-smooth": '<circle cx="12" cy="12" r="8" /><path d="M5 9c4 3 10 3 14 0" />',
 # Stretch (wave 3): a box over one side of a shape, that side drawn out
 "stretch": '<path d="M13 6H3v12h10" /><path d="M10 3h7v18h-7z" stroke-dasharray="2 2" /><path d="M12 12h9M18 9l3 3-3 3" />',
 "solid-add": '<path d="M3 3h11v7h7v11H10v-7H3z" />',
 "solid-subtract": '<path d="M3 3h11v7h-4v4H3z" /><path d="M10 10h11v11H10z" stroke-dasharray="2 2" />',
 "solid-intersect": '<path d="M3 3h11v11H3zM10 10h11v11H10z" stroke-dasharray="2 2" /><path d="M10 10h4v4h-4z" />',
 "reach-drawn": '<rect x="6" y="8" width="12" height="8" />',
 "reach-up": '<rect x="6" y="13" width="12" height="8" /><path d="M12 10V2M9 5l3-3 3 3" />',
 "reach-down": '<rect x="6" y="3" width="12" height="8" /><path d="M12 14v8M9 19l3 3 3-3" />',
 "reach-both": '<rect x="6" y="9" width="12" height="6" /><path d="M12 7V1M9.5 3.5 12 1l2.5 2.5M12 17v6M9.5 20.5 12 23l2.5-2.5" />',
 # the Visuals tab: a disc, half in shade (owner's pick A, 2026-11-20)
 "visuals": '<circle cx="12" cy="12" r="9" /><path d="M12 3v18M12 7.5h7.8M12 12h9M12 16.5h7.8" />',
 # a picture added to the renders, and reading a colour off one
 "image-plus": '<path d="M16 5h6" /><path d="M19 2v6" /><path d="M21 11.5V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7.5" /><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21" /><circle cx="9" cy="9" r="2" />',
 "pipette": '<path d="m2 22 1-1h3l9-9" /><path d="M3 21v-3l9-9" /><path d="m15 6 3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3l.4.4Z" />',
 # X-ray as the 3D header draws it: a sphere seen through (MU108, MU118)
 "xray-ball": '<circle cx="12" cy="12" r="9" /><path d="M12 3a5 9 0 0 0 0 18" stroke-dasharray="2.5 2" /><circle cx="14.5" cy="14" r="2.2" />',
 # cropping a render
 "crop": '<path d="M6 2v14a2 2 0 0 0 2 2h14" /><path d="M18 22V8a2 2 0 0 0-2-2H2" />',
 "mod-solidify": '<path d="M3 9l9-5 9 5-9 5z" /><path d="M3 9v5l9 5 9-5V9" />',
}
out={}
for n in names:
    s=open(src+n+'.svg').read()
    s=re.sub(r'<!--.*?-->','',s,flags=re.S); inner=re.search(r'<svg[^>]*>(.*)</svg>', s, re.S).group(1)
    out[n]=re.sub(r'\s+',' ',inner).strip()
out.update(CUSTOM)
out.update(FINAL)
sys.stdout.write("// Icons from Lucide (https://lucide.dev), ISC License; see LICENSE-lucide.\n")
sys.stdout.write("window.ICONS = "+json.dumps(out, indent=0)+";\n")
