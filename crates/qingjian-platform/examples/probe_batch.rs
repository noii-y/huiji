//! 批量排序探针：一条连接、每个用例开独立 Session（互不学习/不串上下文），
//! 读 TSV（每行一个拼音串），输出「拼音 <TAB> 首选 <TAB> 前5」。用法：probe_batch <管道> <tsv路径>
use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;

use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, ServerMessage, SessionId,
    read_message, write_message,
};

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

fn recv(stream: &mut std::fs::File) -> ServerMessage {
    read_message(stream).expect("读应答失败").expect("对端关闭")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or(r"\\.\pipe\qingjian-1".to_string());
    let tsv = args.next().expect("需要 TSV 路径");
    let cases: Vec<String> = std::fs::read_to_string(tsv)
        .expect("读 TSV 失败")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect();

    let mut stream = open_pipe(&name).expect("打开管道失败");
    for (i, typed) in cases.iter().enumerate() {
        let session = SessionId(5000 + i as u64);
        write_message(
            &mut stream,
            &ClientMessage::OpenSession {
                session,
                app: Some("probe-batch".to_owned()),
                protocol: PROTOCOL_VERSION,
            },
        )
        .unwrap();
        let _ = recv(&mut stream);
        let mut last: Vec<String> = Vec::new();
        for c in typed.chars() {
            let event = KeyEvent::new(vk_of(c), Some(c), KeyModifiers::default());
            write_message(&mut stream, &ClientMessage::Key { session, event }).unwrap();
            if let ServerMessage::KeyResult { frame, .. } = recv(&mut stream) {
                last = frame
                    .candidates
                    .items
                    .iter()
                    .take(5)
                    .map(|x| x.text.clone())
                    .collect();
            }
        }
        let first = last.first().cloned().unwrap_or_default();
        println!("{typed}\t{first}\t{}", last.join(" / "));
    }
}
