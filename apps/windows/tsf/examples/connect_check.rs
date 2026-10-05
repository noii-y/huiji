//! 真机自检：用 DLL 自己的 `connect_default()` 连当前会话的 Server（管道名由 DLL 内部按会话 id 拼），
//! 再开一个会话确认能拿到 SessionOpened。跑在哪个会话，就该连到那个会话的 Server。
fn main() {
    let stream = qingjian_tsf::client::pipe::connect_default()
        .expect("connect_default 连不上：DLL 算出的管道名对不上 Server");
    let (client, input) = qingjian_tsf::client::EngineClient::open(
        stream,
        qingjian_platform::protocol::SessionId(998),
        Some("connect-check".to_string()),
    )
    .expect("OpenSession 失败");
    println!(
        "OK：DLL 已连上本会话 Server，english_mode={:?}",
        input.english_mode
    );
    client.close().ok();
}
