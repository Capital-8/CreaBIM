# Materials

A **material** is what an element is made of: concrete, brick, timber, glass. It has a colour, a **cut hatch** (the pattern drawn where a section cuts through it) and a surface (how rough, metallic or see-through it looks in 3D). Materials are exported to IFC with the elements.

## The material library

CreaBIM comes with 52 common building materials in 17 categories. *Materials › Add from the library* adds one, or a whole category with *Add them all*. The material browser's *Library* tab shows them with descriptions.

## The material browser

*Materials › Material browser…* lists the project's materials by category, with how many elements use each.

- Add, duplicate, rename and edit materials, with a live preview ball.
- *Select them* selects the elements that use a material; *Assign to the selection* gives it to the selection.
- *Delete* asks what the elements still using it should be made of instead.
- *Rate* gives it a price per m², m³, m or piece for the [take-off](take-off).

![The material browser](img/materials.jpg)

Selecting a material in the project browser shows the same editor in *Properties*.

## Giving an element a material

Select it, then pick the material in *Properties*, or use *Materials › Assign material*. This works for walls, slabs, roofs, masses, doors, windows, columns, beams, stairs and railings. Layered walls, floors and roofs get their materials from their [assembly](assemblies).

**The Material section** sits in the same place for every element, after *Preview* and before *Geometry*. An element of one material shows it with its swatch and a line on how it draws (its cut hatch and surface pattern). A layered one shows its **Assembly** and lists its layers, each with its swatch, name and thickness; **Edit the assembly…** opens its layers in the Assemblies window. Nothing shows for an element with no material, such as a void.

## The material card

Click a material's **swatch**, in any row that sets a material, and its **card** opens in place. It edits what is changed most:

- its **Name**, its **Colour** (in 3D and in coloured drawings), its **Cut hatch** and its **Surface** pattern;
- how many it reaches: *Used by 12 elements: a change here changes them all*, counting the elements built of assemblies that have it as a layer;
- **Make a copy for this one**: a copy of the material, used here only, to change on its own;
- **More in Materials…** for everything else, in the material browser.

In a view, `Alt`+click an element, or right-click it and pick **Material: …**, to open the card of what was clicked, right there.

## Importing materials

*Materials › Import materials…* reads materials from IFC files, FreeCAD material cards (`.FCMat`), glTF files (`.gltf`, `.glb`) and Blender files (`.blend`). A material whose name the project already has is updated, so its elements follow.
