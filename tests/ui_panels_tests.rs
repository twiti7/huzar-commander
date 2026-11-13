use huzar_commander::fs_item::{SortColumn, SortOrder};
use huzar_commander::ui::panels::sort_indicator;

#[test]
fn test_sort_indicator_display() {
    assert_eq!(sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Ascending), "▲");
    assert_eq!(sort_indicator(SortColumn::Name, SortColumn::Name, SortOrder::Descending), "▼");
    assert_eq!(sort_indicator(SortColumn::Size, SortColumn::Name, SortOrder::Ascending), "");
}
