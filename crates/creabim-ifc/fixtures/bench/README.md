# IFC bench corpus

Files for `creabim ifc-check` (the IFC bench, stage 1 of the exchange
roadmap). Run the whole bench with

    creabim ifc-check crates/creabim-ifc/fixtures/*.ifc \
        crates/creabim-ifc/fixtures/thirdparty/*.ifc \
        crates/creabim-ifc/fixtures/bench/*.ifc --table bench.md

The other fixture folders are part of the bench too; this folder holds the
files written for it.

| File | What it is |
|------|------------|
| `carried-data.ifc` | `../ifcopenshell-ifc4.ifc` with what CreaBIM does not understand added by hand, the way Bonsai and other tools write it: an unknown property set on two walls, a quantity set, a property set on the wall type, a `BBIM_` set on the window, Uniclass 2015 references on walls, the type and the door, an IfcGroup and an IfcZone, a data-sheet document on the door, and a Bonsai drawing (`IfcAnnotation` DRAWING with `EPset_Drawing`, its group and its DRAWING document) placed on a sheet (`IfcDocumentInformation` SHEET with LAYOUT and DRAWING references). Stage 2 ("do no harm") must bring all of it back. |

Files from elsewhere that may not be redistributed stay out of the repo,
in a local corpus folder, and are listed in the bench report by name only.
