//! How far a long job has got (an export, an import), for its progress
//! bar: the job calls its tick with `n` of `total` done and the step it is
//! in. A tick is called often, so it should stay cheap.

/// Told `n` of `total` done, now in `step`.
pub type Tick<'a> = &'a dyn Fn(usize, usize, &str);

/// A tick that tells no one, for jobs nobody watches.
pub fn quiet(_: usize, _: usize, _: &str) {}

/// The step an element is written or read in, named for people, by its
/// IFC class.
pub fn step_of(class: &str) -> &'static str {
    let c = class.to_ascii_lowercase();
    let c = c.strip_prefix("ifc").unwrap_or(&c);
    if c.starts_with("wall") || c.starts_with("curtainwall") {
        "Walls"
    } else if c.starts_with("slab") || c.starts_with("footing") || c.starts_with("covering") {
        "Slabs"
    } else if c.starts_with("roof") {
        "Roofs"
    } else if c.starts_with("door") || c.starts_with("window") || c.starts_with("opening") {
        "Doors and windows"
    } else if c.starts_with("stair") || c.starts_with("railing") || c.starts_with("ramp") {
        "Stairs and railings"
    } else if c.starts_with("beam") || c.starts_with("column") || c.starts_with("member") {
        "Beams and columns"
    } else if c.starts_with("furnish") || c.starts_with("furniture") {
        "Furniture"
    } else if c.starts_with("space") {
        "Rooms"
    } else if c.starts_with("buildingstorey") {
        "Levels"
    } else {
        "Other elements"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_named_by_class() {
        assert_eq!(step_of("IfcWall"), "Walls");
        assert_eq!(step_of("IFCWALLSTANDARDCASE"), "Walls");
        assert_eq!(step_of("IfcWindow"), "Doors and windows");
        assert_eq!(step_of("IfcFurniture"), "Furniture");
        assert_eq!(step_of("IfcProxy"), "Other elements");
    }
}
