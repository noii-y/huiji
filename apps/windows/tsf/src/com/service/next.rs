//! 按键转发给 Server 之后要做的事：Server 交互在引擎借用里做完，放掉借用再按这个枚举走编辑会话。

/// 一次按键转发给 Server 后、放掉引擎借用要做的事。
pub(super) enum Next {
    /// 把上屏文本 / 组句拼音行写进文档。
    Document {
        /// 本次要立即上屏的文本。
        commit: Option<String>,

        /// 组句拼音行；空串表示收组句。
        preedit: String,

        /// 这个键吃不吃。
        consumed: bool,
    },

    /// 「翻译选中文字」：起异步只读会话读当前选区。
    ReadSelection {
        /// 请求标识，回给 Server 对上是哪一次询问。
        request: u64,
    },

    /// 转发出错、已断连：有界等待重连后重发本键；等不到再放行。
    Reconnect,

    /// Server 无响应（请求超时），已重启 Server：本键按「连不上 Server」兜底（拼音字母吃掉、其余放行）。
    Restarted,
}
