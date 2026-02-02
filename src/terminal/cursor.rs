//! Cursor representation for terminal emulator

/// Cursor position and style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Row position (0-indexed)
    pub row: usize,

    /// Column position (0-indexed)
    pub col: usize,

    /// Whether cursor is visible
    pub visible: bool,

    /// Cursor display style
    pub style: CursorStyle,
}

/// Cursor display style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// Block cursor (default)
    Block,

    /// Underline cursor
    Underline,

    /// Vertical bar cursor
    Bar,
}

impl Cursor {
    /// Create a new cursor at the given position
    pub fn new(row: usize, col: usize) -> Self {
        Self {
            row,
            col,
            visible: true,
            style: CursorStyle::Block,
        }
    }

    /// Create a cursor at origin (0, 0)
    pub fn at_origin() -> Self {
        Self::new(0, 0)
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::at_origin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new(10, 20);
        assert_eq!(cursor.row, 10);
        assert_eq!(cursor.col, 20);
        assert!(cursor.visible);
        assert_eq!(cursor.style, CursorStyle::Block);
    }

    #[test]
    fn test_cursor_at_origin() {
        let cursor = Cursor::at_origin();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_cursor_default() {
        let cursor = Cursor::default();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
        assert!(cursor.visible);
        assert_eq!(cursor.style, CursorStyle::Block);
    }
}
