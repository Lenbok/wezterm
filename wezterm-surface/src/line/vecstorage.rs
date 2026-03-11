use crate::line::cellref::CellRef;
use alloc::sync::Arc;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use wezterm_cell::Cell;

extern crate alloc;
use alloc::vec::Vec;

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VecStorage {
    cells: Vec<Cell>,
}

impl VecStorage {
    pub(crate) fn new(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    #[cfg_attr(not(feature = "use_image"), allow(unused_mut, unused_variables))]
    pub(crate) fn set_cell(&mut self, idx: usize, mut cell: Cell, clear_image_placement: bool) {
        #[cfg(feature = "use_image")]
        if !clear_image_placement {
            // Copy images from the old cell to the new cell.
            // This preserves images when text is written over them (images "float over" text).
            //
            // However, if the NEW cell already has placement_id images attached, don't copy
            // placement_id images from the OLD cell. This handles the case where
            // assign_image_to_cells places a new image and we don't want to duplicate
            // the old image onto the cell.
            let new_cell_has_placement_id = cell
                .attrs()
                .images()
                .map_or(false, |imgs| imgs.iter().any(|i| i.has_placement_id()));

            if let Some(old_images) = self.cells[idx].attrs().images() {
                for image in old_images {
                    // Skip copying placement_id images if new cell already has them
                    if image.has_placement_id() && new_cell_has_placement_id {
                        continue;
                    }

                    // For placement_id images, verify the image belongs at this cell position.
                    // The texture coordinate top_left.x indicates which horizontal slice this is.
                    // If top_left.x > 0, this is not the leftmost slice and should not be at idx=0.
                    // This prevents the "stripe at left edge" bug from cell shift operations.
                    if image.has_placement_id() {
                        let top_left_x: f32 = image.top_left().x.into();
                        if top_left_x > 0.001 && idx == 0 {
                            continue;
                        }
                    }

                    cell.attrs_mut().attach_image(Box::new(image));
                }
            }
        }
        self.cells[idx] = cell;
    }

    pub(crate) fn scan_and_create_hyperlinks(
        &mut self,
        line: &str,
        matches: Vec<crate::hyperlink::RuleMatch>,
    ) -> bool {
        // The capture range is measured in bytes but we need to translate
        // that to the index of the column.  This is complicated a bit further
        // because double wide sequences have a blank column cell after them
        // in the cells array, but the string we match against excludes that
        // string.
        let mut cell_idx = 0;
        let mut has_implicit_hyperlinks = false;
        for (byte_idx, _grapheme) in line.grapheme_indices(true) {
            let cell = &mut self.cells[cell_idx];
            let mut matched = false;
            for m in &matches {
                if m.range.contains(&byte_idx) {
                    let attrs = cell.attrs_mut();
                    // Don't replace existing links
                    if attrs.hyperlink().is_none() {
                        attrs.set_hyperlink(Some(Arc::clone(&m.link)));
                        matched = true;
                    }
                }
            }
            cell_idx += cell.width();
            if matched {
                has_implicit_hyperlinks = true;
            }
        }

        has_implicit_hyperlinks
    }
}

impl core::ops::Deref for VecStorage {
    type Target = Vec<Cell>;

    fn deref(&self) -> &Vec<Cell> {
        &self.cells
    }
}

impl core::ops::DerefMut for VecStorage {
    fn deref_mut(&mut self) -> &mut Vec<Cell> {
        &mut self.cells
    }
}

/// Iterates over a slice of Cell, yielding only visible cells
pub(crate) struct VecStorageIter<'a> {
    pub cells: core::slice::Iter<'a, Cell>,
    pub idx: usize,
    pub skip_width: usize,
}

impl<'a> Iterator for VecStorageIter<'a> {
    type Item = CellRef<'a>;

    fn next(&mut self) -> Option<CellRef<'a>> {
        while self.skip_width > 0 {
            self.skip_width -= 1;
            let _ = self.cells.next()?;
            self.idx += 1;
        }
        let cell = self.cells.next()?;
        let cell_index = self.idx;
        self.idx += 1;
        self.skip_width = cell.width().saturating_sub(1);
        Some(CellRef::CellRef { cell_index, cell })
    }
}
