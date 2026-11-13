use egui::Key;
use huzar_commander::ui::file_dialogs::letter_to_key;

#[test]
fn test_letter_to_key_mappings() {
    assert_eq!(letter_to_key('c'), Some(Key::C));
    assert_eq!(letter_to_key('C'), Some(Key::C));
    assert_eq!(letter_to_key('d'), Some(Key::D));
    assert_eq!(letter_to_key('Z'), Some(Key::Z));
    assert_eq!(letter_to_key('/'), Some(Key::Slash));
    assert_eq!(letter_to_key('0'), Some(Key::Num0));
    assert_eq!(letter_to_key('9'), Some(Key::Num9));
    assert_eq!(letter_to_key('?'), None);
    assert_eq!(letter_to_key(' '), None);
}
