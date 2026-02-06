//! ANSI escape sequence types and codes
//!
//! This module provides strongly-typed representations of ANSI/VT escape sequences
//! used for terminal control. These sequences control cursor movement, colors,
//! screen clearing, and other terminal behaviors.

/// Errors that can occur during ANSI escape sequence parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiParseError {
    /// Invalid parameter value for the command
    InvalidParameter { expected: &'static str, got: u16 },

    /// Missing required parameter at specified index
    MissingParameter { index: usize },

    /// Unknown or unimplemented CSI command
    UnknownCommand(char),
}

impl std::fmt::Display for AnsiParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameter { expected, got } => {
                write!(f, "Invalid parameter: expected {}, got {}", expected, got)
            }
            Self::MissingParameter { index } => {
                write!(f, "Missing required parameter at index {}", index)
            }
            Self::UnknownCommand(ch) => {
                write!(f, "Unknown or unimplemented CSI command: '{}'", ch)
            }
        }
    }
}

impl std::error::Error for AnsiParseError {}

/// CSI (Control Sequence Introducer) commands
/// Format: ESC [ <params> <intermediates> <final>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiCommand {
    /// Cursor Position (CUP) - Move cursor to absolute position
    /// ESC[{row};{col}H or ESC[{row};{col}f
    /// Default: row=1, col=1
    CursorPosition { row: u16, col: u16 },

    /// Cursor Up (CUU) - Move cursor up by n rows
    /// ESC[{n}A
    /// Default: n=1
    CursorUp { n: u16 },

    /// Cursor Down (CUD) - Move cursor down by n rows
    /// ESC[{n}B
    /// Default: n=1
    CursorDown { n: u16 },

    /// Cursor Forward (CUF) - Move cursor right by n columns
    /// ESC[{n}C
    /// Default: n=1
    CursorForward { n: u16 },

    /// Cursor Back (CUB) - Move cursor left by n columns
    /// ESC[{n}D
    /// Default: n=1
    CursorBack { n: u16 },

    /// Erase in Display (ED) - Clear parts of the screen
    /// ESC[{n}J
    /// n=0: Clear from cursor to end of screen
    /// n=1: Clear from cursor to beginning of screen
    /// n=2: Clear entire screen
    /// n=3: Clear entire screen and scrollback buffer
    EraseInDisplay { mode: EraseMode },

    /// Erase in Line (EL) - Clear parts of the current line
    /// ESC[{n}K
    /// n=0: Clear from cursor to end of line
    /// n=1: Clear from cursor to beginning of line
    /// n=2: Clear entire line
    EraseInLine { mode: EraseMode },

    /// Select Graphic Rendition (SGR) - Set text attributes and colors
    /// ESC[{n}m
    /// This variant keeps variable parameters - handled separately
    SelectGraphicRendition,

    /// Insert Lines (IL) - Insert n blank lines
    /// ESC[{n}L
    /// Default: n=1
    InsertLines { n: u16 },

    /// Delete Lines (DL) - Delete n lines
    /// ESC[{n}M
    /// Default: n=1
    DeleteLines { n: u16 },

    /// Set Scrolling Region (DECSTBM)
    /// ESC[{top};{bottom}r
    /// Default: top=1, bottom=viewport_height
    SetScrollingRegion { top: u16, bottom: u16 },

    /// Device Status Report (DSR)
    /// ESC[{n}n
    /// Default: n=0
    DeviceStatusReport { n: u16 },

    /// Set Cursor Style (DECSCUSR)
    /// ESC[{n} q
    /// Default: n=0
    SetCursorStyle { style: u16 },

    /// Cursor Horizontal Absolute (CHA)
    /// ESC[{n}G
    /// Default: n=1
    CursorHorizontalAbsolute { col: u16 },

    /// Device Attributes (DA)
    /// ESC[{n}c
    /// Default: n=0
    DeviceAttributes { n: u16 },

    /// Window Manipulation
    /// ESC[{n}t
    /// Default: n=0
    WindowManipulation { n: u16 },

    /// Vertical Position Absolute (VPA)
    /// ESC[{row}d
    /// Default: row=1
    /// Moves cursor to absolute row position, column unchanged
    VerticalPositionAbsolute { row: u16 },

    /// Erase Character (ECH)
    /// ESC[{n}X
    /// Default: n=1
    /// Erases n characters at cursor by replacing with spaces
    EraseCharacter { n: u16 },

    /// Scroll Down (SD)
    /// ESC[{n}T
    /// Default: n=1
    /// Scrolls viewport content down by n lines
    ScrollDown { n: u16 },

    /// Scroll Up (SU)
    /// ESC[{n}S
    /// Default: n=1
    /// Scrolls viewport content up by n lines
    ScrollUp { n: u16 },

    /// Delete Character (DCH)
    /// ESC[{n}P
    /// Default: n=1
    /// Deletes n characters at cursor, shifting remaining chars left
    DeleteCharacter { n: u16 },

    /// Reset Mode (RM)
    /// ESC[{mode}l
    /// Default: mode=0
    /// Resets terminal mode (no-op currently)
    ResetMode { mode: u16 },

    /// Unknown or unimplemented CSI command
    Unknown(char),
}

