use super::NotifyBackend;

pub struct NoneBackend;

impl NotifyBackend for NoneBackend {
    fn layout_changed(&self, _label: &str) {}
    fn capslock_changed(&self, _enabled: bool) {}
}
