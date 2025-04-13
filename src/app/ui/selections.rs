use std::cmp::Ordering;

#[derive(Debug, Clone, Copy)]
pub struct Index {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Default)]
pub struct Selection {
    start: Option<Index>,
    end: Option<Index>,
}

impl Selection {
    pub fn set_start(&mut self, row: usize, col: usize) {
        self.start = Some(Index { row, col });
    }

    pub fn set_end(&mut self, row: usize, col: usize) {
        self.end = Some(Index { row, col });
    }

    pub fn start(&self) -> Option<Index> {
        self.start
    }

    pub fn end(&self) -> Option<Index> {
        self.end
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }

    pub fn is_empty(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }

    fn is_reverse(&self) -> bool {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                start.row > end.row || start.row == end.row && start.col > end.col
            }
            _ => false,
        }
    }

    pub fn get_bounds(&self) -> Option<(Index, Index)> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                if self.is_reverse() {
                    return Some((end, start));
                }
                Some((start, end))
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn contains(&self, pos: Index) -> bool {
        if let Some((start, end)) = self.get_bounds() {
            let (st_row, st_col) = (start.row, start.col);
            let (en_row, en_col) = (end.row, end.col);

            return match (pos.row, pos.col) {
                (line, _) if line > st_row && line < en_row => true,
                (line, column) if line > st_row && line == en_row => column <= en_col,
                (line, column) if line == st_row && line < en_row => column >= st_col,
                (line, column) if line == st_row && line == en_row => {
                    column <= en_col && column >= st_col
                }
                _ => false,
            };
        }
        false
    }

    #[must_use]
    pub fn contains_row(&self, row_index: usize) -> bool {
        if let Some((start, end)) = self.get_bounds() {
            return row_index >= start.row && row_index <= end.row;
        }
        false
    }

    /// Returns the start and end column of the selection in the given row.
    /// If the selection does not intersect with the row, the function returns None.
    #[must_use]
    pub fn get_selected_columns_in_row(
        &self,
        row_index: usize,
        row_len: usize,
    ) -> Option<(usize, usize)> {
        if let Some((start, end)) = self.get_bounds() {
            let start_col = match start.row.cmp(&row_index) {
                Ordering::Less => 0,
                Ordering::Greater => return None,
                Ordering::Equal => start.col.min(row_len),
            };

            let end_col = match end.row.cmp(&row_index) {
                Ordering::Less => return None,
                Ordering::Greater => row_len,
                Ordering::Equal => end.col.min(row_len),
            };
            return Some((start_col, end_col));
        }
        None
    }
}