impl CsiCommand {
    /// Helper to extract parameter from VTE Params with default value
    ///
    /// In CSI sequences, a parameter of 0 usually means "use default",
    /// so we treat 0 the same as missing parameters.
    #[inline]
    fn param_or(params: &vte::Params, index: usize, default: u16) -> u16 {
        params
            .iter()
            .nth(index)
            .and_then(|p| p.first())
            .copied()
            .filter(|&v| v != 0) // Treat 0 as "use default"
            .unwrap_or(default)
    }

    /// Parse CSI sequence into command with parameters
    ///
    /// This method extracts parameters from the VTE Params and constructs
    /// the appropriate CsiCommand variant with all parameters resolved.
    ///
    /// # Arguments
    /// * `final_byte` - The final character of the CSI sequence
    /// * `params` - Parameter list from VTE parser
    /// * `is_dec_private` - Whether this is a DEC private mode sequence (started with '?')
    ///
    /// # Returns
    /// Result containing the parsed command or an error
    pub fn parse(
        final_byte: char,
        params: &vte::Params,
        is_dec_private: bool,
    ) -> Result<Self, AnsiParseError> {
        if is_dec_private {
            // DEC private mode sequences use different meanings
            return Err(AnsiParseError::UnknownCommand(final_byte));
        }

        match final_byte {
            'H' | 'f' => {
                let row = Self::param_or(params, 0, 1);
                let col = Self::param_or(params, 1, 1);
                Ok(Self::CursorPosition { row, col })
            }
            'A' => Ok(Self::CursorUp {
                n: Self::param_or(params, 0, 1),
            }),
            'B' => Ok(Self::CursorDown {
                n: Self::param_or(params, 0, 1),
            }),
            'C' => Ok(Self::CursorForward {
                n: Self::param_or(params, 0, 1),
            }),
            'D' => Ok(Self::CursorBack {
                n: Self::param_or(params, 0, 1),
            }),
            'J' => {
                let mode_param = Self::param_or(params, 0, 0);
                let mode = EraseMode::from_param(mode_param);
                Ok(Self::EraseInDisplay { mode })
            }
            'K' => {
                let mode_param = Self::param_or(params, 0, 0);
                let mode = EraseMode::from_param(mode_param);
                Ok(Self::EraseInLine { mode })
            }
            'm' => Ok(Self::SelectGraphicRendition),
            'L' => Ok(Self::InsertLines {
                n: Self::param_or(params, 0, 1),
            }),
            'M' => Ok(Self::DeleteLines {
                n: Self::param_or(params, 0, 1),
            }),
            'r' => {
                let top = Self::param_or(params, 0, 1);
                let bottom = Self::param_or(params, 1, 0);
                Ok(Self::SetScrollingRegion { top, bottom })
            }
            'n' => Ok(Self::DeviceStatusReport {
                n: Self::param_or(params, 0, 0),
            }),
            'q' => Ok(Self::SetCursorStyle {
                style: Self::param_or(params, 0, 0),
            }),
            'G' => Ok(Self::CursorHorizontalAbsolute {
                col: Self::param_or(params, 0, 1),
            }),
            'c' => Ok(Self::DeviceAttributes {
                n: Self::param_or(params, 0, 0),
            }),
            't' => Ok(Self::WindowManipulation {
                n: Self::param_or(params, 0, 0),
            }),
            'd' => Ok(Self::VerticalPositionAbsolute {
                row: Self::param_or(params, 0, 1),
            }),
            'X' => Ok(Self::EraseCharacter {
                n: Self::param_or(params, 0, 1),
            }),
            'T' => Ok(Self::ScrollDown {
                n: Self::param_or(params, 0, 1),
            }),
            'S' => Ok(Self::ScrollUp {
                n: Self::param_or(params, 0, 1),
            }),
            'P' => Ok(Self::DeleteCharacter {
                n: Self::param_or(params, 0, 1),
            }),
            'l' => Ok(Self::ResetMode {
                mode: Self::param_or(params, 0, 0),
            }),
            _ => Err(AnsiParseError::UnknownCommand(final_byte)),
        }
    }
}

