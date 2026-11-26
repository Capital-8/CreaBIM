# IFC

**IFC** (Industry Foundation Classes) is the open file format that BIM programs use to share models. An IFC file says not just what shapes there are, but what they are: this is a wall on the ground floor, made of brick, with these doors in it. CreaBIM uses IFC ids for its own elements, so a model that goes out and comes back keeps its identity.

## Exporting

- *File › IFC4 (.ifc)*: the usual IFC file, for Revit, ArchiCAD, Vectorworks, Bonsai (BlenderBIM), FreeCAD, Solibri and IFC viewers.
- *File › IFC5 (.ifcx, alpha)*: the new JSON-based IFC. Its schema is still an alpha, so other tools may not read it yet.

Walls, slabs, roofs, columns, beams, doors and windows carry their base quantities (*Qto_WallBaseQuantities* and the like), measured as the take-off measures them. Design options go out as groups, one for each option, holding what is in it alone.

What goes out: levels (storeys), walls with their location line and layers, slabs, roofs, doors and windows with their hand and swing, openings and shafts, columns, beams, stairs, railings, curtain walls with their mullions and panels, types, materials with their colours, furniture and fixtures with their types, and masses with their IFC class. Guides, sketches and crop boxes stay behind.

### How walls, slabs and roofs go out

Before writing, the export asks how walls, slabs and roofs go out, and says what the other program will see under each. The choice is remembered.

| Choice | The other program gets |
| --- | --- |
| **As modelled elements** (recommended) | One IfcWall or IfcSlab each, with its layer set and its plain body. It comes back into CreaBIM as it went out. |
| **Joined shapes, as drawn** | Their bodies cut where walls run through slabs, as CreaBIM shows them |
| **Layers as separate elements** | The structure stays the IfcWall or IfcSlab; finishes, insulation and membranes become IfcCovering elements (cladding, flooring, roofing) covering it. For trades and quantities by layer. |

The project's name, site and building, from the *Project* tab (*Manage › Project information*), name the IFC project, site and building.

## The Data (IFC) tab

Select an element and open the **Data (IFC)** tab in *Properties* to see what an IFC file says of it:

- **Identity**: its **GlobalId**, the 22-character identity every IFC file it is exported to gives it, the same each time (the copy button puts it on the clipboard), its **IFC class** and its **name**.
- **Where it sits**: the **site** and **building** it is in, named in the *Project* tab, and its **storey**, one for each level. Click the storey to select the level.
- **Property sets** and **Quantities**: the standard sets IFC gives an element of its kind, by name (*Pset_WallCommon*, *Qto_WallBaseQuantities*), each marked **written** (the export writes it today: a room's space sets, CreaBIM's own footing and ceiling sets) or **planned** (not yet), with a line on what it holds and a few of its values. Quantities are measured from the model, as the take-off measures them; a property CreaBIM doesn't track yet reads *Not set*.
- **Kept from the IFC file**: for an element read from IFC, the property sets and classification the file gave it, as the file wrote them, and what the file related it to that CreaBIM doesn't model: its **type** when it came in as a mesh, the **assembly** it is part of, its **material** set (layers, profiles, constituents). CreaBIM doesn't use these values; it carries them, and the export writes them back on the element. Where the export writes a set of the same name itself (a *Pset_WallCommon* with the phase's *Status*), CreaBIM's values stand and the file's other properties join that set.
- **Documents**: the element's data sheets, specs and links, each opened with one click (a file with its program, a link in your browser). **Add: File…** picks a file; one inside the project's folder is kept by its path from the project, so it moves with it. **Link…** takes a web address. Its own can be removed (×); a **Type** one is its type's, which every element of the type shows (add those in the type's Properties); a **From the file** one came in with the IFC and goes back out as it came. A file that isn't where the record says reads *not found* but is still written. The export writes each as an *IfcDocumentReference*, one shared by everything that has it. CreaBIM keeps where a document is, never what is in it.
- **Common sets** (*Pset_WallCommon*, *Pset_DoorCommon*…): written from the element's **Declared ratings** (Properties, after Material): fire, acoustic and U-value as declared on its type or itself; **External** worked out from the rooms (a roof always, a door or window as its wall) unless set Yes or No; **Load-bearing** from its layers (a bearing core) unless set. Nothing unknown is written. A file's common sets come back into these fields, so they are edited, not kept beside.
- **From another file**: for an element read from IFC, FreeCAD or Blender, the file it came from and its name or GlobalId there.
- **Assign as**, at the foot: turn a shape or an imported element into a wall, slab, door or window, or give a shape a class.

