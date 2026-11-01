use crate::egui::Color32;

pub const BACKDROP: Color32 = Color32::from_rgb(0x16, 0x16, 0x15);
pub const FLOOR: Color32 = Color32::from_rgb(0x22, 0x21, 0x1f);
pub const PANEL: Color32 = Color32::from_rgb(0x2c, 0x2b, 0x29);
pub const TOLEX: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
pub const PLATE: Color32 = Color32::from_rgb(0xc9, 0xb8, 0x8a);
pub const PLATE_INK: Color32 = Color32::from_rgb(0x1c, 0x1a, 0x16);
pub const INK: Color32 = Color32::from_rgb(0xec, 0xea, 0xe4);
pub const DIM: Color32 = Color32::from_rgb(0x7c, 0x79, 0x72);
pub const FAINT: Color32 = Color32::from_rgb(0x3a, 0x39, 0x36);
pub const ORANGE: Color32 = Color32::from_rgb(0xff, 0x5b, 0x14);
pub const GREEN: Color32 = Color32::from_rgb(0x3c, 0xc8, 0x6a);
pub const RED: Color32 = Color32::from_rgb(0xe0, 0x32, 0x2c);
pub const KNOB: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
pub const KNOB_CAP: Color32 = Color32::from_rgb(0xe8, 0xe4, 0xd8);
pub const METAL: Color32 = Color32::from_rgb(0xa8, 0xa8, 0xa4);

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let c = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(c(a.r(), b.r()), c(a.g(), b.g()), c(a.b(), b.b()))
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn ink_on(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 140.0 { PLATE_INK } else { INK }
}
