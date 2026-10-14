//! 中文整句纠错行：云朵 + 句子，画在拼音行下方。
//! 未选中时是一行小字提示（按 Tab 选中）；选中后它就是首位候选，按候选行的高亮样式画，空格 / 回车上屏。

use super::{Metrics, Renderer};
use crate::canvas::Canvas;
use crate::frame::Frame;

impl Renderer {
    /// 中文纠错行需要的宽高；没有 `sentence` 时都是 0。
    pub(super) fn cloud_line_size(&mut self, frame: &Frame, m: &Metrics) -> (f32, f32) {
        let Some(sentence) = &frame.sentence else {
            return (0.0, 0.0);
        };
        if frame.sentence_selected {
            // 选中态：一行候选行的高度（序号 + 云朵 + 整句）。
            let text_height = m.px(m.theme.text_font.line_height);
            let mut width = m.padding()
                + self.measure("1", &m.index_style()).width
                + m.column_gap()
                + m.cloud_width()
                + self.measure(sentence, &m.text_style()).width
                + m.padding();
            // 末尾留出「空格上屏」提示的位置。
            let hint = m.annotation_style(m.theme.colors.gloss);
            width += m.column_gap() + self.measure("空格上屏", &hint).width;
            (width, text_height + m.row_padding() * 2.0)
        } else {
            let style = m.annotation_style(m.theme.colors.cloud);
            let width = m.cloud_width() + self.measure(sentence, &style).width;
            (width, style.line_height + m.row_padding() * 2.0)
        }
    }

    /// 画云朵 + 中文纠错句，返回占用高度。`left` 是内容区左边，`content_width` 是内容区宽度。
    pub(super) fn draw_cloud_line(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        left: f32,
        y: f32,
        content_width: f32,
    ) -> f32 {
        let Some(sentence) = &frame.sentence else {
            return 0.0;
        };

        if frame.sentence_selected {
            // 选中态：高亮成首位候选，序号 1、云朵、整句，右侧提示「空格上屏」。
            let text_height = m.px(m.theme.text_font.line_height);
            let height = text_height + m.row_padding() * 2.0;
            self.fill_highlight(
                canvas,
                m,
                left + m.padding() / 2.0,
                y,
                content_width - m.padding(),
                height,
            );
            let top = y + m.row_padding();
            let small_offset = m.small_offset(text_height);
            let mut x = left + m.padding();
            x += self.draw_text(canvas, "1", &m.index_style(), x, top + small_offset)
                + m.column_gap();
            x += self.draw_cloud(canvas, m, x, top, text_height);
            let style = m.style(m.theme.text_font, m.theme.colors.cloud).bold();
            self.draw_text(canvas, sentence, &style, x, top);
            let hint = m.annotation_style(m.theme.colors.gloss);
            let hint_x = left + content_width - m.padding() - self.measure("空格上屏", &hint).width;
            self.draw_text(canvas, "空格上屏", &hint, hint_x, top + small_offset);
            return height;
        }

        // 未选中：云朵 + 灰色小字提示。
        let line_height = m.px(m.theme.annotation_font.line_height);
        let top = y + m.row_padding();
        let mut x = left + m.padding();
        x += self.draw_cloud(canvas, m, x, top, line_height);
        let style = m.annotation_style(m.theme.colors.cloud);
        self.draw_text(canvas, sentence, &style, x, top);
        line_height + m.row_padding() * 2.0
    }
}
