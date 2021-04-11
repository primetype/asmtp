mod focus;
mod pages;
pub(self) mod util;
pub(self) mod widget;

pub use self::focus::Focus;
use self::pages::Pages;
use crate::{app::App, event};
use anyhow::Result;
use tui::{backend::Backend, layout::Rect, Frame};

pub struct Ui {
    pages: Pages,
    focus: Focus,

    first_render: bool,
    size: Rect,
}

impl Ui {
    pub async fn new<B>(backend: &B, app: &App) -> Self
    where
        B: Backend,
    {
        Self {
            pages: Pages::new(app).await,
            focus: Focus::default(),
            first_render: true,
            size: backend.size().unwrap_or_default(),
        }
    }

    pub fn input(&mut self, key: event::Key) {
        self.pages.input(&mut self.focus, key)
    }

    pub async fn update<B>(&mut self, backend: &B, app: &mut App) -> Result<()>
    where
        B: Backend,
    {
        self.pages.update(app).await?;
        self.update_size(backend);
        Ok(())
    }

    pub fn draw<B>(&self, f: &mut Frame<B>)
    where
        B: Backend,
    {
        self.pages.draw(&self.focus, f, f.size());
    }

    fn update_size<B>(&mut self, backend: &B)
    where
        B: Backend,
    {
        if let Ok(size) = backend.size() {
            if self.first_render || self.size != size {
                self.size = size;
            }
        }
    }
}
