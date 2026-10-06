//! 真机排序验证探针：连本会话管道，OpenSession，按字符串逐键发（撇号当音节分隔键 VK 0xDE），
//! 每拍打印候选，结尾打印完整首选列表。用法：probe_input <管道> <拼音串>
use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;

use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId, read_message, write_message,
};

const SESSION: SessionId = SessionId(998);

fn open_pipe(name: &str) -> io::Result<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(name)
}

fn vk_of(c: char) -> u32 {
    if c == '\'' {
        0xDE
    } else {
        0x41 + (c as u32 - 'a' as u32)
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or(r"\\.\pipe\qingjian-1".to_string());
    let typed = args.next().expect("需要拼音串参数，如 di'y'g");
    let mut stream = open_pipe(&name).expect("打开管道失败");
    write_message(
        &mut stream,
        &ClientMessage::OpenSession {
            session: SESSION,
            app: Some("probe-input".to_string()),
            protocol: PROTOCOL_VERSION,
        },
    )
    .expect("写 OpenSession 失败");
    let opened: qingjian_platform::protocol::ServerMessage = read_message(&mut stream)
        .expect("读应答失败")
        .expect("对端关闭");
    println!("OpenSession -> {opened:?}");

    for c in typed.chars() {
        let event = KeyEvent::new(vk_of(c), Some(c), KeyModifiers::default());
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
        if let qingjian_platform::protocol::ServerMessage::KeyResult { frame, .. } = reply {
            let words: Vec<String> = frame
                .candidates
                .items
                .iter()
                .take(10)
                .map(|x| x.text.clone())
                .collect();
            println!("{c} -> {words:?}");
        }
    }
}
