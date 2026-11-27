use super::*;
impl PillRenderer {
    pub(super) fn control_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        let grabbed = s.control_drag;
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            let tile = match hb.action {
                UiAction::Control(t) => Some((t, None)),
                UiAction::ControlGrab(i) => s.control_tiles.get(i).map(|t| (*t, Some(i))),
                _ => None,
            };
            if let Some((t, index)) = tile {
                    let on = match t {
                        ControlTile::Wifi => s.wifi_connected,
                        ControlTile::Bluetooth => s.bluetooth_on,
                        ControlTile::NightLight => s.night_light_on,
                        ControlTile::Microphone if s.mic_available => Some(!s.mic_muted),
                        _ => None,
                    } == Some(true);
                    // Dragging: the held control lit, where it would land outlined.
                    let held = index.is_some() && grabbed.is_some_and(|(from, _)| Some(from) == index);
                    let over = index.is_some() && grabbed.is_some_and(|(from, to)| from != to && Some(to) == index);
                    // The layout makes the first two buttons big.
                    let big = hb.h > 50.;
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        if big { 15. } else { 11. },
                        if on || held {
                            tint()
                        } else {
                            Color::from_rgba8(255, 255, 255, 6)
                        },
                    );
                    if over {
                        self.stroke_round(hb.x, hb.y, hb.w, hb.h, if big { 15. } else { 11. }, accent());
                    }
                    let cx = if big { hb.x + 26. } else { hb.x + hb.w / 2. };
                    self.control_icon(t, cx, hb.y + if big { 27. } else { 15. });
                    if big {
                        self.label(
                            t.label(),
                            hb.x + 52.,
                            hb.y + 11.,
                            13.,
                            Weight::Medium,
                            white(),
                        );
                        let sub = match t {
                            ControlTile::Wifi => s.wifi_name.as_deref().unwrap_or("Configurações"),
                            ControlTile::Bluetooth | ControlTile::NightLight => {
                                match if t == ControlTile::Bluetooth { s.bluetooth_on } else { s.night_light_on } {
                                    Some(true) => "Ligado",
                                    Some(false) => "Desligado",
                                    None => "Configurações",
                                }
                            }
                            ControlTile::Microphone if s.mic_available => if s.mic_muted { "Silenciado" } else { "Ligado" },
                            _ => "",
                        };
                        self.clipped_label(
                            sub,
                            hb.x + 52.,
                            hb.y + 33.,
                            hb.w - 62.,
                            11.,
                            Weight::Regular,
                            muted(),
                        );
                    } else {
                        let label = self
                            .text
                            .ellipsize(t.label(), hb.w - 6., 10., Weight::Medium);
                        self.centered_label(&label, cx, hb.y + 30., 10., Weight::Medium, muted());
                    }
                    continue;
            }
            match hb.action {
                UiAction::ControlRemove(_) => {
                    self.circle(hb.x + hb.w / 2., hb.y + hb.h / 2., 9., Color::from_rgba8(110, 112, 120, 255));
                    self.ui_icon(Icon::Minus, hb.x + hb.w / 2., hb.y + hb.h / 2., 14., white());
                }
                UiAction::ControlAdd(t) => {
                    self.round(hb.x, hb.y, hb.w, hb.h, 10., Color::from_rgba8(255, 255, 255, 6));
                    self.ui_icon(Icon::Plus, hb.x + 16., hb.y + hb.h / 2., 13., accent());
                    let name = self.text.ellipsize(t.label(), hb.w - 36., 11., Weight::Medium);
                    self.label(&name, hb.x + 30., centered_top(hb.y, hb.h, 11.), 11., Weight::Medium, white());
                }
                UiAction::ToggleMute => self.action_icon(
                    &hb,
                    if s.muted {
                        Icon::VolumeOff
                    } else {
                        Icon::Volume
                    },
                    false,
                ),
                UiAction::ToggleMic => self.action_icon(
                    &hb,
                    if s.mic_muted { Icon::MicOff } else { Icon::Mic },
                    s.mic_muted,
                ),
                UiAction::SetVolume(_) => {
                    self.slider(&hb, s.volume_pct, s.muted);
                    if s.ui.volume_percent {
                        self.label_right(
                            &format!("{}%", s.volume_pct),
                            hb.x + hb.w + 43.,
                            hb.y + 9.,
                            Weight::Medium,
                            muted(),
                        );
                    }
                }
                UiAction::SetOutput(i) => {
                    let output = &s.audio_outputs[i];
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        8.,
                        if output.is_default { tint() } else { surface() },
                    );
                    let name = self
                        .text
                        .ellipsize(&output.name, hb.w - 14., 10., Weight::Medium);
                    self.centered_label(
                        &name,
                        hb.x + hb.w / 2.,
                        centered_top(hb.y, hb.h, 10.),
                        10.,
                        Weight::Medium,
                        if output.is_default { accent() } else { muted() },
                    );
                }
                UiAction::EditControls => {
                    // Port of ControlPanel's Edit button (mac-island 3.1.1); "Concluído" while editing.
                    let editing = s.control_editing;
                    self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., if editing { tint() } else { surface() });
                    let label = if editing { "Concluído" } else { "Editar" };
                    self.centered_label(label, hb.x + hb.w / 2., centered_top(hb.y, hb.h, 12.), 12., Weight::Medium, if editing { accent() } else { white() });
                }
                UiAction::OpenMixer => {
                    self.label(
                        "Volume por aplicativo",
                        x + 20.,
                        hb.y + 5.,
                        11.,
                        Weight::Medium,
                        muted(),
                    );
                    self.action_icon(&hb, Icon::Controls, false);
                    if s.app_volumes.is_empty() {
                        self.clipped_label(
                            s.control_error
                                .as_deref()
                                .unwrap_or("Nenhum aplicativo com áudio"),
                            x + 20.,
                            hb.y + 32.,
                            w - 40.,
                            12.,
                            Weight::Regular,
                            muted(),
                        );
                    }
                }
                UiAction::AppMute(i) => self.action_icon(
                    &hb,
                    if s.app_volumes[i].muted {
                        Icon::VolumeOff
                    } else {
                        Icon::Volume
                    },
                    false,
                ),
                UiAction::AppVolume(i, _) => {
                    let a = &s.app_volumes[i];
                    self.clipped_label(
                        &a.name,
                        x + 20.,
                        hb.y + 4.,
                        130.,
                        11.,
                        Weight::Medium,
                        white(),
                    );
                    self.slider(&hb, a.volume_pct, a.muted);
                    if s.ui.volume_percent {
                        self.label_right(
                            &format!("{}%", a.volume_pct),
                            x + w - 20.,
                            hb.y + 5.,
                            Weight::Regular,
                            muted(),
                        );
                    }
                }
                _ => {}
            }
        }
        if s.control_editing {
            let top = crate::layout::control_add_top(s.control_tiles.len());
            let missing = crate::layout::missing_controls(s).len();
            self.label(if missing > 0 { "Adicionar controles" } else { "Todos os controles estão na central" },
                x + 20., y + top + 4., 12., Weight::Semibold, muted());
            if s.control_tiles.is_empty() {
                self.clipped_label("Toque num controle abaixo para pô-lo na central", x + 20., y + CONTENT + 2., w - 40., 11., Weight::Regular, muted());
            }
            return;
        }
        if !s.audio_available {
            self.clipped_label(
                s.control_error
                    .as_deref()
                    .unwrap_or("Saída de áudio indisponível"),
                x + 20.,
                y + crate::layout::control_tiles_end(s.control_tiles.len()) + 20.,
                w - 40.,
                12.,
                Weight::Regular,
                muted(),
            );
        }
    }
    pub(super) fn notices_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        let list = s.notifications.ordered();
        let page = s.notification_page.min(s.notifications.last_page());
        self.label(
            "Seus avisos",
            x + 20.,
            y + CONTENT + 4.,
            18.,
            Weight::Medium,
            white(),
        );
        if list.is_empty() {
            self.empty_state(
                x,
                y + CONTENT + 40.,
                w,
                "Tudo em dia por aqui.",
                s.notification_error
                    .as_deref()
                    .unwrap_or("Quando chegar um aviso, ele aparece aqui."),
                Icon::Bell,
            );
            return;
        }
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            match hb.action {
                UiAction::ReadNotification(id, at) => {
                    let Some(n) = list.iter().find(|n| n.id == id && n.arrival_ms == at) else {
                        continue;
                    };
                    self.subtle_line(hb.x, hb.y + hb.h + 1., hb.w);
                    self.notification_icon(n, hb.x, hb.y + 6., 28.);
                    let age = isle_core::notifications::notification_age(n.arrival_ms, s.now_ms);
                    let agew = self.text.width(&age, 10., Weight::Regular);
                    self.notification_label(
                        &n.app_name,
                        hb.x + 40.,
                        hb.y,
                        hb.w - 60. - agew,
                        10.,
                        Weight::Medium,
                        muted(),
                    );
                    self.label(
                        &age,
                        hb.x + hb.w - agew,
                        hb.y,
                        10.,
                        Weight::Regular,
                        muted(),
                    );
                    self.notification_label(
                        &n.title,
                        hb.x + 40.,
                        hb.y + 17.,
                        hb.w - 44.,
                        13.,
                        Weight::Semibold,
                        white(),
                    );
                    if n.unread {
                        self.round(hb.x + 32., hb.y + 22., 4., 4., 2., accent());
                    }
                    if let Some(reply) = s
                        .reply
                        .as_ref()
                        .filter(|r| r.id == id && r.arrival_ms == at)
                    {
                        self.round(hb.x + 12., hb.y + 35., hb.w - 66., 24., 7., surface());
                        let text = if reply.text.is_empty() {
                            "Digite uma resposta".to_owned()
                        } else {
                            format!("{}|", reply.text)
                        };
                        self.notification_label(
                            &text,
                            hb.x + 20.,
                            hb.y + 40.,
                            hb.w - 82.,
                            11.,
                            Weight::Regular,
                            white(),
                        );
                    } else {
                        self.notification_label(
                            &n.body,
                            hb.x + 40.,
                            hb.y + 38.,
                            hb.w - if n.can_reply() { 90. } else { 44. },
                            11.,
                            Weight::Regular,
                            muted(),
                        );
                    }
                }
                UiAction::ClearNotifications => self.action_icon(&hb, Icon::Trash, false),
                UiAction::SendReply => self.action_icon(&hb, Icon::Send, true),
                UiAction::ReplyNotification(id, at) => {
                    if !s
                        .reply
                        .as_ref()
                        .is_some_and(|r| r.id == id && r.arrival_ms == at)
                    {
                        self.action_icon(&hb, Icon::Message, false);
                    }
                }
                UiAction::NotificationsPage(p) => {
                    self.action_icon(&hb, if p < page { Icon::Left } else { Icon::Right }, false)
                }
                _ => {}
            }
        }
        self.clipped_label(
            &format!(
                "{} avisos · {} não lidos · {}/{}",
                list.len(),
                s.notifications.unread_count(),
                page + 1,
                s.notifications.last_page() + 1
            ),
            x + 20.,
            y + h - 31.,
            w - 140.,
            10.,
            Weight::Regular,
            muted(),
        );
    }
    pub(super) fn tray_view(
        &mut self,
        canvas: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        s: &IslandState,
    ) {
        let body = y + crate::layout::TRAY_DROP;
        self.label(
            "Por perto.",
            x + 20.,
            body + CONTENT + 4.,
            20.,
            Weight::Medium,
            white(),
        );
        let count = isle_core::tray::overflow_indices(&s.tray).len();
        if count == 0 {
            self.empty_state(
                x,
                body + CONTENT + 37.,
                w,
                "Tudo à mão",
                s.tray_error
                    .as_deref()
                    .unwrap_or("Seus aplicativos fixados estão na barra compacta."),
                Icon::Tray,
            );
            return;
        }
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            match hb.action {
                UiAction::TrayClick(i, _) => {
                    let item = &s.tray[i];
                    self.round(
                        hb.x,
                        hb.y,
                        hb.w,
                        hb.h,
                        12.,
                        Color::from_rgba8(255, 255, 255, 4),
                    );
                    if s.ui.tray_grid {
                        let size = 24.;
                        self.draw_tray_icon(
                            item,
                            hb.x + (hb.w - size) / 2.,
                            hb.y + (hb.h - size) / 2.,
                            size,
                        );
                    } else {
                        self.draw_tray_icon(item, hb.x + (hb.w - 28.) / 2., hb.y + 8., 28.);
                        let name = self.text.ellipsize(
                            item.display_name.lines().next().unwrap_or("Aplicativo"),
                            hb.w - 8.,
                            10.,
                            Weight::Medium,
                        );
                        self.centered_label(
                            &name,
                            hb.x + hb.w / 2.,
                            hb.y + 45.,
                            10.,
                            Weight::Medium,
                            muted(),
                        );
                    }
                }
                UiAction::TrayPage(p) => self.action_icon(
                    &hb,
                    if p < crate::pages::tray_page(s) {
                        Icon::Left
                    } else {
                        Icon::Right
                    },
                    false,
                ),
                _ => {}
            }
        }
        self.clipped_label(
            s.tray_error
                .as_deref()
                .unwrap_or("Clique direito abre o menu do aplicativo"),
            x + 20.,
            y + h - 33.,
            w - 140.,
            10.,
            Weight::Regular,
            muted(),
        );
    }
}