## Opening an IFC file

*File › Open…* reads IFC2X3, IFC4 and IFC4X3 files, and `.ifcx`.

- **Walls** become native, editable walls when their body is a straight upright prism, whether the file writes it as an extrusion or as a faceted B-rep or face set (as FreeCAD and many other tools do). A B-rep wall may already have its door and window holes cut: they are read back as openings. A wall drawn round a room along a polyline axis, perhaps one extrusion per layer, becomes one wall per straight run, meeting at the corners.
- **Slabs** become native when they are flat upright prisms, with any holes, and with their top at any height: off a storey, the slab sits on the storey below with a top offset.
- **Doors and windows** become native in a native wall: through the file's opening relationships (`IfcRelVoidsElement`, `IfcRelFillsElement`), or, when the file has none, in the wall their body sits in. They take the size of the hole cut in the wall when there is one, so they show in the door and window schedules and get marks. A hole nothing fills becomes a wall opening.
- **Columns and beams** become native when they are straight prisms: an upright one with a rectangular or round section, a level bar with a rectangular section. Their types come back. Columns, beams, roofs, stairs and railings CreaBIM wrote come back exactly as they were.
- Types, assemblies, materials and grid systems come back too. Furniture and fixtures with an IfcFurnitureType, IfcSanitaryTerminalType, IfcElectricApplianceType or IfcSystemFurnitureElementType that has a body map come in as [objects](furniture) of that type.
- Everything else with a shape (a gable wall, a sloped roof slab, stairs and railings from other tools, terrain…) comes in as a mesh that keeps its IFC class. Openings in a mesh are cut into it, and its corners are joined up, so it is closed.
- Units, placements and IFC ids are kept, and a plan is made for each storey.
- **Property sets** (single and enumerated values) and **classification references** are kept on the elements that came in, and go back out on export. Quantities are not kept: CreaBIM measures its own.
- What the file relates that CreaBIM doesn't model is kept too and goes back out: the types of elements that came in as meshes, assemblies and their parts, the openings mesh doors and windows fill, material layer, profile and constituent sets, and Bonsai's drawings, sheets and documents with their files' paths. The project, site and building keep their GlobalIds. What CreaBIM models itself it writes itself: a native wall keeps CreaBIM's type and layers.

## Groups and design options

An IFC file can gather elements in **groups** (IfcGroup). A group has no place and owns nothing: one element can be in several groups at once. CreaBIM keeps the groups a file brings in, lists them in the project browser's **Groups** section with how many pieces each holds, and writes them back as they came.

- Click a group to select its pieces; its eye hides and shows them.
- Select an element and *Properties* names the groups it is in: they go back to IFC with it.

Some files keep **design options** as groups: four versions of a house that share most of their pieces, say. When the groups look like that, the browser offers to **Make them options**, or **Keep as groups**. Nothing changes until you choose. You can also `Ctrl+click` two or more groups in the browser, whenever you like, to make them options by hand.

Made into options, they become one question of CreaBIM's own [options](phases#options):

- a piece in every group is **shared** by all the options;
- a piece in one group is built in **that option** alone;
- a piece in some of the groups is **copied**, one copy for each further option. Each copy remembers the piece it stands for: unchanged, it goes back to IFC as that piece; changed, as a piece of its own.

The groups stay in the project, each linked to the option made from it, so the file's own grouping is never lost. Pick the option a view shows from the tag beside its phase.

## The import report

While a file opens, its card counts the elements read and the seconds so far. Opened, the card says how long it took, and so does the report.

When an import has anything to say, the **Import report** opens: how many elements of each IFC class came in native and how many as meshes, then the notes grouped by kind with a count. Each note folds open on the elements it names, by class, name and GlobalId, with the reason, for example:

> IfcWall 'Wall-Gable-E' (0NBfRAfdjALxRLSMXhQcSA) is an IfcFacetedBrep that has 2 sloped faces (a raked or gable top, or a battered face); imported as a mesh

A large file reads as its dozen reasons first, not a thousand lines.

*Copy report* copies it all as text, to fix the model in the program that wrote it or to send with a question. *File › Import report* opens it again. `creabim import-ifc` prints the same notes and list.

## Materials only

*Materials › Import materials…* reads just the materials of an IFC file, with their categories and surface styles.
