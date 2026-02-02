#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    // ANSI 256 color palette
    pub fn from_ansi_index(index: u8) -> Self {
        match index {
            // 0-15: Standard ANSI colors
            0 => Self::new(0, 0, 0),        // Black
            1 => Self::new(205, 49, 49),    // Red
            2 => Self::new(13, 188, 121),   // Green
            3 => Self::new(229, 229, 16),   // Yellow
            4 => Self::new(36, 114, 200),   // Blue
            5 => Self::new(188, 63, 188),   // Magenta
            6 => Self::new(17, 168, 205),   // Cyan
            7 => Self::new(229, 229, 229),  // White
            8 => Self::new(102, 102, 102),  // Bright Black
            9 => Self::new(241, 76, 76),    // Bright Red
            10 => Self::new(35, 209, 139),  // Bright Green
            11 => Self::new(245, 245, 67),  // Bright Yellow
            12 => Self::new(59, 142, 234),  // Bright Blue
            13 => Self::new(214, 112, 214), // Bright Magenta
            14 => Self::new(41, 184, 219),  // Bright Cyan
            15 => Self::new(229, 229, 229), // Bright White

            // 16-231: 6×6×6 RGB cube
            16..=231 => {
                let index = index - 16;
                let r = (index / 36) % 6;
                let g = (index / 6) % 6;
                let b = index % 6;

                // Convert 0-5 range to 0-255 using standard xterm color values
                let to_rgb = |v: u8| -> u8 {
                    match v {
                        0 => 0,
                        1 => 95,
                        2 => 135,
                        3 => 175,
                        4 => 215,
                        5 => 255,
                        _ => 0,
                    }
                };

                Self::new(to_rgb(r), to_rgb(g), to_rgb(b))
            }

            // 232-255: Grayscale ramp
            232..=255 => {
                let gray = 8 + (index - 232) * 10;
                Self::new(gray, gray, gray)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let color = Color::new(128, 64, 32);
        assert_eq!(color.r, 128);
        assert_eq!(color.g, 64);
        assert_eq!(color.b, 32);
    }

    #[test]
    fn test_black() {
        let black = Color::black();
        assert_eq!(black.r, 0);
        assert_eq!(black.g, 0);
        assert_eq!(black.b, 0);
    }

    #[test]
    fn test_white() {
        let white = Color::white();
        assert_eq!(white.r, 255);
        assert_eq!(white.g, 255);
        assert_eq!(white.b, 255);
    }

    #[test]
    fn test_from_ansi_index_basic_colors() {
        // Test all 16 ANSI colors
        let black = Color::from_ansi_index(0);
        assert_eq!((black.r, black.g, black.b), (0, 0, 0));

        let red = Color::from_ansi_index(1);
        assert_eq!((red.r, red.g, red.b), (205, 49, 49));

        let green = Color::from_ansi_index(2);
        assert_eq!((green.r, green.g, green.b), (13, 188, 121));

        let yellow = Color::from_ansi_index(3);
        assert_eq!((yellow.r, yellow.g, yellow.b), (229, 229, 16));

        let blue = Color::from_ansi_index(4);
        assert_eq!((blue.r, blue.g, blue.b), (36, 114, 200));

        let magenta = Color::from_ansi_index(5);
        assert_eq!((magenta.r, magenta.g, magenta.b), (188, 63, 188));

        let cyan = Color::from_ansi_index(6);
        assert_eq!((cyan.r, cyan.g, cyan.b), (17, 168, 205));

        let white = Color::from_ansi_index(7);
        assert_eq!((white.r, white.g, white.b), (229, 229, 229));
    }

    #[test]
    fn test_from_ansi_index_bright_colors() {
        let bright_black = Color::from_ansi_index(8);
        assert_eq!(
            (bright_black.r, bright_black.g, bright_black.b),
            (102, 102, 102)
        );

        let bright_red = Color::from_ansi_index(9);
        assert_eq!((bright_red.r, bright_red.g, bright_red.b), (241, 76, 76));

        let bright_green = Color::from_ansi_index(10);
        assert_eq!(
            (bright_green.r, bright_green.g, bright_green.b),
            (35, 209, 139)
        );

        let bright_yellow = Color::from_ansi_index(11);
        assert_eq!(
            (bright_yellow.r, bright_yellow.g, bright_yellow.b),
            (245, 245, 67)
        );

        let bright_blue = Color::from_ansi_index(12);
        assert_eq!(
            (bright_blue.r, bright_blue.g, bright_blue.b),
            (59, 142, 234)
        );

        let bright_magenta = Color::from_ansi_index(13);
        assert_eq!(
            (bright_magenta.r, bright_magenta.g, bright_magenta.b),
            (214, 112, 214)
        );

        let bright_cyan = Color::from_ansi_index(14);
        assert_eq!(
            (bright_cyan.r, bright_cyan.g, bright_cyan.b),
            (41, 184, 219)
        );

        let bright_white = Color::from_ansi_index(15);
        assert_eq!(
            (bright_white.r, bright_white.g, bright_white.b),
            (229, 229, 229)
        );
    }

    #[test]
    fn test_from_ansi_index_rgb_cube_boundaries() {
        // Test RGB cube boundaries (colors 16-231)

        // First color in cube: index 16 = (0,0,0) in 6x6x6 = RGB(0,0,0)
        let color = Color::from_ansi_index(16);
        assert_eq!((color.r, color.g, color.b), (0, 0, 0));

        // index 17 = (0,0,1) in 6x6x6 = RGB(0,0,95)
        let color = Color::from_ansi_index(17);
        assert_eq!((color.r, color.g, color.b), (0, 0, 95));

        // index 21 = (0,0,5) in 6x6x6 = RGB(0,0,255)
        let color = Color::from_ansi_index(21);
        assert_eq!((color.r, color.g, color.b), (0, 0, 255));

        // index 22 = (0,1,0) in 6x6x6 = RGB(0,95,0)
        let color = Color::from_ansi_index(22);
        assert_eq!((color.r, color.g, color.b), (0, 95, 0));

        // index 52 = (1,0,0) in 6x6x6 = RGB(95,0,0)
        let color = Color::from_ansi_index(52);
        assert_eq!((color.r, color.g, color.b), (95, 0, 0));

        // Last color in cube: index 231 = (5,5,5) in 6x6x6 = RGB(255,255,255)
        let color = Color::from_ansi_index(231);
        assert_eq!((color.r, color.g, color.b), (255, 255, 255));
    }

    #[test]
    fn test_from_ansi_index_rgb_cube_mid_values() {
        // Test some middle values in the RGB cube

        // index 196: 196-16=180, r=180/36=5, g=(180/6)%6=0, b=180%6=0 = RGB(255, 0, 0)
        let color = Color::from_ansi_index(196);
        assert_eq!((color.r, color.g, color.b), (255, 0, 0));

        // index 46: 46-16=30, r=30/36=0, g=(30/6)%6=5, b=30%6=0 = RGB(0, 255, 0) - pure green
        let color = Color::from_ansi_index(46);
        assert_eq!((color.r, color.g, color.b), (0, 255, 0));

        // index 226: 226-16=210, r=210/36=5, g=(210/6)%6=5, b=210%6=0 = RGB(255, 255, 0) - pure yellow
        let color = Color::from_ansi_index(226);
        assert_eq!((color.r, color.g, color.b), (255, 255, 0));

        // index 201: 201-16=185, r=185/36=5, g=(185/6)%6=0, b=185%6=5 = RGB(255, 0, 255) - pure magenta
        let color = Color::from_ansi_index(201);
        assert_eq!((color.r, color.g, color.b), (255, 0, 255));

        // index 51: 51-16=35, r=35/36=0, g=(35/6)%6=5, b=35%6=5 = RGB(0, 255, 255) - pure cyan
        let color = Color::from_ansi_index(51);
        assert_eq!((color.r, color.g, color.b), (0, 255, 255));
    }

    #[test]
    fn test_from_ansi_index_grayscale_ramp() {
        // Test grayscale ramp (colors 232-255)

        // First gray: index 232 = 8
        let color = Color::from_ansi_index(232);
        assert_eq!((color.r, color.g, color.b), (8, 8, 8));

        // index 233 = 8 + 1*10 = 18
        let color = Color::from_ansi_index(233);
        assert_eq!((color.r, color.g, color.b), (18, 18, 18));

        // index 244 = 8 + 12*10 = 128 (middle gray)
        let color = Color::from_ansi_index(244);
        assert_eq!((color.r, color.g, color.b), (128, 128, 128));

        // Last gray: index 255 = 8 + 23*10 = 238
        let color = Color::from_ansi_index(255);
        assert_eq!((color.r, color.g, color.b), (238, 238, 238));
    }

    #[test]
    fn test_256_color_coverage() {
        // Verify all 256 colors can be generated without panic
        // RGB values are guaranteed valid by u8 type (0-255)
        for i in 0..=255 {
            let _color = Color::from_ansi_index(i);
        }
    }

    #[test]
    fn test_color_is_copy() {
        // Verify Color implements Copy trait
        let c1 = Color::new(100, 150, 200);
        let c2 = c1; // This should copy, not move
        assert_eq!(c1.r, c2.r); // c1 should still be valid
        assert_eq!(c1.g, c2.g);
        assert_eq!(c1.b, c2.b);
    }

    #[test]
    fn test_color_is_clone() {
        let c1 = Color::new(50, 100, 150);
        let c2 = c1;
        assert_eq!(c1.r, c2.r);
        assert_eq!(c1.g, c2.g);
        assert_eq!(c1.b, c2.b);
    }
}
