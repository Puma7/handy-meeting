//! Synthetic, opt-in benchmark. Creates an isolated fixture; never opens user sessions.
use meeting_core::{library::LibraryIndex, metadata::Metadata, SessionData};
use std::{fs, path::PathBuf, time::Instant};
fn main() -> meeting_core::Result<()> {
    let target = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide a NEW synthetic benchmark folder")?,
    );
    if target.exists() {
        return Err("Benchmark requires a new folder; existing files are preserved".into());
    }
    fs::create_dir_all(&target)?;
    let sessions = target.join("sessions");
    fs::create_dir(&sessions)?;
    let started = Instant::now();
    let mut index = LibraryIndex::open(&target)?;
    for batch in 0..200 {
        index.transaction_batch(|index| {
            for number in batch*500..(batch+1)*500 {
                let id = format!("{number:032x}");
                let data: SessionData = serde_json::from_value(serde_json::json!({
                    "schemaVersion":2,"id":id,"title":format!("Synthetic meeting {number}"),
                    "startedAt":"2026-10-27T06:00:00Z","status":"complete","modelId":"synthetic-benchmark",
                    "durationMs":60000,"parts":[{"file":"audio/system-0.wav","source":"system",
                    "startSample":0,"sampleCount":960000,"confirmedSampleCount":960000,
                    "text":format!("Synthetic evidence report{number} Prüfbericht. No user audio or model inference."),"status":"complete"}]
                }))?;
                let folder = sessions.join(&id);
                fs::create_dir(&folder)?;
                fs::write(folder.join("session.json"),serde_json::to_vec(&data)?)?;
                index.upsert(&data,&Metadata::default(),"synthetic-fixture")?;
            }
            Ok(())
        })?;
        if batch % 20 == 19 {
            eprintln!("{} synthetic meetings indexed", (batch + 1) * 500);
        }
    }
    let build_ms = started.elapsed().as_millis();
    drop(index);
    let started = Instant::now();
    let index = LibraryIndex::open(&target)?;
    let first = index.search("report99999", 0, 25)?;
    assert_eq!(first.total, 1);
    assert_eq!(
        first.items[0].source_id.as_deref(),
        Some("audio/system-0.wav")
    );
    let reopened_query_ms = started.elapsed().as_micros() as f64 / 1000.0;
    let mut queries = Vec::new();
    for n in 0..100 {
        let started = Instant::now();
        let page = index.search(&format!("report{}", n * 997), 0, 25)?;
        assert!(!page.items.is_empty());
        queries.push(started.elapsed().as_micros() as f64 / 1000.0);
    }
    queries.sort_by(f64::total_cmp);
    let started = Instant::now();
    let history = index.search("", 99975, 25)?;
    assert_eq!(history.total, 100000);
    assert_eq!(history.items.len(), 25);
    let history_ms = started.elapsed().as_micros() as f64 / 1000.0;
    let started = Instant::now();
    let broad = index.search("Prüfbericht", 0, 25)?;
    assert_eq!(broad.total, 100000);
    let broad_ms = started.elapsed().as_micros() as f64 / 1000.0;
    let report = serde_json::json!({"synthetic":true,"version":env!("CARGO_PKG_VERSION"),
      "meetings":100000,"manifests":100000,"audioFiles":0,"buildMs":build_ms,
      "reopenedQueryMs":reopened_query_ms,"warmQueryMedianMs":queries[50],"warmQueryP95Ms":queries[94],
      "lastHistoryPageMs":history_ms,"broadSearchMs":broad_ms,"queryCount":100,
      "limits":"Synthetic single-section meetings, no audio/model/UI; reopened DB is not an OS cold-cache measurement; timings are observations, not retrospectively chosen acceptance thresholds."});
    fs::write(
        target.join("benchmark.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
