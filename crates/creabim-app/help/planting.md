# Planting and trees

Plants are part of the [site](site): trees, conifers, palms and shrubs, drawn in plans as symbols and grown in 3D. Each stands on the ground under it, so it follows a slope, unless a slab or a roof under it holds it up.

## Planting a tree or a shrub

1. In the Site panel, under **Planting**, pick a **Tree**, **Conifer**, **Palm** or **Shrub**.
2. Set its *Canopy* (the spread of its crown) and *Height* in the tool options.
3. Click to plant. Keep clicking to plant more.

In plan, each plant is a symbol as wide as its canopy: a scalloped crown with its trunk for a tree, a ringed star for a conifer, fronds for a palm, a smaller scalloped outline for a shrub. In 3D it is a trunk and a crown, a cone, a tall trunk with a head of fronds, or a low mound; sections and elevations draw its outline. Give it a material to change the colour of its crown.

## Grown trees

A new tree is **grown**: a trunk and a few limbs carrying a crown of foliage clumps, fitted to the size you typed. A plant's **Element** tab holds where it stands and its overall size: *X*, *Y*, *Base*, *Height*, *Crown* (its width) and *Material*. How it is made is in its **Tree** tab, which every plant has. Its **Form** block picks the **Kind** (Tree, Conifer, Palm, Shrub) and, for a tree, *Crown*: **Simple** (one round crown) or **Generated**. A generated tree has:

- **Start from** (in Form): a preset to begin with (Shade tree, Umbrella, Columnar, Conical, Vase). Everything below stays editable.
- **Crown**: its depth, its outline up its height, its **form**, how many clumps it is made of (one clump makes the whole crown one body), their size, how irregular and how open it is. The form is how each clump is built: *Polyhedra* (twenty flat faces), *Fully random*, *Pseudo-random* and *Fibonacci* (points over the clump, wrapped into facets), *Geodesic* (a faceted sphere), *Noise* (smooth and ruffled) or *Metaballs* (the clumps fused into one smooth crown).
- **Trunk**: the clear trunk (where the crown starts) and the trunk's diameter.
- **Branches**: few heavy limbs, balanced, or many fine ones. Each limb curves up from the trunk and ends inside the crown, so none shows past the foliage. *Wood* makes the trunk and limbs **Solid** (closed bodies: a section fills their cut) or **Hollow** (open tubes).
- **Leaves and seasons**: when it is in leaf (**Evergreen**, **Deciduous, temperate**: bare in winter, fresh in spring, turning in autumn; or **Dry-season, tropical**: losing its leaves in the dry season), the **month** 3D shows it in (the strip of twelve months, a bar under each for how much of the crown is in leaf), and its **leaf** and **autumn** colours. 3D thins the crown and turns its colour with the month; plans, sections and elevations always draw the whole crown. South of the equator (the project's latitude in the *Project* tab), a tree's year runs six months on.
- **3D detail**: **Lite** (twenty faces a clump and the main limbs only, for a site of hundreds), **Everyday**, or **Presentation** (finer clumps and limbs, with twigs, for close views and renders). Plans and drawings don't change with it.
- **Plan symbol**: the crown's real outline seen from above, in one of thirteen styles: *Scalloped*, *Outline*, *Limbs* (scallops with the main limbs), *Clumps* (the outline with each clump inside it, layered as seen from above), *Serrated* (a toothed rim with lines from the trunk), *Wavy* (a wavy rim with small leaf masses), *Skeleton* (every limb and twig inside a dotted rim, a tree in winter), *Hatched rim*, *Cells* (a polygon a clump), *Wedges* (dotted), *Starburst*, *Leafy* (a faceted rim with leaves scattered inside) and *Jagged*. Every style follows the crown's own outline, clumps and limbs. **Bold outline** draws the outline heavy and what is inside it light. *Trunk and centre mark* and *Light canopy tint* turn the trunk's circle and cross, and a light wash under the crown, on or off.
- **Sections and elevations**: **Clumps** draws each clump's outline and cuts them where the section passes, rugged; **Outline** draws one smooth outline round the whole crown, cut as one, so it reads as the plan does. **Wavy** (a soft wavy outline with the gaps between the leaves), **Scalloped** (scallops with the top of each billow drawn light) and **Leafy** (a faceted outline with leaves scattered inside) are drawn as an architect would by hand, from the same crown. A section that cuts through a crown in one of these styles cuts it as one smooth body. **Limbs through the crown** draws the trunk and main limbs on up into the crown, in any style. 3D keeps the clumps either way.
- **Variation**: its seed. The same seed always grows the same tree; *New* grows another of the same shape. Each tree you plant gets its own seed from where it stands, so a row of trees is not a row of clones.

**Shaping it beside a building.** Select a generated tree to see its grips: one at its top (height), one at the crown's underside (clear trunk), and eight round the crown at its widest. Drag a side grip in to pull that side of the crown off a wall, or out to let it spread; the trunk stays where it is, and the plan, sections and 3D all follow. `Tab` types the distance. *Even out* in Properties puts every side back.

A tree is grown once and kept: editing anything else in the model doesn't grow it again.

See also [Site](site).
