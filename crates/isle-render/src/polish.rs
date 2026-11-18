//! Native Cimori shell and shared view components. This is the real tiny-skia UI.
use super::*;
use crate::layout::BODY as CONTENT;
#[path = "ai_view.rs"]
mod ai_view;
#[path = "daily_view.rs"]
mod daily_view;
#[path = "system_view.rs"]
mod system_view;

impl PillRenderer {
    pub(super) fn hover_feedback(&mut self, canvas: f32, w: f32, h: f32, y: f32, s: &IslandState, cfg: &IsleConfig) {
        if !matches!(s.mode, PillMode::Expanded(_)) {
            return;
        }
        let Some((px, py)) = self.pointer else { return };
        let Some(hb) = compute_hitboxes(canvas, w, h, y, s)
            .into_iter()
            .find(|b| b.contains(px, py))
        else {
            return;
        };
        if s.ui.hover_highlight {
            self.round(
                hb.x, hb.y, hb.w, hb.h, 9.,
                Color::from_rgba8(245, 241, 235, if self.pressed { 26 } else { 12 }),
            );
        }
        if !s.ui.tooltips { return; }
        let title = match hb.action {
            UiAction::SwitchTab(t) => t.label().to_owned(),
            UiAction::AiOpen => "Assistente".into(),
            UiAction::ToggleCollapse if cfg.ui.arrow_pins && cfg.auto_hide => if s.arrow_pinned { "Soltar ilha" } else { "Fixar ilha aberta" }.into(),
            UiAction::ToggleCollapse => "Recolher".into(),
            UiAction::OpenSettings => "Configurações".into(),
            UiAction::ToggleMic => if s.mic_muted {
                "Ativar microfone"
            } else {
                "Silenciar microfone"
            }
            .into(),
            UiAction::ToggleMute => if s.muted {
                "Ativar som"
            } else {
                "Silenciar som"
            }
            .into(),
            UiAction::OpenMixer => "Mixer do Windows".into(),
            UiAction::OpenPower => "Desligar".into(),
            UiAction::ControlRemove(t) => format!("Tirar {}", t.label()),
            UiAction::ControlAdd(t) => format!("Pôr {}", t.label()),
            UiAction::Control(t) if hb.h < 50. => t.label().to_owned(),
            UiAction::AiNew => "Nova conversa".into(),
            UiAction::AiBack => "Voltar".into(),
            UiAction::AiSend => "Enviar mensagem".into(),
            UiAction::AiStop => "Parar resposta".into(),
            UiAction::AiScroll(n) => if n > 0 {
                "Histórico anterior"
            } else {
                "Mensagens recentes"
            }
            .into(),
            UiAction::ClearNotifications => "Limpar todos os avisos".into(),
            UiAction::SendReply => "Enviar resposta".into(),
            UiAction::ReplyNotification(..) => "Responder".into(),
            UiAction::TrayClick(i, _) => s.tray[i].display_name.clone(),
            UiAction::Timer(crate::pages::TimerAction::Reset) => "Reiniciar".into(),
            UiAction::Media(k) => match k {
                MediaKey::Previous => "Faixa anterior",
                MediaKey::Next => "Próxima faixa",
                _ => {
                    if s.media.as_ref().is_some_and(|m| m.playing) {
                        "Pausar"
                    } else {
                        "Reproduzir"
                    }
                }
            }
            .into(),
            _ => return,
        };
        let title = self.text.ellipsize(&title, w - 60., 10., Weight::Medium);
        let tw = self.text.width(&title, 10., Weight::Medium) + 18.;
        let left = (hb.x + hb.w / 2. - tw / 2.)
            .clamp((canvas - w) / 2. + 10., (canvas + w) / 2. - tw - 10.);
        let ty = if hb.y + hb.h + 31. < y + h {
            hb.y + hb.h + 6.
        } else {
            hb.y - 29.
        };
        self.round(left, ty, tw, 23., 7., ink());
        self.label(&title, left + 9., ty + 5., 10., Weight::Medium, white());
    }
    pub(super) fn centered_label(
        &mut self,
        text: &str,
        cx: f32,
        y: f32,
        size: f32,
        weight: Weight,
        color: Color,
    ) {
        let w = self.text.width(text, size, weight);
        self.label(text, cx - w / 2., y, size, weight, color);
    }
    pub(super) fn clipped_label(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        size: f32,
        weight: Weight,
        color: Color,
    ) {
        let text = self.text.ellipsize(text, width.max(0.), size, weight);
        self.label(&text, x, y, size, weight, color);
    }
    pub(super) fn subtle_line(&mut self, x: f32, y: f32, w: f32) {
        self.round(x, y, w, 1., 0.5, line());
    }
    pub(super) fn action_icon(&mut self, hb: &crate::pages::HitBox, icon: Icon, selected: bool) {
        self.round(
            hb.x,
            hb.y,
            hb.w,
            hb.h,
            10.,
            if selected { tint() } else { surface() },
        );
        self.ui_icon(
            icon,
            hb.x + hb.w / 2.,
            hb.y + hb.h / 2.,
            17.,
            if selected { accent() } else { white() },
        );
    }
    pub(super) fn empty_state(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        title: &str,
        detail: &str,
        icon: Icon,
    ) {
        self.ui_icon(icon, x + w / 2., y + 42., 34., accent());
        self.centered_label(title, x + w / 2., y + 76., 17., Weight::Medium, white());
        for (i, l) in self
            .text
            .wrap(detail, w - 64., 12., Weight::Regular, 3)
            .iter()
            .enumerate()
        {
            self.centered_label(
                l,
                x + w / 2.,
                y + 106. + i as f32 * 18.,
                12.,
                Weight::Regular,
                muted(),
            );
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_expanded(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        canvas: f32,
        active: TabPage,
        state: &IslandState,
        _cfg: &IsleConfig,
    ) {
        // Port of ExpandedView._buildHeader (isle): ✦ (dimmed, lit while the assistant is open), the date,
        // the time in bold, the gear.
        let (star, date_x, time_x) = crate::layout::header_status(x, y, w, state);
        if let Some(star) = star {
            let lit = state.chat.open && active == TabPage::AiHub;
            self.ui_icon(Icon::Spark, star.x + star.w / 2., star.y + star.h / 2., 15., if lit { white() } else { Color::from_rgba8(255, 255, 255, 153) });
        }
        self.label(&state.date_text, date_x, centered_top(y + 9., 28., 13.), 13., Weight::Regular, Color::from_rgba8(255, 255, 255, 230));
        self.label(&state.clock_text, time_x, centered_top(y + 9., 28., 13.), 13., Weight::Semibold, white());
        self.ui_icon(Icon::Up, x + w - 35., y + 23., 16., if state.arrow_pinned {accent()} else {muted()});
        self.ui_icon(Icon::Gear, x + w - 69., y + 23., 15., muted());
        if active == TabPage::ControlCenter {
            // Port of ControlPanel's Edit button (mac-island 3.1.1); "Concluído" while editing.
            let e = crate::layout::edit_button(x, y);
            let editing = state.control_editing;
            self.round(e.x, e.y, e.w, e.h, e.h / 2., if editing { tint() } else { surface() });
            let label = if editing { "Concluído" } else { "Editar" };
            self.centered_label(label, e.x + e.w / 2., centered_top(e.y, e.h, 12.), 12., Weight::Medium, if editing { accent() } else { white() });
        }
        // Port of ExpandedView._syncTabs (isle): the open tab on a pill with its name, the others' icons dimmed
        // (TAB_OPACITY 255 / 140); a red dot on a page with news.
        let open = crate::layout::open_tab(state);
        let nav = crate::layout::navigation(x, y, w, &state.modules, open);
        let pad = crate::layout::tab_pad(&nav);
        for hb in &nav {
            let UiAction::SwitchTab(tab) = hb.action else {
                continue;
            };
            let on = Some(tab) == open;
            if on {
                self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., Color::from_rgba8(255, 255, 255, 23));
                self.label(tab.label(), hb.x + pad + 22., centered_top(hb.y, hb.h, 13.), 13., Weight::Semibold, white());
            }
            let icon = match tab {
                TabPage::AiHub => Icon::Gauge,
                TabPage::ControlCenter => Icon::Controls,
                TabPage::CalendarWeather => Icon::Calendar,
                TabPage::Weather => Icon::Sun,
                TabPage::Media => Icon::Music,
                TabPage::Notifications => Icon::Bell,
                TabPage::Timer => Icon::Timer,
                TabPage::Tray => Icon::Tray,
                TabPage::Clipboard => Icon::Clipboard,
                TabPage::Shelf => Icon::Folder,
                TabPage::Usb => Icon::Usb,
                TabPage::Downloads => Icon::Download,
                TabPage::Power => Icon::Power,
            };
            let (cx, cy) = (hb.x + pad + 8., hb.y + hb.h / 2.);
            self.ui_icon(icon, cx, cy, 16., if on { white() } else { Color::from_rgba8(255, 255, 255, 140) });
            if !on && tab == TabPage::Notifications && state.notifications.unread_count() > 0 {
                self.round(cx + 5., cy - 9., 6., 6., 3., Color::from_rgba8(255, 69, 58, 255));
            }
        }
        self.subtle_line(x + 20., y + 84., w - 40.);
        self.panel_body(canvas,x,y,w,h,active,state);
    }
    pub(super) fn panel_contents(&mut self,canvas:f32,x:f32,y:f32,w:f32,h:f32,active:TabPage,state:&IslandState){
        match active {
            TabPage::AiHub => self.ai_view(x, y, w, h, state),
            TabPage::ControlCenter => self.control_view(canvas, x, y, w, h, state),
            TabPage::Notifications => self.notices_view(canvas, x, y, w, h, state),
            TabPage::Tray => self.tray_view(canvas, x, y, w, h, state),
            TabPage::Media => self.media_view(canvas, x, y, w, h, state),
            TabPage::CalendarWeather => self.agenda_view(canvas, x, y, w, h, state),
            TabPage::Weather => self.weather_view(canvas, x, y, w, h, state),
            TabPage::Timer => self.focus_view(canvas, x, y, w, h, state),
            TabPage::Clipboard | TabPage::Shelf | TabPage::Usb | TabPage::Downloads => {
                self.phase8_view(active, x, y, w, h, &state.phase8, state.now_ms)
            }
            TabPage::Power => self.power_view(canvas, x, y, w, h, state),
        }
    }
}
