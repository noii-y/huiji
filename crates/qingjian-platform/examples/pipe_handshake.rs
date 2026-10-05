//! 真机握手探针：连本会话管道，OpenSession，逐键发 nihao，打印每拍候选。
//! 用来判断 holder 是「只接受连接」还是「真的路由应答」。
use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;

use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId, read_message, write_message,
};

const SESSION: SessionId = SessionId(999);

fn open_pipe(name: &str) -> io::Result<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(name)
}

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
