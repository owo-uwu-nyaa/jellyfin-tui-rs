use std::cmp::min;

use jellyhaj_widgets_core::{
    DimensionsParameter, ItemWidget, JellyhajWidget, Wrapper, async_task::TaskSubmitter,
};
use ratatui::{
    layout::{Position, Rect, Size},
    widgets::{Block, Padding, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};

pub struct ItemGrid<T: ItemWidget> {
    items: Vec<T>,
    current: usize,
    width: usize,
    title: String,
    item_size: Size,
    skip_rows: usize,
}

impl<T: ItemWidget> ItemGrid<T> {
    pub fn new(items: Vec<T>, current: usize, title: String, dim: DimensionsParameter<'_>) -> Self {
        Self {
            items,
            current,
            width: 1,
            title,
            item_size: <T as ItemWidget>::dimensions_static(dim),
            skip_rows: 0,
        }
    }
}

pub struct ItemGridState<T> {
    pub items: Vec<T>,
    pub title: String,
    pub current: usize,
}

pub enum ItemGridAction<T> {
    SpecificInner(usize, T),
    CurrentInner(T),
    Up,
    Left,
    Right,
    Down,
}

#[derive(Clone, Copy)]
struct GridWrapper {
    index: usize,
}

impl<T: Send + 'static> Wrapper<T> for GridWrapper {
    type F = ItemGridAction<T>;

    fn wrap(&self, val: T) -> Self::F {
        ItemGridAction::SpecificInner(self.index, val)
    }
}

impl<T: ItemWidget> JellyhajWidget for ItemGrid<T> {
    type State = ItemGridState<<T as ItemWidget>::State>;
    type Action = ItemGridAction<<T as ItemWidget>::Action>;
    type ActionResult = <T as ItemWidget>::ActionResult;

    fn into_state(self) -> Self::State {
        ItemGridState {
            items: self.items.into_iter().map(ItemWidget::into_state).collect(),
            title: self.title,
            current: self.current,
        }
    }

    fn apply_action(
        &mut self,
        action: Self::Action,
    ) -> jellyhaj_widgets_core::Result<Option<Self::ActionResult>> {
        match action {
            ItemGridAction::SpecificInner(index, action) => self
                .items
                .get_mut(index)
                .and_then(|v| ItemWidget::apply_action(v, action).transpose())
                .transpose(),
            ItemGridAction::CurrentInner(action) => self
                .items
                .get_mut(self.current)
                .and_then(|v| ItemWidget::apply_action(v, action).transpose())
                .transpose(),
            ItemGridAction::Up => {
                self.current = self.current.saturating_sub(self.width);
                Ok(None)
            }
            ItemGridAction::Left => {
                self.current = self.current.saturating_sub(1);
                Ok(None)
            }
            ItemGridAction::Right => {
                self.current = min(self.items.len().saturating_sub(1), self.current + 1);
                Ok(None)
            }
            ItemGridAction::Down => {
                self.current = min(
                    self.items.len().saturating_sub(1),
                    self.current + self.width,
                );
                Ok(None)
            }
        }
    }

    fn click(
        &mut self,
        mut position: ratatui::prelude::Position,
        size: Size,
        kind: ratatui::crossterm::event::MouseEventKind,
        modifier: ratatui::crossterm::event::KeyModifiers,
    ) -> jellyhaj_widgets_core::Result<Option<Self::ActionResult>> {
        if position.x < 2
            || position.y < 2
            || position.x >= size.width - 2
            || position.y >= size.height - 2
        {
            Ok(None)
        } else {
            position.x -= 2;
            position.y -= 2;
            let row = position.y / (self.item_size.height + 1);
            let row = (row as usize) + self.skip_rows;
            let y_position = position.y % (self.item_size.height + 1);
            let col = position.x / (self.item_size.width + 1);
            let index = row * self.width + (col as usize);
            let x_position = position.x % (self.item_size.width + 1);
            if x_position < self.item_size.width
                && y_position < self.item_size.height
                && let Some(item) = self.items.get_mut(index)
            {
                ItemWidget::click(
                    item,
                    Position {
                        x: x_position,
                        y: y_position,
                    },
                    self.item_size,
                    kind,
                    modifier,
                )
            } else {
                Ok(None)
            }
        }
    }

    fn render_fallible_inner(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        task: jellyhaj_widgets_core::async_task::TaskSubmitter<
            Self::Action,
            impl jellyhaj_widgets_core::Wrapper<Self::Action>,
        >,
    ) -> jellyhaj_widgets_core::Result<()> {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        self.width = ((main.width + 1) / (self.item_size.width + 1)).into();
        let height: usize = ((main.height + 1) / (self.item_size.height + 1)).into();
        let rows = self.items.len().div_ceil(self.width);
        let row_index = self.current / self.width;
        self.skip_rows = if height < rows
            && let position = height / 2
            && row_index > position
        {
            min(row_index - position, rows - height)
        } else {
            0
        };
        let position = (0..height)
            .map(|row| main.y + (self.item_size.height + 1) * (row as u16))
            .flat_map(|y| {
                (0..self.width)
                    .map(|col| main.x + (self.item_size.width + 1) * (col as u16))
                    .map(move |x| Position { x, y })
            });
        for ((index, item), position) in self
            .items
            .iter_mut()
            .enumerate()
            .skip(self.skip_rows * self.width)
            .zip(position)
        {
            item.render_item(
                Rect::from((position, self.item_size)),
                buf,
                TaskSubmitter::clone(&task).wrap_with(GridWrapper { index }),
            )?
        }
        outer.render(area, buf);
        if height < rows {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight).render(
                area,
                buf,
                &mut ScrollbarState::new(rows)
                    .position(row_index)
                    .viewport_content_length(self.item_size.height as usize + 1),
            );
        }
        Ok(())
    }
    fn min_width(&self) -> Option<u16> {
        Some(self.item_size.width + 4)
    }

    fn min_height(&self) -> Option<u16> {
        Some(self.item_size.width + 4)
    }

    fn min_width_static(par: DimensionsParameter<'_>) -> Option<u16> {
        Some(<T as ItemWidget>::dimensions_static(par).width + 4)
    }

    fn min_height_static(par: DimensionsParameter<'_>) -> Option<u16> {
        Some(<T as ItemWidget>::dimensions_static(par).height + 4)
    }
}
