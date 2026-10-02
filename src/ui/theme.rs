use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub key_fg: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub line_num_fg: Color,
    pub line_num_bg: Color,
    pub add_bg: Color,
    pub add_fg: Color,
    pub add_intraline: Color,
    pub del_bg: Color,
    pub del_fg: Color,
    pub del_intraline: Color,
    pub virtual_bg: Color,
    pub virtual_fg: Color,
    pub ruler_bg: Color,
    pub ruler_viewport: Color,
    pub status_m: Color,
    pub status_a: Color,
    pub status_d: Color,
    pub status_u: Color,
}

pub fn detect_dark_mode() -> bool {
    // 1. Check COLORFGBG environment variable (format: "fg;bg")
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = colorfgbg.split(';').collect();
        if let Some(bg_str) = parts.last() {
            if let Ok(bg_num) = bg_str.parse::<u8>() {
                return bg_num <= 6 || bg_num == 8;
            }
        }
    }

    // 2. On macOS, query AppleInterfaceStyle
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            if output.status.success() {
                let style = String::from_utf8_lossy(&output.stdout);
                if style.trim().eq_ignore_ascii_case("Dark") {
                    return true;
                }
            } else {
                return false;
            }
        }
    }

    // 3. On Linux, query GNOME / Freedesktop color-scheme
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            if output.status.success() {
                let scheme = String::from_utf8_lossy(&output.stdout);
                if scheme.contains("prefer-light") {
                    return false;
                }
            }
        }
    }

    // Default to dark mode
    true
}

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "auto" | "terminal" | "default" | "system" => Self::terminal(),
            "tokyonight" | "tokyo-night" => Self::tokyonight(),
            "catppuccin" | "catppuccin-mocha" => Self::catppuccin(),
            "gruvbox" => Self::gruvbox(),
            "vscode-dark" | "vscode" => Self::vscode_dark(),
            _ => Self::terminal(),
        }
    }

    pub fn terminal() -> Self {
        let is_dark = detect_dark_mode();
        if is_dark {
            Self {
                name: "terminal-dark".to_string(),
                bg: Color::Reset,
                fg: Color::Reset,
                border: Color::Rgb(69, 71, 90),
                header_bg: Color::Reset,
                header_fg: Color::Rgb(137, 180, 250),
                status_bg: Color::Rgb(24, 25, 38),
                status_fg: Color::Rgb(205, 214, 244),
                key_fg: Color::Rgb(242, 205, 172),
                selected_bg: Color::Rgb(49, 50, 68),
                selected_fg: Color::Rgb(245, 245, 255),
                line_num_fg: Color::Rgb(108, 112, 134),
                line_num_bg: Color::Reset,
                add_bg: Color::Rgb(26, 44, 34),
                add_fg: Color::Rgb(166, 227, 161),
                add_intraline: Color::Rgb(40, 84, 56),
                del_bg: Color::Rgb(48, 24, 30),
                del_fg: Color::Rgb(243, 139, 168),
                del_intraline: Color::Rgb(95, 38, 50),
                virtual_bg: Color::Reset,
                virtual_fg: Color::Rgb(88, 91, 112),
                ruler_bg: Color::Reset,
                ruler_viewport: Color::Rgb(137, 180, 250),
                status_m: Color::Rgb(249, 226, 175),
                status_a: Color::Rgb(166, 227, 161),
                status_d: Color::Rgb(243, 139, 168),
                status_u: Color::Rgb(137, 180, 250),
            }
        } else {
            Self {
                name: "terminal-light".to_string(),
                bg: Color::Reset,
                fg: Color::Reset,
                border: Color::Rgb(200, 205, 215),
                header_bg: Color::Reset,
                header_fg: Color::Rgb(20, 100, 220),
                status_bg: Color::Rgb(235, 240, 248),
                status_fg: Color::Rgb(40, 50, 70),
                key_fg: Color::Rgb(190, 120, 20),
                selected_bg: Color::Rgb(220, 232, 250),
                selected_fg: Color::Rgb(20, 30, 50),
                line_num_fg: Color::Rgb(150, 155, 170),
                line_num_bg: Color::Reset,
                add_bg: Color::Rgb(225, 245, 228),
                add_fg: Color::Rgb(28, 130, 48),
                add_intraline: Color::Rgb(185, 232, 192),
                del_bg: Color::Rgb(252, 230, 232),
                del_fg: Color::Rgb(210, 40, 60),
                del_intraline: Color::Rgb(248, 196, 202),
                virtual_bg: Color::Reset,
                virtual_fg: Color::Rgb(190, 195, 205),
                ruler_bg: Color::Reset,
                ruler_viewport: Color::Rgb(30, 120, 240),
                status_m: Color::Rgb(200, 130, 20),
                status_a: Color::Rgb(28, 130, 48),
                status_d: Color::Rgb(210, 40, 60),
                status_u: Color::Rgb(30, 120, 240),
            }
        }
    }

    pub fn vscode_dark() -> Self {
        Self {
            name: "vscode-dark".to_string(),
            bg: Color::Rgb(30, 30, 30),
            fg: Color::Rgb(204, 204, 204),
            border: Color::Rgb(60, 60, 60),
            header_bg: Color::Rgb(37, 37, 38),
            header_fg: Color::Rgb(220, 220, 220),
            status_bg: Color::Rgb(0, 122, 204),
            status_fg: Color::Rgb(255, 255, 255),
            key_fg: Color::Rgb(255, 235, 120),
            selected_bg: Color::Rgb(4, 57, 94),
            selected_fg: Color::Rgb(255, 255, 255),
            line_num_fg: Color::Rgb(110, 110, 110),
            line_num_bg: Color::Rgb(30, 30, 30),
            add_bg: Color::Rgb(20, 48, 25),
            add_fg: Color::Rgb(140, 220, 140),
            add_intraline: Color::Rgb(35, 90, 45),
            del_bg: Color::Rgb(55, 22, 25),
            del_fg: Color::Rgb(240, 140, 140),
            del_intraline: Color::Rgb(105, 35, 40),
            virtual_bg: Color::Rgb(25, 25, 25),
            virtual_fg: Color::Rgb(50, 50, 50),
            ruler_bg: Color::Rgb(35, 35, 35),
            ruler_viewport: Color::Rgb(90, 90, 90),
            status_m: Color::Rgb(226, 192, 141),
            status_a: Color::Rgb(115, 201, 145),
            status_d: Color::Rgb(235, 100, 100),
            status_u: Color::Rgb(140, 175, 255),
        }
    }

    pub fn tokyonight() -> Self {
        Self {
            name: "tokyonight".to_string(),
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(192, 202, 245),
            border: Color::Rgb(41, 46, 66),
            header_bg: Color::Rgb(31, 35, 53),
            header_fg: Color::Rgb(122, 162, 247),
            status_bg: Color::Rgb(41, 46, 66),
            status_fg: Color::Rgb(192, 202, 245),
            key_fg: Color::Rgb(224, 175, 104),
            selected_bg: Color::Rgb(54, 76, 120),
            selected_fg: Color::Rgb(255, 255, 255),
            line_num_fg: Color::Rgb(86, 95, 137),
            line_num_bg: Color::Rgb(26, 27, 38),
            add_bg: Color::Rgb(25, 45, 40),
            add_fg: Color::Rgb(158, 206, 106),
            add_intraline: Color::Rgb(45, 80, 60),
            del_bg: Color::Rgb(50, 25, 35),
            del_fg: Color::Rgb(247, 118, 142),
            del_intraline: Color::Rgb(90, 35, 50),
            virtual_bg: Color::Rgb(22, 22, 30),
            virtual_fg: Color::Rgb(55, 60, 80),
            ruler_bg: Color::Rgb(31, 35, 53),
            ruler_viewport: Color::Rgb(122, 162, 247),
            status_m: Color::Rgb(224, 175, 104),
            status_a: Color::Rgb(158, 206, 106),
            status_d: Color::Rgb(247, 118, 142),
            status_u: Color::Rgb(122, 162, 247),
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            name: "catppuccin".to_string(),
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            border: Color::Rgb(69, 71, 90),
            header_bg: Color::Rgb(24, 24, 37),
            header_fg: Color::Rgb(137, 180, 250),
            status_bg: Color::Rgb(49, 50, 68),
            status_fg: Color::Rgb(205, 214, 244),
            key_fg: Color::Rgb(249, 226, 175),
            selected_bg: Color::Rgb(69, 71, 90),
            selected_fg: Color::Rgb(255, 255, 255),
            line_num_fg: Color::Rgb(108, 112, 134),
            line_num_bg: Color::Rgb(30, 30, 46),
            add_bg: Color::Rgb(26, 45, 38),
            add_fg: Color::Rgb(166, 227, 161),
            add_intraline: Color::Rgb(40, 75, 60),
            del_bg: Color::Rgb(50, 30, 40),
            del_fg: Color::Rgb(243, 139, 168),
            del_intraline: Color::Rgb(90, 40, 55),
            virtual_bg: Color::Rgb(24, 24, 37),
            virtual_fg: Color::Rgb(69, 71, 90),
            ruler_bg: Color::Rgb(24, 24, 37),
            ruler_viewport: Color::Rgb(137, 180, 250),
            status_m: Color::Rgb(249, 226, 175),
            status_a: Color::Rgb(166, 227, 161),
            status_d: Color::Rgb(243, 139, 168),
            status_u: Color::Rgb(137, 180, 250),
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".to_string(),
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            border: Color::Rgb(80, 73, 69),
            header_bg: Color::Rgb(50, 48, 47),
            header_fg: Color::Rgb(250, 189, 47),
            status_bg: Color::Rgb(60, 56, 54),
            status_fg: Color::Rgb(235, 219, 178),
            key_fg: Color::Rgb(254, 128, 25),
            selected_bg: Color::Rgb(80, 73, 69),
            selected_fg: Color::Rgb(255, 255, 255),
            line_num_fg: Color::Rgb(146, 131, 116),
            line_num_bg: Color::Rgb(40, 40, 40),
            add_bg: Color::Rgb(35, 45, 30),
            add_fg: Color::Rgb(184, 187, 38),
            add_intraline: Color::Rgb(60, 80, 45),
            del_bg: Color::Rgb(50, 30, 30),
            del_fg: Color::Rgb(251, 73, 52),
            del_intraline: Color::Rgb(95, 45, 45),
            virtual_bg: Color::Rgb(30, 30, 30),
            virtual_fg: Color::Rgb(70, 65, 60),
            ruler_bg: Color::Rgb(50, 48, 47),
            ruler_viewport: Color::Rgb(250, 189, 47),
            status_m: Color::Rgb(250, 189, 47),
            status_a: Color::Rgb(184, 187, 38),
            status_d: Color::Rgb(251, 73, 52),
            status_u: Color::Rgb(131, 165, 152),
        }
    }
}
