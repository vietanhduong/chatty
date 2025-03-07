use ratatui::widgets::ScrollbarState;

#[derive(Debug, Default)]
pub struct Scroll {
    list_len: usize,
    viewport_len: usize,
    pub position: usize,
    pub scrollbar_state: ScrollbarState,
}

impl Scroll {
    pub fn up(&mut self) {
        self.position = self.position.saturating_sub(1);
        self.scrollbar_state.prev();
    }

    pub fn page_up(&mut self) {
        [..10].iter().for_each(|_| self.up());
    }

    pub fn down(&mut self) {
        let mut clamp = 0_usize;
        if self.list_len > self.viewport_len {
            clamp = self.list_len - self.viewport_len + 1;
        }

        self.position = self
            .position
            .saturating_add(1)
            .clamp(0, clamp.saturating_sub(1));
        self.scrollbar_state.next();
    }

    pub fn page_down(&mut self) {
        [..10].iter().for_each(|_| self.down());
    }

    fn get_position_as_if_last(&self) -> usize {
        let mut pos = 0;
        if self.list_len > self.viewport_len {
            pos = self.list_len - self.viewport_len;
        }
        pos
    }

    pub fn is_position_at_last(&self) -> bool {
        self.position == self.get_position_as_if_last()
    }

    pub fn last(&mut self) {
        self.position = self.get_position_as_if_last();
        self.scrollbar_state.last();
    }

    pub fn set_state(&mut self, list_len: usize, viewport_len: usize) {
        self.list_len = list_len;
        self.viewport_len = viewport_len;
        let mut content_len = list_len.saturating_sub(viewport_len);
        if content_len == 0 {
            content_len = 1;
        }
        self.scrollbar_state = self.scrollbar_state.content_length(content_len);
    }
}
