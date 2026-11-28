use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Selection {
    rows: BTreeSet<usize>,
    anchor: Option<usize>,
    active: Option<usize>,
}

impl Selection {
    pub fn click(&mut self, visible: &[usize], row: usize, extend: bool, toggle: bool) {
        if extend {
            let end = visible.binary_search(&row).expect("clicked row is visible");
            let start = self
                .anchor
                .or(self.primary())
                .and_then(|anchor| visible.binary_search(&anchor).ok())
                .unwrap_or(end);
            if !toggle {
                self.rows.clear();
            }
            self.rows
                .extend(visible[start.min(end)..=start.max(end)].iter().copied());
            self.anchor = Some(visible[start]);
            self.active = Some(row);
        } else if toggle {
            if !self.rows.remove(&row) {
                self.rows.insert(row);
            }
            self.anchor = Some(row);
            self.active = Some(row);
        } else {
            self.single(row);
        }
    }

    pub fn single(&mut self, row: usize) {
        self.rows.clear();
        self.rows.insert(row);
        self.active = Some(row);
        self.anchor = Some(row);
    }

    pub fn retain_visible(&mut self, visible: &[usize]) {
        self.rows.retain(|row| visible.binary_search(row).is_ok());
        if self
            .anchor
            .is_some_and(|row| visible.binary_search(&row).is_err())
        {
            self.anchor = self.primary();
        }
    }

    pub fn primary(&self) -> Option<usize> {
        self.active
            .filter(|row| self.contains(*row))
            .or_else(|| self.rows.last().copied())
    }

    pub fn contains(&self, row: usize) -> bool {
        self.rows.contains(&row)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + Clone + '_ {
        self.rows.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_selects_the_visible_range_in_either_direction() {
        let visible = [2, 3, 7, 9, 16, 40];
        let mut selection = Selection::default();
        selection.click(&visible, 7, false, false);
        selection.click(&visible, 16, true, false);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [7, 9, 16]);
        selection.click(&visible, 2, true, false);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [2, 3, 7]);
        assert_eq!(selection.primary(), Some(2));
    }

    #[test]
    fn ctrl_toggles_individual_rows_and_combines_with_shift() {
        let visible = [2, 3, 7, 9, 16, 40];
        let mut selection = Selection::default();
        selection.click(&visible, 2, false, false);
        selection.click(&visible, 9, false, true);
        selection.click(&visible, 40, true, true);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [2, 9, 16, 40]);
        selection.click(&visible, 2, false, true);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [9, 16, 40]);
        selection.click(&visible, 9, false, false);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [9]);
        selection.click(&visible, 9, false, true);
        assert_eq!(selection.primary(), None);
    }

    #[test]
    fn filtering_restricts_selection_and_removes_invisible_anchor() {
        let mut selection = Selection::default();
        selection.click(&[1, 3, 5, 7], 3, false, false);
        selection.click(&[1, 3, 5, 7], 7, true, false);
        selection.retain_visible(&[1, 5, 7]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [5, 7]);
        selection.click(&[1, 5, 7], 1, true, false);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [1, 5, 7]);
    }
}
