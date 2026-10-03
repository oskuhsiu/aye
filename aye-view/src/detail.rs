//! Detail source ranges shared by wrapping, mouse selection and rendering.
use ratatui::{
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub(crate) struct Detail {
    text: String,
    rows: Vec<Range<usize>>,
    area: Rect,
    scroll: usize,
    selection: Option<Selection>,
}

struct Selection {
    origin: Position,
    anchor: Range<usize>,
    head: Range<usize>,
    moved: bool,
    dragging: bool,
}

/// Soft wraps retain source offsets, so copying never invents newlines.
pub(crate) fn wrapped_ranges(text: &str, width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut offset = 0;
    for line in text.split('\n') {
        let mut start = offset;
        let mut used = 0;
        for (index, grapheme) in line.grapheme_indices(true) {
            let cells = Span::raw(grapheme).width();
            if used + cells > width && offset + index > start {
                rows.push(start..offset + index);
                start = offset + index;
                used = 0;
            }
            used += cells;
        }
        rows.push(start..offset + line.len());
        offset += line.len() + 1;
    }
    rows
}

impl Detail {
    pub(crate) fn layout(&mut self, text: String, area: Rect, scroll: usize) -> (usize, usize) {
        let rows = wrapped_ranges(&text, usize::from(area.width));
        let max_scroll = rows.len().saturating_sub(usize::from(area.height));
        let scroll = scroll.min(max_scroll);
        if self.text != text || self.area != area || self.scroll != scroll {
            self.clear();
        }
        self.text = text;
        self.rows = rows;
        self.area = area;
        self.scroll = scroll;
        (scroll, max_scroll)
    }

    pub(crate) fn clear(&mut self) -> bool {
        self.selection.take().is_some()
    }

    pub(crate) fn dragging(&self) -> bool {
        self.selection.as_ref().is_some_and(|s| s.dragging)
    }

    pub(crate) fn stop_drag(&mut self) {
        if let Some(selection) = &mut self.selection {
            selection.dragging = false;
        }
    }

    fn at(&self, point: Position) -> Option<Range<usize>> {
        if self.area.is_empty() {
            return None;
        }
        let row = point.y.clamp(self.area.y, self.area.bottom() - 1) - self.area.y;
        let Some(range) = self.rows.get(self.scroll + usize::from(row)) else {
            return Some(self.text.len()..self.text.len());
        };
        let column = usize::from(point.x.clamp(self.area.x, self.area.right() - 1) - self.area.x);
        let mut used = 0;
        for (index, grapheme) in self.text[range.clone()].grapheme_indices(true) {
            let cells = Span::raw(grapheme).width();
            // A grapheme too wide for this row is not painted by Paragraph.
            if used + cells > usize::from(self.area.width) {
                return None;
            }
            if column < used + cells {
                return Some(range.start + index..range.start + index + grapheme.len());
            }
            used += cells;
        }
        Some(range.end..range.end)
    }

    pub(crate) fn press(&mut self, point: Position) {
        self.selection = self.at(point).map(|anchor| Selection {
            origin: point,
            head: anchor.clone(),
            anchor,
            moved: false,
            dragging: true,
        });
    }

    pub(crate) fn drag(&mut self, point: Position) -> bool {
        let Some(head) = self.at(point) else {
            return false;
        };
        let Some(selection) = &mut self.selection else {
            return false;
        };
        let previous = (selection.head.clone(), selection.moved);
        selection.head = head;
        selection.moved |= selection.origin != point;
        previous != (selection.head.clone(), selection.moved)
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        let selection = self.selection.as_ref().filter(|s| s.moved)?;
        let range = selection.anchor.start.min(selection.head.start)
            ..selection.anchor.end.max(selection.head.end);
        (!range.is_empty()).then_some(range)
    }

    pub(crate) fn selected_text(&self) -> Option<&str> {
        self.selected_range().map(|range| &self.text[range])
    }

    pub(crate) fn visible_lines(&self) -> Vec<Line<'_>> {
        let selected = self.selected_range();
        self.rows
            .iter()
            .skip(self.scroll)
            .take(usize::from(self.area.height))
            .map(|row| {
                let text = &self.text[row.clone()];
                let Some(selected) = &selected else {
                    return Line::raw(text);
                };
                let mut spans = text
                    .grapheme_indices(true)
                    .map(|(index, grapheme)| {
                        let offset = row.start + index;
                        if selected.contains(&offset) {
                            Span::styled(
                                grapheme,
                                Style::default().add_modifier(Modifier::REVERSED),
                            )
                        } else {
                            Span::raw(grapheme)
                        }
                    })
                    .collect::<Vec<_>>();
                // Show selected hard newlines, including otherwise empty rows.
                if selected.contains(&row.end) && self.text.as_bytes().get(row.end) == Some(&b'\n')
                {
                    spans.push(Span::styled(
                        " ",
                        Style::default().add_modifier(Modifier::REVERSED),
                    ));
                }
                Line::from(spans)
            })
            .collect()
    }
}
