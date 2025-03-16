use std::{cmp::{min, max}, collections::HashMap};

use ratatui::layout::Rect;

use super::{Command, KeybindEventStream};

impl<T:Command> KeybindEventStream<'_,T>{

    pub fn inner(&self, area: Rect)->Rect{
        let len = self.current.as_deref().map(HashMap::len).unwrap_or(0);
        let width = area.width-4/20;
        let full_height = len/width as usize+2;
        let height = max(full_height, min(5, area.height as usize/4));

        Rect { x: area.x, y: area.y, width: area.width, height: area.height-height as u16 }
    }


    pub fn render(
        &self
    ){
    }

}
