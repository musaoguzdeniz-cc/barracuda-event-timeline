use crate::events::{Event, Kind, TimelineData};
use crate::{theme, windows};
use chrono::Local;
use eframe::egui::{self, Align, Button, Frame, Layout, RichText, ScrollArea, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

const THEME_APP: &str = "Windows Event Timeline";

pub struct Timeline {
    data: Option<TimelineData>,
    worker: Option<Receiver<Result<TimelineData, String>>>,
    error: Option<String>,
    anchor: Option<usize>,
    near_boot: bool,
    selected: Option<usize>,
    kind: Option<Kind>,
    search: String,
    show_xml: bool,
    live: bool,
}

impl Timeline {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::load(&cc.egui_ctx, THEME_APP);
        Self {
            data: None,
            worker: None,
            error: None,
            anchor: None,
            near_boot: false,
            selected: None,
            kind: None,
            search: String::new(),
            show_xml: false,
            live: true,
        }
    }

    fn start(&mut self, files: Vec<PathBuf>, ctx: &egui::Context) {
        if self.worker.is_some() {
            return;
        }
        self.live = files.is_empty();
        self.error = None;
        self.data = None;
        let (tx, rx) = mpsc::channel();
        self.worker = Some(rx);
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = tx.send(windows::read(&files));
            ctx.request_repaint();
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let colors = theme::colors(ui.ctx());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Barracuda")
                    .size(23.0)
                    .strong()
                    .color(colors.accent_text),
            );
            ui.label(
                RichText::new("Windows Olay Zaman Çizelgesi")
                    .size(20.0)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button("Görünüm", |ui| theme::picker(ui, THEME_APP, "Tema"));
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let ready = self.worker.is_none();
            if ui
                .add_enabled(
                    ready,
                    Button::new(RichText::new("Canlı günlükleri oku").color(colors.on_accent))
                        .fill(colors.accent),
                )
                .clicked()
            {
                self.start(Vec::new(), ui.ctx());
            }
            if ui
                .add_enabled(ready, Button::new(".evtx dosyalarını aç"))
                .clicked()
                && let Some(files) = rfd::FileDialog::new()
                    .add_filter("Windows Olay Günlüğü", &["evtx"])
                    .pick_files()
            {
                self.start(files, ui.ctx());
            }
            if !ready {
                ui.spinner();
                ui.label("Olaylar okunuyor…");
            } else if let Some(data) = &self.data {
                let scope = if self.live {
                    "Canlı · son 7 gün"
                } else {
                    "Dışa aktarılmış günlükler"
                };
                ui.label(
                    RichText::new(format!("{scope} · {} kaynak", data.opened)).color(colors.muted),
                );
            }
        });
    }

    fn filters(&mut self, ui: &mut Ui) {
        let Some(data) = &self.data else { return };
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.near_boot, false, "Tüm okunan olaylar");
            if let Some(anchor) = self.anchor {
                ui.selectable_value(&mut self.near_boot, true, "Açılış çevresi · ±30 dk");
                egui::ComboBox::from_id_salt("boot-marker")
                    .selected_text(timestamp(&data.events[anchor]))
                    .show_ui(ui, |ui| {
                        for (index, event) in data
                            .events
                            .iter()
                            .enumerate()
                            .rev()
                            .filter(|(_, event)| event.boot_marker())
                        {
                            let text =
                                format!("{} · {} · {}", timestamp(event), event.computer, event.id);
                            if ui
                                .selectable_value(&mut self.anchor, Some(index), text)
                                .changed()
                            {
                                self.near_boot = true;
                            }
                        }
                    });
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.kind, None, "Tüm türler");
            for kind in Kind::ALL {
                ui.selectable_value(&mut self.kind, Some(kind), kind.label());
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .desired_width(f32::INFINITY)
                .hint_text("Olay kimliği, sağlayıcı, bilgisayar veya açıklama ara"),
        );
    }

    fn results(&mut self, ui: &mut Ui) {
        let Some(data) = &self.data else { return };
        let query = self.search.trim().to_lowercase();
        let rows: Vec<_> = data
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                (!self.near_boot || self.anchor.is_some_and(|anchor| data.near(event, anchor)))
                    && self.kind.is_none_or(|kind| kind == event.kind())
                    && (query.is_empty()
                        || event.id.to_string() == query
                        || [
                            &event.provider,
                            &event.computer,
                            &event.channel,
                            &event.message,
                        ]
                        .into_iter()
                        .any(|text| text.to_lowercase().contains(&query)))
            })
            .map(|(index, _)| index)
            .collect();
        if self.selected.is_none_or(|index| !rows.contains(&index)) {
            self.selected = self
                .anchor
                .filter(|index| rows.contains(index))
                .or_else(|| rows.last().copied());
        }
        let height = ui.available_height().max(220.0);
        ui.columns(2, |columns| {
            columns[0].set_min_height(height);
            columns[1].set_min_height(height);
            columns[0].heading(format!("Olaylar · {}", rows.len()));
            if rows.is_empty() {
                columns[0].label("Bu görünümde olay bulunamadı.");
            }
            let row_height = 46.0;
            ScrollArea::vertical()
                .id_salt("timeline-rows")
                .max_height(height - 33.0)
                .auto_shrink([false, false])
                .show_rows(&mut columns[0], row_height, rows.len(), |ui, range| {
                    for row in range {
                        let index = rows[row];
                        let event = &data.events[index];
                        if event_row(ui, event, self.selected == Some(index), row_height) {
                            self.selected = Some(index);
                            self.show_xml = false;
                        }
                    }
                });
            Frame::new()
                .fill(theme::colors(columns[1].ctx()).surface)
                .inner_margin(12)
                .corner_radius(7)
                .show(&mut columns[1], |ui| {
                    ui.set_min_height((height - 24.0).max(0.0));
                    if let Some(index) = self.selected {
                        detail(ui, &data.events[index], &mut self.show_xml, height - 24.0);
                    } else {
                        ui.label("Olay ayrıntısı için bir kayıt seç.");
                    }
                });
        });
    }
}

