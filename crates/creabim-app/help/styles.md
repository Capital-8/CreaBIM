# Annotation styles

An **annotation style** is a named look for dimensions, notes and tags: font, size on paper, colour, bold or italic, the dimension ends (arrows, dots, ticks, the *Slug*…), the leader end, the end size, the leader's shape and the tag shape. Change a style and everything that uses it changes.

## The style manager

Open *File › Annotation styles…*, *Annotation styles…* at the bottom of any *Style* menu in the tool options, or the button in a style's *Properties*.

- *New* or *Duplicate* makes a style. Set its text, ends and tag shape, and what it is *Meant for* (dimensions, notes, tags or any annotation).
- The **Slug** end is a short thick stroke with round ends, as a pen leaves it: across a dimension line at 45°, or along a leader at its point. It reads as hand drafting, and works in ruled drawings too.
- **Leader** *Straight* or *Curved*: a note's leader in straight runs bent at its elbow, or one smooth curve from beside the text to what it points at, through the elbow when it has one.
- **Font**: the list writes each name in its own face.
  - *In every PDF*: Helvetica, Times and Courier, which every reader already has.
  - *CreaBIM*: Structa, Vectra, Spencer, Chisel, Kinetic, TechHand and AtelierHand.
  - *Classics*: thirteen open fonts, from Inter and IBM Plex to osifont and the hand-lettered Kalam and Patrick Hand.

  CreaBIM and Classic fonts ship with CreaBIM and go inside every SVG and PDF that uses them, so a sheet reads the same on any computer. A character a font lacks (Structa has no ²) is written in Helvetica. Each has one weight: bold is drawn with a slightly heavier outline, and italic is slanted.
- A live preview shows it.
- *Select all N using it* selects its users; *Apply to N selected* gives it to the selection.
- *New ones get it* makes it the default for new dimensions, notes or tags.
- Deleting a style returns its users to the default look.

## Using styles

- The Dimension, Note and Tag tools have a **Style** menu in the tool options, for new annotations.
- A placed annotation's style is in its *Properties*, with *New text style…* to make one on the spot.
- *Own colour* overrides the style's colour for one annotation.

Sizes are millimetres on paper, so text is the same size at any scale. Styles can be copied from another project with *File › Masters* › **Bring in an SVG, DXF or project…** (picking a `.creabim`).
