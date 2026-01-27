use std::cmp::min;

use jellyhaj_widgets_core::{
    DimensionsParameter, ItemWidget, JellyhajWidget, Wrapper, async_task::TaskSubmitter,
};
use ratatui::{
    layout::{Position, Rect, Size},
    widgets::{Block, Padding, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use tracing::instrument;

#[derive(Debug)]
pub struct ItemList<T: ItemWidget> {
    items: Vec<T>,
    current: usize,
    title: String,
    pub active: bool,
    offset: usize,
    item_size: Size,
}
impl<T: ItemWidget> ItemList<T> {
    pub fn new(items: Vec<T>, current: usize , title: String, dim: DimensionsParameter<'_>) -> Self {
        Self {
            items,
            current,
            title,
            active: false,
            offset: 0,
            item_size: <T as ItemWidget>::dimensions_static(dim),
        }
    }
}

pub enum ItemListAction<T> {
    SpecificInner(usize, T),
    CurrentInner(T),
    Left,
    Right,
}

pub struct ItemListData<T> {
    pub items: Vec<T>,
    pub title: String,
    pub current: usize,
}

#[derive(Clone, Copy)]
struct ListWrapper {
    index: usize,
}

impl<T: Send + 'static> Wrapper<T> for ListWrapper {
    type F = ItemListAction<T>;

    fn wrap(&self, val: T) -> Self::F {
        ItemListAction::SpecificInner(self.index, val)
    }
}

impl<T: ItemWidget> JellyhajWidget for ItemList<T> {
    type State = ItemListData<<T as ItemWidget>::State>;

    type Action = ItemListAction<<T as ItemWidget>::Action>;

    type ActionResult = <T as ItemWidget>::ActionResult;

    fn into_state(self) -> Self::State {
        ItemListData {
            items: self
                .items
                .into_iter()
                .map(<T as ItemWidget>::into_state)
                .collect(),
            title: self.title,
            current: self.current,
        }
    }

    fn apply_action(
        &mut self,
        action: Self::Action,
    ) -> jellyhaj_widgets_core::Result<Option<Self::ActionResult>> {
        match action {
            ItemListAction::SpecificInner(index, action) => self.items[index].apply_action(action),
            ItemListAction::CurrentInner(action) => self.items[self.current].apply_action(action),
            ItemListAction::Left => {
                self.current = min(self.items.len(), self.current + 1);
                Ok(None)
            }
            ItemListAction::Right => {
                self.current = self.current.saturating_sub(1);
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
            let index = position.x / (self.item_size.width + 1);
            let index = (index as usize) + self.offset;
            let x_position = position.x % (self.item_size.width + 1);
            if x_position < self.item_size.width
                && let Some(item) = self.items.get_mut(index)
            {
                ItemWidget::click(
                    item,
                    Position {
                        x: x_position,
                        y: position.y,
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

    #[instrument(skip_all, name = "render_list")]
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
        let visible = min(
            self.items.len(),
            ((main.width + 1) / (self.item_size.width + 1)).into(),
        );
        let mut items = self.items.as_mut_slice();
        let mut current = self.current;
        if visible < items.len()
            && let position_in_visible = visible / 2
            && current > position_in_visible
        {
            self.offset = min(current - position_in_visible, items.len() - visible);
            current -= self.offset;
            items = &mut items[self.offset..];
        } else {
            self.offset = 0
        }

        for (i, item) in items.iter_mut().enumerate().take(visible) {
            item.set_active(self.active && i == current);
            let area = Rect {
                x: main.x
                    + u16::try_from(i).expect("index larger than u16") * (self.item_size.width + 1),
                y: main.y,
                width: self.item_size.width,
                height: main.height,
            };
            item.render_item(
                area,
                buf,
                TaskSubmitter::clone(&task).wrap_with(ListWrapper {
                    index: i + self.offset,
                }),
            )?
        }
        if visible < self.items.len() {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::HorizontalBottom).render(
                area,
                buf,
                &mut ScrollbarState::new(self.items.len())
                    .position(self.current)
                    .viewport_content_length(self.item_size.width as usize + 1),
            );
        }
        outer.render(area, buf);
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