impl eframe::App for Timeline {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Some(worker) = &self.worker else { return };
        match worker.try_recv() {
            Ok(Ok(data)) => {
                self.worker = None;
                self.anchor = data.last_boot();
                self.near_boot = self.anchor.is_some();
                self.selected = self.anchor;
                self.kind = None;
                self.search.clear();
                self.show_xml = false;
                self.data = Some(data);
            }
            Ok(Err(error)) => {
                self.worker = None;
                self.error = Some(error);
            }
            Err(TryRecvError::Disconnected) => {
                self.worker = None;
                self.error = Some("Olay okuyucusu yanıt vermedi.".into());
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        Frame::central_panel(ui.style()).inner_margin(15).show(ui, |ui| {
            self.toolbar(ui);
            ui.separator();
            if let Some(error) = &self.error {
                ui.colored_label(theme::colors(ui.ctx()).error, error);
            }
            if let Some(data) = &self.data {
                if !data.warnings.is_empty() {
                    ui.collapsing(format!("Okuma notları · {}", data.warnings.len()), |ui| {
                        for warning in &data.warnings {
                            ui.colored_label(theme::colors(ui.ctx()).warning, warning);
                        }
                    });
                }
                self.filters(ui);
                ui.add_space(6.0);
                self.results(ui);
            } else if self.worker.is_none() && self.error.is_none() {
                ui.label("Bu bilgisayarın günlüklerini oku veya karşılaştırmak için System ve Application .evtx dosyalarını birlikte seç.");
            }
        });
    }
}

fn timestamp(event: &Event) -> String {
    event
        .time
        .with_timezone(&Local)
        .format("%d.%m.%Y %H:%M:%S")
        .to_string()
}

fn event_row(ui: &mut Ui, event: &Event, selected: bool, height: f32) -> bool {
    let colors = theme::colors(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    let painter = ui.painter();
    let background = if selected || response.hovered() {
        colors.selected
    } else {
        colors.surface
    };
    painter.rect_filled(rect, 5, background);
    if selected || response.has_focus() {
        painter.rect_stroke(
            rect,
            5,
            egui::Stroke::new(1.0, colors.accent_text),
            egui::StrokeKind::Inside,
        );
    }
    let heading = format!(
        "{}   ·   {}   ·   Olay {}",
        timestamp(event),
        event.level_label(),
        event.id
    );
    let subtitle = format!("{}   ·   {}", event.provider, event.kind().label());
    let severity = match event.level {
        Some(1 | 2) => colors.error,
        Some(3) => colors.warning,
        _ => colors.text,
    };
    for (text, offset, color) in [(heading, 5.0, severity), (subtitle, 24.0, colors.muted)] {
        let mut job = egui::text::LayoutJob::simple(
            text,
            egui::FontId::proportional(13.0),
            color,
            (rect.width() - 18.0).max(0.0),
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        painter.galley(
            rect.left_top() + egui::vec2(9.0, offset),
            painter.layout_job(job),
            color,
        );
    }
    response.clicked()
}

fn detail(ui: &mut Ui, event: &Event, show_xml: &mut bool, height: f32) {
    let colors = theme::colors(ui.ctx());
    ScrollArea::vertical()
        .id_salt(("detail", event.time, event.record))
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                RichText::new(event.kind().label())
                    .color(colors.secondary_text)
                    .strong(),
            );
            ui.heading(format!("Olay {} · {}", event.id, event.level_label()));
            ui.label(&event.provider);
            ui.label(
                RichText::new(format!("{} · {}", timestamp(event), event.computer))
                    .color(colors.muted),
            );
            let record = event
                .record
                .map_or_else(|| "—".into(), |record| record.to_string());
            ui.label(
                RichText::new(format!(
                    "{} · Kayıt {} · {}",
                    event.channel, record, event.source
                ))
                .color(colors.muted),
            );
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(show_xml, false, "Açıklama");
                ui.selectable_value(show_xml, true, "Olay XML’i");
                let content = if *show_xml {
                    &event.xml
                } else {
                    &event.message
                };
                if ui
                    .add_enabled(!content.is_empty(), Button::new("Kopyala"))
                    .clicked()
                {
                    ui.ctx().copy_text(content.clone());
                }
            });
            if event.clipped {
                ui.colored_label(
                    colors.warning,
                    "Uzun açıklama veya XML önizlemede kısaltıldı.",
                );
            }
            if *show_xml {
                if event.xml.is_empty() {
                    ui.label("Olay XML’i okunamadı.");
                } else {
                    ui.label(RichText::new(&event.xml).monospace());
                }
            } else if event.message.is_empty() {
                ui.label(
                    "Windows bu sağlayıcının açıklamasını çözemedi. Olay XML’ini inceleyebilirsin.",
                );
            } else {
                ui.label(&event.message);
            }
        });
}
