use egui::Key;

use super::{CommanderApp, PanelSide};

fn format_cli_token(text: &str) -> String {
    if text.contains(' ') {
        format!("\"{}\"", text)
    } else {
        text.to_string()
    }
}

impl CommanderApp {
    pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if self.context_menu.is_open {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.context_menu.close();
            }
            return;
        }

        if self.is_any_modal_open() {
            return;
        }

        let mut toggle_drive_side = None;
        ctx.input(|i| {
            if i.modifiers.alt && i.key_pressed(Key::F1) {
                toggle_drive_side = Some(PanelSide::Left);
            } else if i.modifiers.alt && i.key_pressed(Key::F2) {
                toggle_drive_side = Some(PanelSide::Right);
            }
        });

        if let Some(side) = toggle_drive_side {
            if self.drive_menu.side == Some(side) {
                self.drive_menu.side = None;
            } else {
                self.open_drive_menu(side);
            }
            return;
        }

        if self.drive_menu.side.is_some() {
            return;
        }

        let mut is_panel_nav = false;
        let mut enter_pressed = false;
        let wants_kbd = ctx.wants_keyboard_input();
        let editing_any_path = self.left_panel.is_editing_path || self.right_panel.is_editing_path;

        ctx.input(|i| {
            if i.modifiers.alt && i.key_pressed(Key::F7) {
                self.open_search_dialog();
            } else if i.modifiers.alt && i.key_pressed(Key::F5) {
                self.trigger_pack_zip();
            } else if i.modifiers.alt && i.key_pressed(Key::Enter) {
                self.trigger_properties();
            } else if i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::Enter) {
                if let Some(item) = self
                    .active_panel()
                    .items
                    .get(self.active_panel().cursor_idx)
                    .filter(|it| !it.is_parent)
                    .cloned()
                {
                    let token = format_cli_token(&item.path.to_string_lossy());
                    if !self.cmd_line.is_empty() && !self.cmd_line.ends_with(' ') {
                        self.cmd_line.push(' ');
                    }
                    self.cmd_line.push_str(&token);
                    self.focus_cli_requested = true;
                }
            } else if i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::Enter) {
                if let Some(item) = self
                    .active_panel()
                    .items
                    .get(self.active_panel().cursor_idx)
                    .filter(|it| !it.is_parent)
                    .cloned()
                {
                    let token = format_cli_token(&item.name);
                    if !self.cmd_line.is_empty() && !self.cmd_line.ends_with(' ') {
                        self.cmd_line.push(' ');
                    }
                    self.cmd_line.push_str(&token);
                    self.focus_cli_requested = true;
                }
            } else if i.modifiers.alt && i.key_pressed(Key::ArrowLeft) {
                is_panel_nav = true;
                self.navigate_panel_back(self.active_side);
            } else if i.modifiers.alt && i.key_pressed(Key::ArrowRight) {
                is_panel_nav = true;
                self.navigate_panel_forward(self.active_side);
            } else if i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::L) {
                self.active_panel_mut().start_path_edit();
            } else if i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::T)
            {
                self.active_tabs_mut().duplicate_active_tab();
                let msg = self.i18n.t("status.tab_created").to_string();
                self.set_status(msg);
            } else if i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::W)
            {
                if self.active_tabs_mut().close_active_tab() {
                    let msg = self.i18n.t("status.tab_closed").to_string();
                    self.set_status(msg);
                } else if self.active_tabs().len() > 1 {
                    let msg = self.i18n.t("status.tab_locked_cannot_close").to_string();
                    self.set_status(msg);
                }
            } else if i.modifiers.ctrl
                && i.modifiers.shift
                && !i.modifiers.alt
                && i.key_pressed(Key::D)
            {
                self.trigger_compare_files();
            } else if i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::D)
            {
                self.open_bookmarks_dialog();
            } else if i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::M)
            {
                self.open_multi_rename_dialog();
            } else if i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::Q)
            {
                self.toggle_quick_view();
            } else if i.modifiers.ctrl
                && i.modifiers.shift
                && !i.modifiers.alt
                && i.key_pressed(Key::Tab)
            {
                is_panel_nav = true;
                self.active_tabs_mut().prev_tab();
            } else if i.modifiers.ctrl
                && !i.modifiers.shift
                && !i.modifiers.alt
                && i.key_pressed(Key::Tab)
            {
                is_panel_nav = true;
                self.active_tabs_mut().next_tab();
            } else if i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && i.key_pressed(Key::F2)
            {
                self.trigger_compare_dirs();
            } else if (i.modifiers.shift && i.key_pressed(Key::F6))
                || (!i.modifiers.alt
                    && !i.modifiers.ctrl
                    && !i.modifiers.shift
                    && i.key_pressed(Key::F2))
            {
                self.trigger_rename();
            } else if i.modifiers.ctrl && i.key_pressed(Key::R) {
                self.refresh_both_panels_async();
                let msg = self.i18n.t("status.panels_refreshed").to_string();
                self.set_status(msg);
            } else if i.modifiers.ctrl && i.key_pressed(Key::U) {
                self.swap_panels();
            } else if i.modifiers.ctrl && i.key_pressed(Key::ArrowLeft) {
                let right_path = self.right_panel.current_path.clone();
                let repaint_signal = self.make_repaint_signal();
                self.left_panel
                    .start_loading_dir(right_path, None, Some(repaint_signal));
            } else if i.modifiers.ctrl && i.key_pressed(Key::ArrowRight) {
                let left_path = self.left_panel.current_path.clone();
                let repaint_signal = self.make_repaint_signal();
                self.right_panel
                    .start_loading_dir(left_path, None, Some(repaint_signal));
            } else if i.modifiers.ctrl && i.key_pressed(Key::H) {
                self.left_panel.toggle_hidden_all();
                self.right_panel.toggle_hidden_all();
                let state_str = if self.active_panel().show_hidden {
                    self.i18n.t("status.hidden_visible")
                } else {
                    self.i18n.t("status.hidden_hidden")
                };
                let msg = self
                    .i18n
                    .t_fmt("status.hidden_status", &[("state", state_str)]);
                self.set_status(msg);
            } else if !wants_kbd
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
            {
                self.open_mask_select_dialog(true);
            } else if !wants_kbd && !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::Minus) {
                self.open_mask_select_dialog(false);
            } else if !wants_kbd && !i.modifiers.any() && i.key_pressed(Key::Space) {
                is_panel_nav = true;
                let repaint_signal = self.make_repaint_signal();
                if let Some(path) = self
                    .active_panel_mut()
                    .space_select_and_calculate(Some(repaint_signal))
                {
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let msg = self
                        .i18n
                        .t_fmt("status.calc_dir_size_bg", &[("name", &name)]);
                    self.set_status(msg);
                }
            } else if i.modifiers.ctrl && i.key_pressed(Key::F) {
                self.active_panel_mut().is_filter_open = true;
            } else if !wants_kbd
                && i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::C)
            {
                if !self.active_panel().is_in_zip() {
                    self.trigger_clipboard_copy();
                }
            } else if !wants_kbd
                && i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::X)
            {
                if !self.active_panel().is_in_zip() {
                    self.trigger_clipboard_cut();
                }
            } else if !wants_kbd
                && i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::V)
            {
                if !self.active_panel().is_in_zip() {
                    let target_dir = self.active_panel().current_path.clone();
                    self.trigger_clipboard_paste(target_dir);
                }
            } else if !wants_kbd
                && i.modifiers.ctrl
                && !i.modifiers.alt
                && !i.modifiers.shift
                && i.key_pressed(Key::A)
            {
                self.active_panel_mut().select_all();
            } else if i.key_pressed(Key::Escape) {
                if self.active_panel().is_editing_path {
                    self.active_panel_mut().cancel_path_edit();
                    is_panel_nav = true;
                } else if self.active_panel().quick_search_active().is_some() {
                    self.active_panel_mut().quick_search_clear();
                } else if self.active_panel_mut().cancel_loading()
                    || self.passive_panel_mut().cancel_loading()
                {
                    let msg = self.i18n.t("panels.loading_cancelled").to_string();
                    self.set_status(msg);
                } else if !self.active_panel().filter.is_empty() {
                    self.active_panel_mut().clear_filter();
                    is_panel_nav = true;
                } else if wants_kbd {
                    is_panel_nav = true;
                }
            } else if !i.modifiers.alt && i.key_pressed(Key::F1) {
                self.about_dialog.open_overview();
            } else if !wants_kbd
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && i.key_pressed(Key::Tab)
            {
                is_panel_nav = true;
                self.active_side = match self.active_side {
                    PanelSide::Left => PanelSide::Right,
                    PanelSide::Right => PanelSide::Left,
                };
            } else if i.key_pressed(Key::F3) {
                self.trigger_f3_view();
            } else if i.key_pressed(Key::F4) {
                self.trigger_f4_edit();
            } else if i.key_pressed(Key::F5) {
                self.trigger_f5_copy();
            } else if i.key_pressed(Key::F6) {
                self.trigger_f6_move();
            } else if i.key_pressed(Key::F7) {
                self.trigger_f7_mkdir();
            } else if i.key_pressed(Key::F8) || (!wants_kbd && i.key_pressed(Key::Delete)) {
                self.trigger_f8_delete();
            } else if !wants_kbd && i.key_pressed(Key::Backspace) {
                is_panel_nav = true;
                let repaint_signal = self.make_repaint_signal();
                self.active_panel_mut().go_up_async(Some(repaint_signal));
            } else if !wants_kbd && i.key_pressed(Key::ArrowUp) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if p.cursor_idx > 0 {
                    p.cursor_idx -= 1;
                    p.scroll_to_cursor = true;
                }
            } else if !wants_kbd && i.key_pressed(Key::ArrowDown) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() && p.cursor_idx + 1 < p.items.len() {
                    p.cursor_idx += 1;
                    p.scroll_to_cursor = true;
                }
            } else if !wants_kbd && i.key_pressed(Key::PageUp) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                p.cursor_idx = p.cursor_idx.saturating_sub(15);
                p.scroll_to_cursor = true;
            } else if !wants_kbd && i.key_pressed(Key::PageDown) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() {
                    p.cursor_idx = (p.cursor_idx + 15).min(p.items.len() - 1);
                    p.scroll_to_cursor = true;
                }
            } else if !wants_kbd && i.key_pressed(Key::Home) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                p.cursor_idx = 0;
                p.scroll_to_cursor = true;
            } else if !wants_kbd && i.key_pressed(Key::End) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                if !p.items.is_empty() {
                    p.cursor_idx = p.items.len() - 1;
                    p.scroll_to_cursor = true;
                }
            } else if !wants_kbd && i.key_pressed(Key::Insert) {
                is_panel_nav = true;
                let p = self.active_panel_mut();
                let idx = p.cursor_idx;
                p.toggle_select(idx);
                if idx + 1 < p.items.len() {
                    p.cursor_idx += 1;
                    p.scroll_to_cursor = true;
                }
            } else if !i.modifiers.ctrl && !i.modifiers.alt && i.key_pressed(Key::Enter) {
                enter_pressed = true;
            } else if !wants_kbd && !editing_any_path && !i.modifiers.ctrl && !i.modifiers.alt {
                for ev in &i.events {
                    if let egui::Event::Text(txt) = ev {
                        if txt.is_empty()
                            || txt == "+"
                            || txt == "-"
                            || txt == "="
                            || txt == " "
                            || txt.chars().all(|c| c.is_control())
                        {
                            continue;
                        }
                        self.active_panel_mut().quick_search_push(txt);
                    }
                }
            }
        });

        if is_panel_nav {
            // Zdejmij focus z menu i zamknij ewentualne otwarte menu rozwijane
            ctx.memory_mut(|mem| {
                mem.stop_text_input();
                mem.close_popup();
            });
        }

        if enter_pressed {
            if editing_any_path || (wants_kbd && !self.cmd_line.trim().is_empty()) {
                return;
            }
            // Zdejmij focus z menu oraz skonsumuj klawisz Enter, aby nie aktywował przycisków menu
            ctx.memory_mut(|mem| {
                mem.stop_text_input();
                mem.close_popup();
            });
            let _ = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

            let active = self.active_panel();
            if let Some(item) = active.items.get(active.cursor_idx).cloned() {
                self.open_item(&item);
            }
        }
    }
}
