//! 整句翻译探针：发一句、等一句译文、量一次端到端耗时，用来判断云端整句翻译这条链路通不通、快不快。
//!
//! 不碰候选、不进学习，只走 [`Engine::request_translation`] 与 [`Engine::poll_prediction`]。
//! 逐句串行：连续发会让前一句的结果过期被丢，所以发完一句必须等它回来再发下一句。

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use qingjian_core::Engine;

use crate::error::CliError;

/// 逐句翻译命令行直接给的句子，每句打印 `原文 → 译文 → 耗时(ms)` 的制表符行。
pub fn run(engine: &mut Engine, sentences: &[String], timeout: Duration) -> Result<(), CliError> {
    for text in sentences {
        let (translated, elapsed) = translate_one(engine, text, timeout)?;
        match translated {
            Some(sentence) => println!("{text}\t{sentence}\t{elapsed}"),
            None => println!("{text}\t（模型没给译文）\t{elapsed}"),
        }
    }
    Ok(())
}

/// 批量评测：读 TSV（`编号\t原句\t场景`）或一行一句的文本，逐句翻译，输出并可保存完整表格。
pub fn run_file(
    engine: &mut Engine,
    path: &Path,
    timeout: Duration,
    save: Option<&Path>,
) -> Result<(), CliError> {
    let content = fs::read_to_string(path)?;
    let rows = parse_rows(&content);
    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push("编号\t中文原句\t场景\t云端译文\t云端延迟(ms)".to_owned());
    for (id, sentence, scene) in rows {
        let (translated, elapsed) = translate_one(engine, &sentence, timeout)?;
        let translation = translated.unwrap_or_else(|| "（模型没给译文）".to_owned());
        lines.push(format!(
            "{id}\t{sentence}\t{scene}\t{translation}\t{elapsed}"
        ));
    }
    let table = lines.join("\n");
    println!("{table}");
    if let Some(out) = save {
        fs::write(out, &table)?;
        tracing::info!(path = %out.display(), "整句翻译结果已保存");
    }
    Ok(())
}

/// 发一句翻译、等到译文，带回译文（模型给空为 `None`）与端到端毫秒数。
fn translate_one(
    engine: &mut Engine,
    text: &str,
    timeout: Duration,
) -> Result<(Option<String>, u128), CliError> {
    let started = Instant::now();
    let sequence = engine
        .request_translation(text)
        .ok_or(CliError::TranslationUnavailable)?;
    let translated = wait_for_sentence(engine, sequence, timeout)?;
    Ok((translated, started.elapsed().as_millis()))
}

/// 轮询直到拿到这一句的译文；模型明确给空时返回 `Ok(None)`，超过 `timeout` 还没回来才报错。
fn wait_for_sentence(
    engine: &mut Engine,
    sequence: u64,
    timeout: Duration,
) -> Result<Option<String>, CliError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(prediction) = engine.poll_prediction()
            && prediction.sequence == sequence
        {
            return Ok(prediction.sentence);
        }
        if Instant::now() >= deadline {
            return Err(CliError::TranslationTimeout(timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 解析测试集：三列 TSV（编号是数字）取编号 / 原句 / 场景；纯文本按行号当编号；跳过空行与表头。
fn parse_rows(content: &str) -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 2 && !cols[0].is_empty() && cols[0].chars().all(|c| c.is_ascii_digit()) {
            rows.push((
                cols[0].to_owned(),
                cols[1].to_owned(),
                cols.get(2).copied().unwrap_or("").to_owned(),
            ));
        } else if index == 0 {
            // 首行表头
            continue;
        } else {
            rows.push(((index + 1).to_string(), line.to_owned(), String::new()));
        }
    }
    rows
}
