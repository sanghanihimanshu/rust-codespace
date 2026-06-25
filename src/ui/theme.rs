use gpui::{rgb, Rgba};

// Catppuccin Macchiato palette — functions because rgb() is not const
pub fn bg()      -> Rgba { rgb(0x1e2030) }
pub fn surface() -> Rgba { rgb(0x24273a) }
pub fn overlay() -> Rgba { rgb(0x363a4f) }
pub fn muted()   -> Rgba { rgb(0x494d64) }
pub fn text()    -> Rgba { rgb(0xcad3f5) }
pub fn subtext() -> Rgba { rgb(0xa5adcb) }
pub fn accent()  -> Rgba { rgb(0x8aadf4) } // blue
pub fn green()   -> Rgba { rgb(0xa6da95) }
pub fn red()     -> Rgba { rgb(0xed8796) }
pub fn yellow()  -> Rgba { rgb(0xeed49f) }
