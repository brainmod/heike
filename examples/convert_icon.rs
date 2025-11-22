use std::fs;
use resvg::usvg::{self, TreeParsing};

fn main() {
    // Read SVG
    let svg_data = fs::read_to_string("heike.svg").expect("Failed to read SVG");

    // Parse SVG
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_data.as_bytes(), &opts).expect("Failed to parse SVG");

    // Render at 256x256 for window icon
    let size = 256;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();

    let rtree = resvg::Tree::from_usvg(&tree);
    rtree.render(resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());

    pixmap.save_png("heike_icon.png")
        .expect("Failed to save PNG");

    println!("Generated heike_icon.png (256x256)");
}
