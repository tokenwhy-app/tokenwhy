// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Only the given fonts are used (no system font lookup), so every machine draws the same pixels.

use resvg::{tiny_skia, usvg};
use std::sync::Arc;

fn tree(svg: &str, fonts: &[&[u8]]) -> Result<usvg::Tree, String> {
    let mut db = usvg::fontdb::Database::new();
    for f in fonts {
        db.load_font_data(f.to_vec());
    }
    let opt = usvg::Options {
        fontdb: Arc::new(db),
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())
}

pub fn png(svg: &str, fonts: &[&[u8]]) -> Result<Vec<u8>, String> {
    let t = tree(svg, fonts)?;
    let size = t.size().to_int_size();
    let mut pix = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty card")?;
    resvg::render(&t, tiny_skia::Transform::default(), &mut pix.as_mut());
    pix.encode_png().map_err(|e| e.to_string())
}

pub fn text_boxes(svg: &str, fonts: &[&[u8]]) -> Result<Vec<(String, [f32; 4])>, String> {
    fn walk(g: &usvg::Group, out: &mut Vec<(String, [f32; 4])>) {
        for n in g.children() {
            match n {
                usvg::Node::Group(c) => walk(c, out),
                usvg::Node::Text(t) => {
                    let b = t.flattened().abs_bounding_box();
                    out.push((
                        t.id().to_string(),
                        [b.left(), b.top(), b.right(), b.bottom()],
                    ));
                }
                _ => {}
            }
        }
    }
    let t = tree(svg, fonts)?;
    let mut out = vec![];
    walk(t.root(), &mut out);
    Ok(out)
}
