//! The phase an area stands at of its own (MU133, Phase per area REV00):
//! while the areas loop draws such an area, whatever reads its view's
//! phase reads the area's instead. The view itself, and its sheets, keep
//! theirs.

use creabim_core::ElementId;

thread_local! {
    /// The area being drawn, when it stands at a phase of its own: its
    /// view, and that phase.
    static LENS: std::cell::Cell<Option<(ElementId, u32)>> = const { std::cell::Cell::new(None) };
}

/// The phase `view` is seen at here: the area's own while one standing
/// at a phase of its own is drawn, else `own` (the view's).
pub fn lensed(view: ElementId, own: Option<u32>) -> Option<u32> {
    LENS.with(|l| match l.get() {
        Some((v, p)) if v == view => Some(p),
        _ => own,
    })
}

/// The area phase in force for `view`, if any.
pub fn lens_of(view: ElementId) -> Option<u32> {
    LENS.with(|l| l.get().filter(|(v, _)| *v == view).map(|(_, p)| p))
}

/// Stand the area being drawn at its own phase, or at its view's.
pub fn set_lens(lens: Option<(ElementId, u32)>) {
    LENS.with(|l| l.set(lens));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_areas_phase_stands_in_for_its_view_alone() {
        let (a, b) = (ElementId::new(), ElementId::new());
        assert_eq!(lensed(a, Some(1)), Some(1));
        set_lens(Some((a, 2)));
        assert_eq!(lensed(a, Some(1)), Some(2));
        assert_eq!(lensed(b, Some(1)), Some(1), "another view keeps its own");
        assert_eq!(lens_of(a), Some(2));
        set_lens(None);
        assert_eq!(lensed(a, Some(1)), Some(1));
    }
}
