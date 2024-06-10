use std::{
    fs::{File, OpenOptions},
    io::prelude::*,
};

use anyhow::Context;
use opentelemetry_stdout::SpanData;

// struct FakeEmfWriter {
//     name: String,
//     wrote_header: bool,
//     inner: File,
// }
//
// impl FakeEmfWriter {
//     pub fn new(name: &str, file: File) -> anyhow::Result<Self> {
//         let wrote_header = false;
//         let inner = file;
//         Ok(Self {
//             name: name.to_string(),
//             wrote_header,
//             inner,
//         })
//     }
// }
//
// impl Write for FakeEmfWriter {
//     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//         if !self.wrote_header {
//             let header
//             todo!()
//         }
//         todo!()
//     }
//
//     fn flush(&mut self) -> std::io::Result<()> {
//         todo!()
//     }
// }

#[derive(serde::Serialize)]
struct FakeEmfJson {
    #[serde(rename = "foobarName")]
    foobar_name: String,
    _foobar: FoobarFakeEmf,
}

#[derive(serde::Serialize)]
struct FoobarFakeEmf {
    // TODO: Fix this type, its ms since epoch
    #[serde(rename = "Timestamp")]
    timestamp: u64,
    telemetry: Vec<serde_json::Value>,
}

struct MetricsGuard {}

impl Drop for MetricsGuard {
    fn drop(&mut self) {
        // 1. Combine the logs for this run into `logs/run_id/combined-fake-emf.json` or w/e
        // 2. Call the uploader
    }
}

fn main() -> anyhow::Result<()> {
    let log_files = &["logs/test.jsonl"];

    let mut combined_fake_emf = FakeEmfJson {
        foobar_name: "cargo-foobar".into(),
        _foobar: FoobarFakeEmf {
            timestamp: 0,
            telemetry: Vec::new(),
        },
    };

    for log in log_files {
        // ResourceSpan
        let json_lines = serde_json::Deserializer::from_reader(File::open(&log)?)
            .into_iter::<serde_json::Value>();
        let mut out = File::create("combined-fake-emf.jsonl")?;
        for line in json_lines {
            let json = line?;
            let resource_spans = json
                .as_object()
                .context("json not an obj")?
                .get("resourceSpans")
                .context("no resourceSpans in json")?
                .as_array()
                .context("resourceSpans is not an array")?;
            combined_fake_emf
                ._foobar
                .telemetry
                .extend_from_slice(&resource_spans);
            serde_json::ser::to_writer(&mut out, &combined_fake_emf)?;
            out.write_all(b"\n")?;
        }
    }

    Ok(())
}
