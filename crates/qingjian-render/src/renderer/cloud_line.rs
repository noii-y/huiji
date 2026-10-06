//! 中文整句纠错行：云朵 + 句子，画在拼音行下方，按 Tab 上屏。

use super::{Metrics, Renderer};
use crate::canvas::Canvas;
use crate::frame::Frame;

impl Renderer {
    /// 中文纠错行需要的宽高；没有 `sentence` 时都是 0。
    pub(super) fn cloud_line_size(&mut self, frame: &Frame, m: &Metrics) -> (f32, f32) {
        let Some(sentence) = &frame.sentence else {
            return (0.0, 0.0);
        };
        let style = m.annotation_style(m.theme.colors.cloud);
        let width = m.cloud_width() + self.measure(sentence, &style).width;
        (width, style.line_height + m.row_padding() * 2.0)
    }

    /// 画云朵 + 中文纠错句，返回占用高度。`left` 是内容区左边。
    pub(super) fn draw_cloud_line(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        left: f32,
        y: f32,
    ) -> f32 {
        let Some(sentence) = &frame.sentence else {
            return 0.0;
        };
        let line_height = m.px(m.theme.annotation_font.line_height);
        let top = y + m.row_padding();
        let mut x = left + m.padding();
        x += self.draw_cloud(canvas, m, x, top, line_height);
        let style = m.annotation_style(m.theme.colors.cloud);
        self.draw_text(canvas, sentence, &style, x, top);
        line_height + m.row_padding() * 2.0
    }
}
