use std::borrow::Cow;

#[derive(Debug, Default)]
pub struct Focus {
    zoom: Vec<Cow<'static, str>>,
}

impl Focus {
    pub fn check_current<T>(&self, current: T) -> bool
    where
        T: PartialEq<Cow<'static, str>>,
    {
        if let Some(last) = self.zoom.last() {
            current.eq(last)
        } else {
            false
        }
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        let zoom = self.zoom.get(index)?;
        Some(zoom.as_ref())
    }

    pub fn current(&self) -> Option<&str> {
        let zoom = self.zoom.last()?;
        Some(zoom.as_ref())
    }

    pub fn is_root(&self) -> bool {
        self.zoom.is_empty()
    }

    pub fn push<T>(&mut self, zoom: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.zoom.push(zoom.into())
    }

    pub fn pop(&mut self) -> Option<Cow<'static, str>> {
        self.zoom.pop()
    }
}

#[test]
fn test() {
    let mut focus = Focus::default();

    assert!(focus.is_root());

    focus.push("one");
    assert!(focus.check_current("one"));
    assert!(!focus.check_current("two"));
    assert!(!focus.is_root());

    focus.push("two");
    assert!(!focus.check_current("one"));
    assert!(focus.check_current("two"));
    assert!(!focus.is_root());

    assert_eq!(focus.pop(), Some(Cow::Borrowed("two")));
    assert_eq!(focus.pop(), Some(Cow::Borrowed("one")));
    assert_eq!(focus.pop(), None);
    assert_eq!(focus.pop(), None);
    assert!(focus.is_root());
}
