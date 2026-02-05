use super::color::Color;

/// Terminal cell with character, colors, and text attributes
/// Note: bold is rendered (brightens color), italic is rendered (cyan tint), underline is rendered (line below text)
#[derive(Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl Cell {
    /// Create a new cell with default attributes (used in tests)
    pub fn new(ch: char, fg: Color, bg: Color) -> Self {
        Self {
            ch,
            fg,
            bg,
            bold: false,
            italic: false,
            underline: false,
            reverse: false,
        }
    }

    pub fn with_attributes(
        ch: char,
        fg: Color,
        bg: Color,
        bold: bool,
        italic: bool,
        underline: bool,
        reverse: bool,
    ) -> Self {
        Self {
            ch,
            fg,
            bg,
            bold,
            italic,
            underline,
            reverse,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::white(),
            bg: Color::black(),
            bold: false,
            italic: false,
            underline: false,
            reverse: false,
        }
    }
}

pub struct TerminalGrid {
    pub width: usize,
    pub cells: Vec<Vec<Cell>>,
    pub viewport_height: usize,
    pub viewport_start: usize,
    pub max_scrollback: usize,
    // Alternate screen buffer support
    alternate_cells: Vec<Vec<Cell>>,
    alternate_viewport_start: usize,
    pub use_alternate_screen: bool,
    // Scrolling region support (DECSTBM)
    pub scroll_top: usize,    // Top margin (0-indexed, inclusive)
    pub scroll_bottom: usize, // Bottom margin (0-indexed, inclusive)
}

impl TerminalGrid {
    pub fn new(width: usize, viewport_height: usize) -> Self {
        Self {
            width,
            viewport_height,
            cells: vec![vec![Cell::default(); width]; viewport_height],
            viewport_start: 0,
            max_scrollback: 10000,
            alternate_cells: vec![vec![Cell::default(); width]; viewport_height],
            alternate_viewport_start: 0,
            use_alternate_screen: false,
            scroll_top: 0,
            scroll_bottom: viewport_height.saturating_sub(1),
        }
    }

    pub fn put_cell(&mut self, cell: Cell, row: usize, col: usize) {
        while row >= self.cells.len() {
            self.cells.push(vec![Cell::default(); self.width]);
        }

        if col < self.width {
            self.cells[row][col] = cell;
        }

        if self.cells.len() > self.max_scrollback {
            let excess = self.cells.len() - self.max_scrollback;
            self.cells.drain(0..excess);
            self.viewport_start = self.viewport_start.saturating_sub(excess);
        }
    }

    pub fn clear_viewport(&mut self) {
        let end = (self.viewport_start + self.viewport_height).min(self.cells.len());
        for row in self.viewport_start..end {
            for cell in &mut self.cells[row] {
                *cell = Cell::default();
            }
        }
    }

    pub fn clear_line(&mut self, row: usize) {
        if row < self.cells.len() {
            for cell in &mut self.cells[row] {
                *cell = Cell::default();
            }
        }
    }

    pub fn viewport_to_end(&mut self) {
        if self.cells.len() > self.viewport_height {
            self.viewport_start = self.cells.len() - self.viewport_height;
        } else {
            self.viewport_start = 0;
        }
    }

    pub fn get_viewport(&self) -> &[Vec<Cell>] {
        let start = self.viewport_start;
        let end = (start + self.viewport_height).min(self.cells.len());
        &self.cells[start..end]
    }

    /// Switch to the alternate screen buffer
    pub fn use_alternate_screen(&mut self) {
        if !self.use_alternate_screen {
            // Swap main and alternate buffers
            std::mem::swap(&mut self.cells, &mut self.alternate_cells);
            std::mem::swap(&mut self.viewport_start, &mut self.alternate_viewport_start);
            self.use_alternate_screen = true;
        }
    }

    /// Switch back to the main screen buffer
    pub fn use_main_screen(&mut self) {
        if self.use_alternate_screen {
            // Swap back
            std::mem::swap(&mut self.cells, &mut self.alternate_cells);
            std::mem::swap(&mut self.viewport_start, &mut self.alternate_viewport_start);
            self.use_alternate_screen = false;
        }
    }

    pub fn resize(&mut self, new_width: usize, new_viewport_height: usize) {
        // Update viewport height
        self.viewport_height = new_viewport_height;

        // If width changed, resize all existing rows in BOTH buffers
        if new_width != self.width {
            for row in &mut self.cells {
                row.resize(new_width, Cell::default());
            }
            for row in &mut self.alternate_cells {
                row.resize(new_width, Cell::default());
            }
            self.width = new_width;
        }

        // Ensure we have at least viewport_height rows in BOTH buffers
        while self.cells.len() < self.viewport_height {
            self.cells.push(vec![Cell::default(); self.width]);
        }
        while self.alternate_cells.len() < self.viewport_height {
            self.alternate_cells.push(vec![Cell::default(); self.width]);
        }

        // Adjust viewport to stay in bounds
        self.viewport_to_end();

        // Reset scrolling region to full screen on resize
        self.scroll_top = 0;
        self.scroll_bottom = self.viewport_height.saturating_sub(1);
    }

