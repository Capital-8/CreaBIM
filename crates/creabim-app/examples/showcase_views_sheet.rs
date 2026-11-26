//! Puts a project's hero views (the PNGs `showcase_render` wrote) on a
//! sheet of their own: pictures kept in a copy of the project's title
//! block, so the project and its PDF carry them. The sheet itself is
//! `creabim_core::ridgeline::hero_views_sheet`, the one the starter house
//! opens with.
//!
//! `cargo run --profile fast -p creabim-app --example showcase_views_sheet -- <model.creabim> <views-dir>`

use creabim_core::ridgeline::{hero_views_sheet, HeroPicture, HERO_VIEWS};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, dir) = (&args[1], Path::new(&args[2]));
    let mut doc = creabim_store::load(model).expect("the project opens");
    let pictures = HERO_VIEWS.map(|(file, _)| {
        let file = format!("{file}.png");
        let img = image::open(dir.join(&file))
            .unwrap_or_else(|e| panic!("{file}: {e}"))
            .resize(1400, 875, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        HeroPicture {
            width: img.width(),
            height: img.height(),
            rgba: creabim_geom::raster::store_image(img.as_raw()),
        }
    });
    let tx = hero_views_sheet(doc.model(), pictures).expect("a title block and a sheet");
    doc.commit(tx).expect("the sheet is valid");
    creabim_store::save(model, &doc).expect("the project saves");
    println!("added sheet A-902 Hero Views to {model}");
}
