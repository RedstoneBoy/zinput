use zinput_engine::eframe::{
    egui::{CursorIcon, Response, Sense, Ui, Widget, WidgetText},
    emath::{pos2, vec2, Align2, NumExt, Rect},
    epaint::{Color32, FontFamily, FontId, Stroke},
};

pub struct Slider<'a> {
    width: Option<f32>,
    height: Option<f32>,
    min_width: f32,
    min_height: f32,

    right_to_left: bool,

    value: f32,

    min_value: Option<&'a mut f32>,
    max_value: Option<&'a mut f32>,

    label: String,
    show_values: bool,
}

impl<'a> Slider<'a> {
    pub fn new(value: f32) -> Self {
        Slider {
            width: None,
            height: None,
            min_width: 20.0,
            min_height: 5.0,

            right_to_left: false,

            value,

            min_value: None,
            max_value: None,

            label: String::new(),
            show_values: true,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = min_height;
        self
    }

    pub fn right_to_left(mut self) -> Self {
        self.right_to_left = true;
        self
    }

    pub fn min_value(mut self, min_value: &'a mut f32) -> Self {
        self.min_value = Some(min_value);
        self
    }

    pub fn max_value(mut self, max_value: &'a mut f32) -> Self {
        self.max_value = Some(max_value);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn show_values(mut self, show_values: bool) -> Self {
        self.show_values = show_values;
        self
    }
}

impl<'a> Widget for Slider<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let font_id = FontId {
            size: 12.0,
            family: FontFamily::Proportional,
        };
        let mono_font_id = FontId {
            size: 12.0,
            family: FontFamily::Monospace,
        };

        let width = self
            .width
            .unwrap_or_else(|| ui.available_size_before_wrap().x)
            .at_least(self.min_width);

        let height = self
            .height
            .unwrap_or_else(|| ui.available_size_before_wrap().y)
            .at_least(self.min_height);

        let (full_rect, mut response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let painter = ui.painter().with_clip_rect(full_rect);

        // TODO: Right to left

        let label_rect = painter.text(
            if self.right_to_left { pos2(full_rect.right(), full_rect.top()) } else { full_rect.min },
            if self.right_to_left { Align2::RIGHT_TOP } else { Align2::LEFT_TOP },
            self.label,
            font_id.clone(),
            ui.visuals().text_color(),
        );

        if self.show_values {
            let val_rect = painter.text(
                if !self.right_to_left { pos2(full_rect.right(), full_rect.top()) } else { full_rect.min },
                if !self.right_to_left { Align2::RIGHT_TOP } else { Align2::LEFT_TOP },
                format!("{:.2}", self.value),
                mono_font_id.clone(),
                ui.visuals().text_color(),
            );

            if let Some(min) = &mut self.min_value {
                let val_rect = painter.text(
                    pos2(if !self.right_to_left { full_rect.left() } else { full_rect.right() }, full_rect.bottom()),
                    if !self.right_to_left { Align2::LEFT_BOTTOM } else { Align2::RIGHT_BOTTOM },
                    format!("{:.2}", min),
                    mono_font_id.clone(),
                    ui.visuals().text_color(),
                );

                let response = ui
                    .allocate_rect(val_rect, Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeHorizontal);

                if let Some(pos) = response.interact_pointer_pos() {
                    let delta = response.drag_delta();
                    let max = match &self.max_value {
                        Some(v) => **v,
                        None => 1.0,
                    };

                    let sign = if self.right_to_left { -1.0 } else { 1.0 };

                    **min = (**min + delta.x * sign / full_rect.width()).clamp(0.0, max);
                }
            }

            if let Some(max) = &mut self.max_value {
                let val_rect = painter.text(
                    pos2(if self.right_to_left { full_rect.left() } else { full_rect.right() }, full_rect.bottom()),
                    if self.right_to_left { Align2::LEFT_BOTTOM } else { Align2::RIGHT_BOTTOM },
                    format!("{:.2}", max),
                    mono_font_id,
                    ui.visuals().text_color(),
                );

                let response = ui
                    .allocate_rect(val_rect, Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeHorizontal);

                if let Some(pos) = response.interact_pointer_pos() {
                    let delta = response.drag_delta();
                    let min = match &self.min_value {
                        Some(v) => **v,
                        None => 0.0,
                    };

                    let sign = if self.right_to_left { -1.0 } else { 1.0 };

                    **max = (**max + delta.x * sign / full_rect.width()).clamp(min, 1.0);
                }
            }
        }

        let rect = {
            let mut rect = full_rect;
            rect.min.y += font_id.size + 2.0;
            rect.max.y -= font_id.size + 2.0;
            rect
        };

        painter.rect_filled(
            Rect {
                min: pos2(
                    if !self.right_to_left {
                        rect.min.x
                    } else {
                        rect.max.x - self.value.clamp(0.0, 1.0) * width
                    },
                    rect.min.y,
                ),
                max: pos2(
                    if !self.right_to_left {
                        rect.min.x + self.value.clamp(0.0, 1.0) * width
                    } else {
                        rect.max.x
                    },
                    rect.max.y,
                ),
            },
            0.0,
            Color32::LIGHT_GRAY,
        );

        if let Some(min) = self.min_value {
            let line_rect = if !self.right_to_left {
                Rect {
                    min: pos2(
                        rect.min.x + min.clamp(0.0, 1.0) * rect.width() - 1.0,
                        rect.min.y,
                    ),
                    max: pos2(
                        rect.min.x + min.clamp(0.0, 1.0) * rect.width() + 1.0,
                        rect.max.y,
                    ),
                }
            } else {
                Rect {
                    min: pos2(
                        rect.max.x - min.clamp(0.0, 1.0) * rect.width() - 1.0,
                        rect.min.y,
                    ),
                    max: pos2(
                        rect.max.x - min.clamp(0.0, 1.0) * rect.width() + 1.0,
                        rect.max.y,
                    ),
                }
            };
            painter.rect_filled(line_rect, 0.0, Color32::LIGHT_RED);
        }

        if let Some(max) = self.max_value {
            let line_rect = if !self.right_to_left {
                Rect {
                    min: pos2(
                        rect.min.x + max.clamp(0.0, 1.0) * rect.width() - 1.0,
                        rect.min.y,
                    ),
                    max: pos2(
                        rect.min.x + max.clamp(0.0, 1.0) * rect.width() + 1.0,
                        rect.max.y,
                    ),
                }
            } else {
                Rect {
                    min: pos2(
                        rect.max.x - max.clamp(0.0, 1.0) * rect.width() - 1.0,
                        rect.min.y,
                    ),
                    max: pos2(
                        rect.max.x - max.clamp(0.0, 1.0) * rect.width() + 1.0,
                        rect.max.y,
                    ),
                }
            };

            painter.rect_filled(
                line_rect,
                0.0,
                Color32::LIGHT_RED,
            );
        }

        painter.rect_stroke(
            rect.shrink(1.0),
            0.0,
            Stroke::new(2.0, ui.visuals().text_color()),
        );

        response
    }
}
