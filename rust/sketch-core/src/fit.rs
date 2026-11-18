use crate::Affine;

/// Aspect-fits content of size `content_w × content_h` into a box, centred, with uniform scale.
pub fn fit_transform(content_w: f64, content_h: f64, box_w: f64, box_h: f64) -> Affine {
    let scale = (box_w / content_w).min(box_h / content_h);
    Affine::scale_translate(
        scale,
        (box_w - content_w * scale) / 2.0,
        (box_h - content_h * scale) / 2.0,
    )
}