    /// Set scrolling region margins (DECSTBM)
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        // Validate margins (0-indexed, inclusive)
        if top < bottom && bottom < self.viewport_height {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
        }
        // If invalid, ignore the command (keep current margins)
    }

    /// Reset scrolling region to full screen
    pub fn reset_scroll_region(&mut self) {
        self.scroll_top = 0;
        self.scroll_bottom = self.viewport_height.saturating_sub(1);
    }

    /// Insert n blank lines at the given row within scrolling region
    /// Lines below are pushed down, lines pushed past bottom margin are deleted
    pub fn insert_lines(&mut self, row: usize, count: usize) {
        // Only operate within scrolling region
        if row < self.scroll_top || row > self.scroll_bottom {
            return;
        }

        let count = count.min(self.scroll_bottom - row + 1);

        // Calculate absolute row index
        let abs_row = self.viewport_start + row;

        // Delete lines at the bottom of the scrolling region
        for _ in 0..count {
            if abs_row + self.scroll_bottom - row < self.cells.len() {
                self.cells.remove(abs_row + self.scroll_bottom - row);
            }
        }

        // Insert blank lines at cursor position
        for _ in 0..count {
            self.cells
                .insert(abs_row, vec![Cell::default(); self.width]);
        }
    }

    /// Delete n lines at the given row within scrolling region
    /// Lines below are pulled up, blank lines are added at bottom margin
    pub fn delete_lines(&mut self, row: usize, count: usize) {
        // Only operate within scrolling region
        if row < self.scroll_top || row > self.scroll_bottom {
            return;
        }

        let count = count.min(self.scroll_bottom - row + 1);

        // Calculate absolute row index
        let abs_row = self.viewport_start + row;

        // Delete lines at cursor position
        for _ in 0..count {
            if abs_row < self.cells.len() && abs_row + self.scroll_bottom - row < self.cells.len() {
                self.cells.remove(abs_row);
            }
        }

        // Insert blank lines at bottom of scrolling region
        for _ in 0..count {
            let insert_pos = abs_row + self.scroll_bottom - row;
            if insert_pos <= self.cells.len() {
                self.cells
                    .insert(insert_pos, vec![Cell::default(); self.width]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_new() {
        let fg = Color::new(255, 0, 0);
        let bg = Color::new(0, 255, 0);
        let cell = Cell::new('A', fg, bg);

        assert_eq!(cell.ch, 'A');
        assert_eq!(cell.fg.r, 255);
        assert_eq!(cell.fg.g, 0);
        assert_eq!(cell.fg.b, 0);
        assert_eq!(cell.bg.r, 0);
        assert_eq!(cell.bg.g, 255);
        assert_eq!(cell.bg.b, 0);
    }

    #[test]
    fn test_cell_default() {
        let cell = Cell::default();

        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg.r, 255);
        assert_eq!(cell.fg.g, 255);
        assert_eq!(cell.fg.b, 255);
        assert_eq!(cell.bg.r, 0);
        assert_eq!(cell.bg.g, 0);
        assert_eq!(cell.bg.b, 0);
    }

    #[test]
    fn test_cell_is_copy() {
        let cell1 = Cell::new('X', Color::white(), Color::black());
        let cell2 = cell1; // Should copy, not move
        assert_eq!(cell1.ch, cell2.ch); // cell1 should still be valid
    }

    #[test]
    fn test_terminal_grid_new() {
        let grid = TerminalGrid::new(80, 24);

        assert_eq!(grid.width, 80);
        assert_eq!(grid.viewport_height, 24);
        assert_eq!(grid.viewport_start, 0);
        assert_eq!(grid.max_scrollback, 10000);
        assert_eq!(grid.cells.len(), 24);
        assert_eq!(grid.cells[0].len(), 80);
    }

    #[test]
    fn test_put_cell_basic() {
        let mut grid = TerminalGrid::new(80, 24);
        let cell = Cell::new('A', Color::white(), Color::black());

        grid.put_cell(cell, 5, 10);

        assert_eq!(grid.cells[5][10].ch, 'A');
    }

    #[test]
    fn test_put_cell_expands_rows() {
        let mut grid = TerminalGrid::new(80, 24);
        let cell = Cell::new('B', Color::white(), Color::black());

        // Put cell beyond current row count
        grid.put_cell(cell, 30, 10);

        assert!(grid.cells.len() >= 31);
        assert_eq!(grid.cells[30][10].ch, 'B');
    }

    #[test]
    fn test_put_cell_respects_scrollback_limit() {
        let mut grid = TerminalGrid::new(80, 24);
        grid.max_scrollback = 100;

        // Add more than max_scrollback rows
        for i in 0..150 {
            grid.put_cell(Cell::new('X', Color::white(), Color::black()), i, 0);
        }

        assert_eq!(grid.cells.len(), 100);
    }

    #[test]
    fn test_put_cell_ignores_out_of_bounds_column() {
        let mut grid = TerminalGrid::new(80, 24);
        let cell = Cell::new('C', Color::white(), Color::black());

        grid.put_cell(cell, 0, 100); // Column 100 is out of bounds

        // Should not panic, just ignore
        assert_eq!(grid.cells[0][0].ch, ' '); // First cell should still be default
    }

    #[test]
    fn test_clear_viewport() {
        let mut grid = TerminalGrid::new(80, 24);

        // Fill grid with characters
        for row in 0..24 {
            for col in 0..80 {
                grid.put_cell(Cell::new('X', Color::white(), Color::black()), row, col);
            }
        }

        grid.clear_viewport();

        // All visible cells should be cleared
        for row in 0..24 {
            for col in 0..80 {
                assert_eq!(grid.cells[row][col].ch, ' ');
            }
        }
    }

    #[test]
    fn test_clear_line() {
        let mut grid = TerminalGrid::new(80, 24);

        // Fill a specific row
        for col in 0..80 {
            grid.put_cell(Cell::new('Y', Color::white(), Color::black()), 10, col);
        }

        grid.clear_line(10);

        // Row 10 should be cleared
        for col in 0..80 {
            assert_eq!(grid.cells[10][col].ch, ' ');
        }

        // Other rows should be unaffected (still empty in this case)
        assert_eq!(grid.cells[9][0].ch, ' ');
    }

    #[test]
    fn test_viewport_to_end() {
        let mut grid = TerminalGrid::new(80, 24);
        grid.viewport_start = 0;

        // Add 50 rows
        for i in 0..50 {
            grid.put_cell(Cell::new('Z', Color::white(), Color::black()), i, 0);
        }

        grid.viewport_to_end();

        // Viewport should now start at row 26 (50 - 24)
        assert_eq!(grid.viewport_start, 26);
    }

    #[test]
    fn test_viewport_to_end_insufficient_rows() {
        let mut grid = TerminalGrid::new(80, 24);

        // Only 10 rows (less than viewport_height)
        for i in 0..10 {
            grid.put_cell(Cell::new('A', Color::white(), Color::black()), i, 0);
        }

        grid.viewport_to_end();

        // Viewport should start at 0
        assert_eq!(grid.viewport_start, 0);
    }

    #[test]
    fn test_get_viewport() {
        let mut grid = TerminalGrid::new(80, 24);

        // Add 50 rows
        for i in 0..50 {
            for j in 0..80 {
                grid.put_cell(
                    Cell::new(
                        ((i % 10) as u8 + b'0') as char,
                        Color::white(),
                        Color::black(),
                    ),
                    i,
                    j,
                );
            }
        }

        grid.viewport_start = 10;
        let viewport = grid.get_viewport();

        assert_eq!(viewport.len(), 24);
        assert_eq!(viewport[0][0].ch, '0'); // Row 10 has '0' (10 % 10)
        assert_eq!(viewport[5][0].ch, '5'); // Row 15 has '5' (15 % 10)
    }

    #[test]
    fn test_resize_width_increase() {
        let mut grid = TerminalGrid::new(80, 24);

        // Fill with data
        grid.put_cell(Cell::new('A', Color::white(), Color::black()), 0, 0);

        grid.resize(100, 24);

        assert_eq!(grid.width, 100);
        assert_eq!(grid.cells[0].len(), 100);
        // Original data should be preserved
        assert_eq!(grid.cells[0][0].ch, 'A');
        // New cells should be default
        assert_eq!(grid.cells[0][99].ch, ' ');
    }

    #[test]
    fn test_resize_width_decrease() {
        let mut grid = TerminalGrid::new(80, 24);

        // Fill with data
        grid.put_cell(Cell::new('B', Color::white(), Color::black()), 0, 0);
        grid.put_cell(Cell::new('X', Color::white(), Color::black()), 0, 70);

        grid.resize(60, 24);

        assert_eq!(grid.width, 60);
        assert_eq!(grid.cells[0].len(), 60);
        // Data within new width should be preserved
        assert_eq!(grid.cells[0][0].ch, 'B');
        // Data beyond new width is truncated (can't verify, but row length is correct)
    }

    #[test]
    fn test_resize_height_increase() {
        let mut grid = TerminalGrid::new(80, 24);

        grid.resize(80, 30);

        assert_eq!(grid.viewport_height, 30);
        assert!(grid.cells.len() >= 30);
    }

    #[test]
    fn test_resize_height_decrease() {
        let mut grid = TerminalGrid::new(80, 24);

        // Add many rows
        for i in 0..50 {
            grid.put_cell(Cell::new('C', Color::white(), Color::black()), i, 0);
        }

        grid.resize(80, 20);

        assert_eq!(grid.viewport_height, 20);
        // Viewport should adjust to end
        assert_eq!(grid.viewport_start, 30); // 50 - 20
    }

    #[test]
    fn test_resize_preserves_content() {
        let mut grid = TerminalGrid::new(80, 24);

        // Create a pattern
        for i in 0..24 {
            grid.put_cell(
                Cell::new((i as u8 + b'A') as char, Color::white(), Color::black()),
                i,
                0,
            );
        }

        grid.resize(100, 30);

        // Original content should be preserved
        for i in 0..24 {
            assert_eq!(grid.cells[i][0].ch, (i as u8 + b'A') as char);
        }
    }
}
