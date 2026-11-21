use std::sync::Arc;

fn is_valid_font_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes.starts_with(&[0, 1, 0, 0])
            || bytes.starts_with(b"OTTO")
            || bytes.starts_with(b"true"))
}

fn append_font_to_families(fonts: &mut egui::FontDefinitions, font_name: &str) {
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            if !list.iter().any(|name| name == font_name) {
                list.push(font_name.to_owned());
            }
        }
    }
}

fn try_register_system_font(fonts: &mut egui::FontDefinitions, name: &str, candidates: &[&str]) {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if is_valid_font_bytes(&bytes) {
                fonts.font_data.insert(
                    name.to_owned(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                append_font_to_families(fonts, name);
                break;
            }
        }
    }
}

pub fn build_font_definitions(include_system_fallbacks: bool) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        if !prop.iter().any(|name| name == "Hack") {
            prop.push("Hack".to_owned());
        }
    }

    if include_system_fallbacks {
        #[cfg(target_os = "windows")]
        {
            try_register_system_font(
                &mut fonts,
                "SegoeUISymbol",
                &[r"C:\Windows\Fonts\seguisym.ttf"],
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            try_register_system_font(
                &mut fonts,
                "SystemSymbol",
                &[
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "/usr/share/fonts/TTF/DejaVuSans.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
                ],
            );
        }
    }

    fonts
}

pub fn configure_app_fonts(ctx: &egui::Context) {
    ctx.set_fonts(build_font_definitions(true));
}
