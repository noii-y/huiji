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
    } else if c.is_ascii_lowercase() {
        0x41 + (c as u32 - 'a' as u32)
    } else {
        // 非拼音字符（理论上不该出现）：退回一个不映射字母的虚拟键，避免减法溢出
        0
    }
}

fn recv(stream: &mut std::fs::File) -> ServerMessage {
    read_message(stream).expect("读应答失败").expect("对端关闭")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or(r"\\.\pipe\qingjian-1".to_string());
    let tsv = args.next().expect("需要 TSV 路径");
    // 标准答案 TSV：每行「目标 <TAB> 拼音 [ <TAB> 上文]」。取前两列，第三列（上文）本探针不使用。
    let cases: Vec<(String, String)> = std::fs::read_to_string(tsv)
        .expect("读 TSV 失败")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('\t').collect();
            match cols.as_slice() {
                [target, typed, ..] => Some(((*target).to_owned(), (*typed).to_owned())),
                _ => None,
            }
        })
        .collect();

    let mut stream = open_pipe(&name).expect("打开管道失败");
    let mut first_hits = 0usize;
    let mut top5_hits = 0usize;
    for (i, (target, typed)) in cases.iter().enumerate() {
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
        let in_first = &first == target;
        let in_top5 = last.iter().any(|x| x == target);
        first_hits += usize::from(in_first);
        top5_hits += usize::from(in_top5);
        let mark = if in_first {
            "OK"
        } else if in_top5 {
            "TOP5"
        } else {
            "MISS"
        };
        println!(
            "[{mark}] {typed}\t目标={target}\t首选={first}\t前5={}",
            last.join(" / ")
        );
    }
    let total = cases.len().max(1);
    println!(
        "\n共 {n} 条：首选命中 {a} ({ap:.1}%)，前五命中 {b} ({bp:.1}%)",
        n = cases.len(),
        a = first_hits,
        ap = 100.0 * first_hits as f64 / total as f64,
        b = top5_hits,
        bp = 100.0 * top5_hits as f64 / total as f64,
    );
}