impl PillRenderer {
    /// Port of PowerPanel::Draw (mac-island 3.1.1): the actions on, each an icon over its name; one
    /// that asks first opens "Desligar agora?" with Cancelar and the action under the row.
    pub(super) fn power_view(&mut self, canvas: f32, x: f32, y: f32, w: f32, h: f32, s: &IslandState) {
        use isle_core::power::PowerAction;
        if s.power.shown(s.hibernate_allowed).is_empty() {
            self.clipped_label("Ligue alguma ação em Configurações → Central de controle", x + 20., y + CONTENT + 30., w - 40., 12., Weight::Regular, muted());
            return;
        }
        for hb in compute_hitboxes(canvas, w, h, y, s) {
            match hb.action {
                UiAction::Power(a) => {
                    let asked = s.power_confirm == Some(a);
                    self.round(hb.x, hb.y, hb.w, hb.h, 14., if asked { tint() } else { Color::from_rgba8(255, 255, 255, 6) });
                    let icon = match a {
                        PowerAction::Sleep => Icon::Moon,
                        PowerAction::Hibernate => Icon::Download,
                        PowerAction::Restart => Icon::Restart,
                        PowerAction::ShutDown => Icon::Power,
                        PowerAction::Lock => Icon::Lock,
                        PowerAction::SignOut => Icon::SignOut,
                    };
                    self.ui_icon(icon, hb.x + hb.w / 2., hb.y + 30., 20., if asked { accent() } else { white() });
                    let name = self.text.ellipsize(a.name(), hb.w - 8., 11., Weight::Medium);
                    self.centered_label(&name, hb.x + hb.w / 2., hb.y + 52., 11., Weight::Medium, muted());
                }
                UiAction::PowerCancel | UiAction::PowerConfirm => {
                    let confirm = hb.action == UiAction::PowerConfirm;
                    let label = if confirm { s.power_confirm.map_or("", |a| a.name()) } else { "Cancelar" };
                    self.round(hb.x, hb.y, hb.w, hb.h, hb.h / 2., if confirm { red() } else { surface() });
                    self.centered_label(label, hb.x + hb.w / 2., centered_top(hb.y, hb.h, 12.), 12., Weight::Semibold, white());
                }
                _ => {}
            }
        }
        if let Some(a) = s.power_confirm {
            self.label(a.question(), x + 20., y + CONTENT + 106., 14., Weight::Semibold, white());
        }
    }
}

impl PillRenderer {
    /// A rounded outline, 1.5 px, in logical coordinates.
    fn stroke_round(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
        let s = self.scale;
        if let Some(path) = build_rounded_rect(x * s, y * s, w * s, h * s, r * s) {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color(color);
            self.layer.stroke_path(&path, &paint, &Stroke { width: 1.5 * s, ..Default::default() }, Transform::identity(), None);
        }
    }
}