/// DEC Private Mode sequences
/// Format: ESC [ ? {mode} h   (set)
///         ESC [ ? {mode} l   (reset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecPrivateMode {
    /// Application Cursor Keys (DECCKM)
    /// Mode 1
    ApplicationCursorKeys,

    /// Designate USASCII for character sets G0-G3 (DECANM)
    /// Mode 2
    DesignateUSASCII,

    /// 132 Column Mode (DECCOLM)
    /// Mode 3
    ColumnMode132,

    /// Smooth (Slow) Scroll (DECSCLM)
    /// Mode 4
    SmoothScroll,

    /// Reverse Video (DECSCNM)
    /// Mode 5
    ReverseVideo,

    /// Origin Mode (DECOM)
    /// Mode 6
    OriginMode,

    /// Auto-wrap Mode (DECAWM)
    /// Mode 7
    AutoWrapMode,

    /// Auto-repeat Keys (DECARM)
    /// Mode 8
    AutoRepeatKeys,

    /// Send Mouse X & Y on button press (X10 mouse protocol)
    /// Mode 9
    MouseX10,

    /// Start Blinking Cursor (AT&T 610)
    /// Mode 12
    CursorBlink,

    /// Show cursor (DECTCEM)
    /// Mode 25
    ShowCursor,

    /// Mouse tracking (X11 mouse protocol)
    /// Mode 1000
    MouseTracking,

    /// Use Hilite Mouse Tracking
    /// Mode 1001
    MouseHiliteTracking,

    /// Use Cell Motion Mouse Tracking
    /// Mode 1002
    MouseCellMotion,

    /// Use All Motion Mouse Tracking
    /// Mode 1003
    MouseAllMotion,

    /// Send FocusIn/FocusOut events
    /// Mode 1004
    FocusEvents,

    /// Enable UTF-8 Mouse Mode
    /// Mode 1005
    MouseUTF8,

    /// Enable SGR Mouse Mode
    /// Mode 1006
    MouseSGR,

    /// Enable Alternate Scroll Mode
    /// Mode 1007
    AlternateScroll,

    /// Enable urxvt Mouse Mode
    /// Mode 1015
    MouseUrxvt,

    /// Alternate Screen Buffer
    /// Mode 1049 (save cursor + switch to alternate screen)
    /// Mode 47 (just switch, no cursor save)
    AlternateScreenBuffer,

    /// Bracketed Paste Mode
    /// Mode 2004
    BracketedPaste,

    /// Synchronized Output Mode
    /// Mode 2026
    SynchronizedOutput,

    /// Unknown or unimplemented mode
    Unknown(u16),
}

impl DecPrivateMode {
    /// Parse mode number into DecPrivateMode
    pub fn from_mode(mode: u16) -> Self {
        match mode {
            1 => Self::ApplicationCursorKeys,
            2 => Self::DesignateUSASCII,
            3 => Self::ColumnMode132,
            4 => Self::SmoothScroll,
            5 => Self::ReverseVideo,
            6 => Self::OriginMode,
            7 => Self::AutoWrapMode,
            8 => Self::AutoRepeatKeys,
            9 => Self::MouseX10,
            12 => Self::CursorBlink,
            25 => Self::ShowCursor,
            47 => Self::AlternateScreenBuffer,
            1000 => Self::MouseTracking,
            1001 => Self::MouseHiliteTracking,
            1002 => Self::MouseCellMotion,
            1003 => Self::MouseAllMotion,
            1004 => Self::FocusEvents,
            1005 => Self::MouseUTF8,
            1006 => Self::MouseSGR,
            1007 => Self::AlternateScroll,
            1015 => Self::MouseUrxvt,
            1049 => Self::AlternateScreenBuffer,
            2004 => Self::BracketedPaste,
            2026 => Self::SynchronizedOutput,
            _ => Self::Unknown(mode),
        }
    }
}

