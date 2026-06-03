use crate::input::ScreenId;
use crate::widgets::modal::ModalKind;

pub enum UiAction {
    None,
    Navigate(ScreenId),
    OpenModal(ModalKind),
    Refresh,
    Quit,
}
