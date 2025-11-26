use resvg::usvg::{self, TreeParsing};
use std::fs;

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
    let svg_size = rtree.size;

    // Calculate scale to fit SVG into the target size
    let scale_x = size as f32 / svg_size.width();
    let scale_y = size as f32 / svg_size.height();
    let scale = scale_x.min(scale_y); // Maintain aspect ratio

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    rtree.render(transform, &mut pixmap.as_mut());

    pixmap
        .save_png("heike_icon.png")
        .expect("Failed to save PNG");

    println!("Generated heike_icon.png (256x256)");
}