/// SGR (Select Graphic Rendition) parameters for text styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrParameter {
    /// Reset all attributes to default
    Reset,

    /// Bold or increased intensity
    Bold,

    /// Faint or decreased intensity
    Faint,

    /// Italic
    Italic,

    /// Underline
    Underline,

    /// Slow blink
    SlowBlink,

    /// Rapid blink
    RapidBlink,

    /// Reverse video (swap foreground and background)
    ReverseVideo,

    /// Conceal (hide text)
    Conceal,

    /// Crossed-out (strikethrough)
    CrossedOut,

    /// Normal intensity (not bold or faint)
    NormalIntensity,

    /// Not italic
    NotItalic,

    /// Not underlined
    NotUnderlined,

    /// Not blinking
    NotBlinking,

    /// Not reversed
    NotReversed,

    /// Not concealed
    NotConcealed,

    /// Not crossed out
    NotCrossedOut,

    /// Set foreground color (basic 8 colors)
    /// Colors 30-37: black, red, green, yellow, blue, magenta, cyan, white
    ForegroundColor(u8),

    /// Set background color (basic 8 colors)
    /// Colors 40-47: black, red, green, yellow, blue, magenta, cyan, white
    BackgroundColor(u8),

    /// Extended foreground color
    /// Next parameters specify the color (256-color or RGB)
    ExtendedForeground,

    /// Default foreground color
    DefaultForeground,

    /// Extended background color
    /// Next parameters specify the color (256-color or RGB)
    ExtendedBackground,

    /// Default background color
    DefaultBackground,

    /// Extended underline color
    /// Next parameters specify the underline color (256-color or RGB)
    ExtendedUnderlineColor,

    /// Default underline color
    DefaultUnderlineColor,

    /// Set foreground color (bright/bold colors)
    /// Colors 90-97: bright versions of 30-37
    BrightForegroundColor(u8),

    /// Set background color (bright/bold colors)
    /// Colors 100-107: bright versions of 40-47
    BrightBackgroundColor(u8),

    /// Unknown parameter
    Unknown(u16),
}

impl SgrParameter {
    /// Parse SGR code into parameter
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => Self::Reset,
            1 => Self::Bold,
            2 => Self::Faint,
            3 => Self::Italic,
            4 => Self::Underline,
            5 => Self::SlowBlink,
            6 => Self::RapidBlink,
            7 => Self::ReverseVideo,
            8 => Self::Conceal,
            9 => Self::CrossedOut,
            22 => Self::NormalIntensity,
            23 => Self::NotItalic,
            24 => Self::NotUnderlined,
            25 => Self::NotBlinking,
            27 => Self::NotReversed,
            28 => Self::NotConcealed,
            29 => Self::NotCrossedOut,
            30..=37 => Self::ForegroundColor((code - 30) as u8),
            38 => Self::ExtendedForeground,
            39 => Self::DefaultForeground,
            40..=47 => Self::BackgroundColor((code - 40) as u8),
            48 => Self::ExtendedBackground,
            49 => Self::DefaultBackground,
            58 => Self::ExtendedUnderlineColor,
            59 => Self::DefaultUnderlineColor,
            90..=97 => Self::BrightForegroundColor((code - 90) as u8),
            100..=107 => Self::BrightBackgroundColor((code - 100) as u8),
            _ => Self::Unknown(code),
        }
    }
}

/// Erase mode for EraseInDisplay command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseMode {
    /// Clear from cursor to end
    ToEnd,

    /// Clear from cursor to beginning
    ToBeginning,

    /// Clear entire region
    All,

    /// Clear entire region including scrollback (only for EraseInDisplay)
    AllWithScrollback,
}

