//! 真机握手探针：连本会话管道，OpenSession，逐键发 nihao，打印每拍候选。
//! 用来判断 holder 是「只接受连接」还是「真的路由应答」。
//! Windows 命名管道专用：非 Windows 平台只编译一个提示入口（`std::os::windows` 不存在）。
#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(windows)]
use std::io;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[cfg(windows)]
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId, read_message, write_message,
};

#[cfg(windows)]
const SESSION: SessionId = SessionId(999);

#[cfg(windows)]
fn open_pipe(name: &str) -> io::Result<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(name)
}

#[cfg(windows)]
fn main() {
    let name = std::env::args()
        .nth(1)
        .unwrap_or(r"\\.\pipe\qingjian-1".to_string());
    let mut stream = open_pipe(&name).expect("打开管道失败");
    write_message(
        &mut stream,
        &ClientMessage::OpenSession {
            session: SESSION,
            app: Some("handshake-probe".to_string()),
            protocol: PROTOCOL_VERSION,
        },
    )
    .expect("写 OpenSession 失败");
    let opened: qingjian_platform::protocol::ServerMessage = read_message(&mut stream)
        .expect("读应答失败")
        .expect("对端关闭");
    println!("OpenSession -> {opened:?}");

    for (vk, ch) in [
        (0x4Eu32, 'n'),
        (0x49, 'i'),
        (0x48, 'h'),
        (0x41, 'a'),
        (0x4F, 'o'),
    ] {
        let event = KeyEvent::new(vk, Some(ch), KeyModifiers::default());
        write_message(
            &mut stream,
            &ClientMessage::Key {
                session: SESSION,
                event,
            },
        )
        .expect("写 Key 失败");
        let reply = read_message(&mut stream)
            .expect("读 KeyResult 失败")
            .expect("对端关闭");
        match reply {
            qingjian_platform::protocol::ServerMessage::KeyResult { frame, .. } => {
                let words: Vec<String> = frame
                    .candidates
                    .items
                    .iter()
                    .take(6)
                    .map(|c| c.text.clone())
                    .collect();
                println!("{ch}: preedit={} 候选={words:?}", frame.preedit.len());
            }
            other => println!("{ch}: 非预期应答 {other:?}"),
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("pipe_handshake 仅支持 Windows 命名管道，请在本机 Windows 上运行。");
}
