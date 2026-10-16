//! 冷启动字词评测：独立内存学习器、不上屏、不写个人数据，原候选逐条保留。

mod entry;
pub mod error;

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use qingjian_core::Engine;
use serde_json::json;

use entry::Entry;
use error::ColdError;

pub fn run(engine: &mut Engine, input: &Path, output: &Path) -> Result<(), ColdError> {
    let entries: Vec<Entry> = std::fs::read_to_string(input)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let mut ids = HashSet::new();
    for row in &entries {
        if row.text.is_empty()
            || row.keys.is_empty()
            || !row
                .keys
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '\'')
            || !ids.insert(&row.id)
        {
            return Err(ColdError::Invalid(row.id.clone()));
        }
    }
    let dictionaries = std::iter::once(engine.dictionary()).chain(engine.extra_dictionaries());
    let mut readings: HashMap<String, HashSet<String>> = HashMap::new();
    for dictionary in dictionaries {
        for word in dictionary.entries() {
            readings
                .entry(word.text.to_owned())
                .or_default()
                .insert(word.pinyin.replace(' ', ""));
        }
    }
    let mut output = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
    );
    for (index, entry) in entries.iter().enumerate() {
        engine.clear();
        engine.break_chain();
        engine.history_mut().clear();
        engine.set_rescoring_context(None);
        engine.set_input(&entry.keys);
        let start = Instant::now();
        let query = engine.query();
        let engine_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (baseline, candidates, query_error) = match query {
            Ok(query) => (
                query
                    .candidates
                    .items
                    .iter()
                    .map(|c| c.text.clone())
                    .collect::<Vec<_>>(),
                serde_json::to_value(&query.candidates.items)?,
                None,
            ),
            Err(error) => (Vec::new(), json!([]), Some(error.to_string())),
        };
        let keys = entry.keys.replace('\'', "");
        let present = readings.get(&entry.text);
        let row = json!({
            "entry": entry, "dictionary_text": present.is_some(),
            "dictionary_reading": present.is_some_and(|p| p.contains(&keys)),
            "baseline": baseline, "candidate_metadata": candidates,
            "query_error": query_error,
            "engine_ms": engine_ms,
        });
        writeln!(output, "{row}")?;
        if (index + 1) % 250 == 0 {
            output.flush()?;
            tracing::info!(
                done = index + 1,
                total = entries.len(),
                "冷启动字词评测进度"
            );
        }
    }
    output.flush()?;
    println!(
        "冷启动字词评测完成：{} 条；个人数据未加载，未上屏学习",
        entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::args::Args;
    use clap::Parser;

    #[test]
    fn isolation_rejects_personal_files_and_config() {
        for flag in ["--user-dict", "--config"] {
            assert!(
                Args::try_parse_from([
                    "cli",
                    "--eval-cold",
                    "in.jsonl",
                    "--cold-output",
                    "out.jsonl",
                    flag,
                    "personal.tsv"
                ])
                .is_err()
            );
        }
    }
}