impl EraseMode {
    /// Parse erase mode parameter
    pub fn from_param(param: u16) -> Self {
        match param {
            0 => Self::ToEnd,
            1 => Self::ToBeginning,
            2 => Self::All,
            3 => Self::AllWithScrollback,
            _ => Self::ToEnd, // Default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dec_private_mode_from_mode() {
        assert_eq!(
            DecPrivateMode::from_mode(1),
            DecPrivateMode::ApplicationCursorKeys
        );
        assert_eq!(DecPrivateMode::from_mode(25), DecPrivateMode::ShowCursor);
        assert_eq!(
            DecPrivateMode::from_mode(1049),
            DecPrivateMode::AlternateScreenBuffer
        );
        assert_eq!(
            DecPrivateMode::from_mode(47),
            DecPrivateMode::AlternateScreenBuffer
        );

        match DecPrivateMode::from_mode(9999) {
            DecPrivateMode::Unknown(9999) => {}
            _ => panic!("Expected Unknown(9999)"),
        }
    }

    #[test]
    fn test_sgr_parameter_from_code() {
        assert_eq!(SgrParameter::from_code(0), SgrParameter::Reset);
        assert_eq!(SgrParameter::from_code(1), SgrParameter::Bold);
        assert_eq!(SgrParameter::from_code(3), SgrParameter::Italic);
        assert_eq!(SgrParameter::from_code(4), SgrParameter::Underline);
        assert_eq!(SgrParameter::from_code(7), SgrParameter::ReverseVideo);
        assert_eq!(
            SgrParameter::from_code(38),
            SgrParameter::ExtendedForeground
        );
        assert_eq!(
            SgrParameter::from_code(48),
            SgrParameter::ExtendedBackground
        );

        match SgrParameter::from_code(30) {
            SgrParameter::ForegroundColor(0) => {}
            _ => panic!("Expected ForegroundColor(0)"),
        }

        match SgrParameter::from_code(91) {
            SgrParameter::BrightForegroundColor(1) => {}
            _ => panic!("Expected BrightForegroundColor(1)"),
        }
    }

    #[test]
    fn test_erase_mode_from_param() {
        assert_eq!(EraseMode::from_param(0), EraseMode::ToEnd);
        assert_eq!(EraseMode::from_param(1), EraseMode::ToBeginning);
        assert_eq!(EraseMode::from_param(2), EraseMode::All);
        assert_eq!(EraseMode::from_param(3), EraseMode::AllWithScrollback);
        assert_eq!(EraseMode::from_param(99), EraseMode::ToEnd); // Default
    }

    #[test]
    fn test_ansi_parse_error_invalid_parameter() {
        let err = AnsiParseError::InvalidParameter {
            expected: "0-3",
            got: 99,
        };

        match err {
            AnsiParseError::InvalidParameter { expected, got } => {
                assert_eq!(expected, "0-3");
                assert_eq!(got, 99);
            }
            _ => panic!("Expected InvalidParameter variant"),
        }
    }

    #[test]
    fn test_ansi_parse_error_missing_parameter() {
        let err = AnsiParseError::MissingParameter { index: 2 };

        match err {
            AnsiParseError::MissingParameter { index } => {
                assert_eq!(index, 2);
            }
            _ => panic!("Expected MissingParameter variant"),
        }
    }

    #[test]
    fn test_ansi_parse_error_unknown_command() {
        let err = AnsiParseError::UnknownCommand('Z');

        match err {
            AnsiParseError::UnknownCommand(ch) => {
                assert_eq!(ch, 'Z');
            }
            _ => panic!("Expected UnknownCommand variant"),
        }
    }

    #[test]
    fn test_ansi_parse_error_equality() {
        let err1 = AnsiParseError::UnknownCommand('X');
        let err2 = AnsiParseError::UnknownCommand('X');
        let err3 = AnsiParseError::UnknownCommand('Y');

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_ansi_parse_error_debug() {
        let err = AnsiParseError::InvalidParameter {
            expected: "0-255",
            got: 300,
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidParameter"));
        assert!(debug_str.contains("0-255"));
        assert!(debug_str.contains("300"));
    }

    // Note: CsiCommand::parse() is tested indirectly through integration tests
    // in parser.rs since creating vte::Params directly requires internal VTE APIs.
    // The existing parser tests verify correct parameter extraction and command parsing.
}
