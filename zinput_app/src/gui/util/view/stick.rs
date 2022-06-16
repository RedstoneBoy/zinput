use zinput_engine::eframe::{
    egui::{Response, Sense, Ui, Widget},
    emath::{pos2, vec2, Align2, NumExt},
    epaint::{Color32, FontFamily, FontId, Stroke},
};

pub struct StickView<'a> {
    size: Option<f32>,
    min_size: f32,

    x: f32,
    y: f32,

    deadzone: Option<f32>,
    polygon: Option<&'a [f32]>,

    draw_circle: bool,
    draw_center_dot: bool,
    draw_square: bool,
}

impl<'a> StickView<'a> {
    pub fn new(x: f32, y: f32) -> Self {
        StickView {
            size: None,
            min_size: 20.0,

            x,
            y,

            deadzone: None,
            polygon: None,

            draw_circle: true,
            draw_center_dot: true,
            draw_square: false,
        }
    }

    pub fn min_size(mut self, min_size: f32) -> Self {
        self.min_size = min_size;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn deadzone(mut self, deadzone: f32) -> Self {
        self.deadzone = Some(deadzone);
        self
    }

    pub fn polygon(mut self, polygon: &'a [f32]) -> Self {
        self.polygon = Some(polygon);
        self
    }

    pub fn draw_circle(mut self, draw_circle: bool) -> Self {
        self.draw_circle = draw_circle;
        self
    }

    pub fn draw_center_dot(mut self, draw_center_dot: bool) -> Self {
        self.draw_center_dot = draw_center_dot;
        self
    }

    pub fn draw_square(mut self, draw_square: bool) -> Self {
        self.draw_square = draw_square;
        self
    }
}

impl<'a> Widget for StickView<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let font_id = FontId {
            size: 12.0,
            family: FontFamily::Monospace,
        };

        let size = self
            .size
            .unwrap_or_else(|| ui.available_size_before_wrap().min_elem())
            .at_least(self.min_size);

        let (mut rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter().with_clip_rect(rect);

        let text_height = painter
            .text(
                pos2(rect.left(), rect.bottom()),
                Align2::LEFT_BOTTOM,
                format!("{:+.2}", self.x),
                font_id.clone(),
                ui.visuals().text_color(),
            )
            .height();

        painter.text(
            pos2(rect.right(), rect.bottom()),
            Align2::RIGHT_BOTTOM,
            format!("{:+.2}", self.y),
            font_id.clone(),
            ui.visuals().text_color(),
        );

        let radius = (size - text_height) / 2.0 - 1.0;

        rect.set_height(rect.height() - text_height);

        if let Some(deadzone) = self.deadzone {
            painter.circle_filled(
                rect.center(),
                deadzone.clamp(0.0, 1.0) * radius,
                Color32::LIGHT_RED,
            );
        }

        if self.draw_center_dot {
            painter.circle_filled(rect.center(), 1.0, ui.visuals().text_color());
        }

        if self.draw_square {
            painter.rect_stroke(
                rect.shrink(1.0),
                0.0,
                Stroke::new(2.0, ui.visuals().text_color()),
            );
        }

        if self.draw_circle {
            painter.circle_stroke(
                rect.center(),
                radius,
                Stroke::new(2.0, ui.visuals().text_color()),
            );
        }

        if let Some(polygon) = self.polygon {
            let divisor = polygon.len() as f32;

            let mut points = polygon.iter().enumerate().map(|(i, scalar)| {
                let angle = i as f32 * std::f32::consts::PI * 2.0 / divisor;
                let scalar = scalar * radius;
                let x = scalar * angle.cos();
                let y = scalar * angle.sin();

                (x, y)
            });

            let mut first = None;
            let mut prev = points.next();
            for (x, y) in points {
                if first.is_none() {
                    first = Some((x, y));
                }

                if let Some((prev_x, prev_y)) = prev {
                    prev = Some((x, y));
                    painter.line_segment(
                        [
                            rect.center() + vec2(prev_x, -prev_y),
                            rect.center() + vec2(x, -y),
                        ],
                        Stroke::new(2.0, Color32::LIGHT_RED),
                    );
                }
            }

            if let (Some(a), Some(b)) = (first, prev) {
                painter.line_segment(
                    [
                        rect.center() + vec2(a.0, -a.1),
                        rect.center() + vec2(b.0, -b.1),
                    ],
                    Stroke::new(2.0, Color32::LIGHT_RED),
                );
            }
        }

        let x = self.x.clamp(-1.0, 1.0) * radius;
        let y = -self.y.clamp(-1.0, 1.0) * radius;

        painter.circle_filled(rect.center() + vec2(x, y), 2.0, Color32::LIGHT_BLUE);

        response
    }
}
