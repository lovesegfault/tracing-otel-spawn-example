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

struct MetricsGuard {

}

impl Drop for MetricsGuard {
    fn drop(&mut self) {
        // 1. Combine the logs for this run into `logs/run_id/combined-fake-emf.json` or w/e
        // 2. Call the uploader
    }
}

fn main() -> anyhow::Result<()> {
    let log_files = &[
        "logs/018fdf43-7c5f-7bc9-9969-82646838d255/child-018fdf43-7c7c-7323-a403-16aa9f1fd2cc.json",
        "logs/018fdf43-7c5f-7bc9-9969-82646838d255/grandchild-018fdf43-7e82-7e72-9afa-3e6fd52b8edf.json",
        "logs/018fdf43-7c5f-7bc9-9969-82646838d255/parent-018fdf43-7c5f-785c-898a-0db99cec4f6d.json",
        "logs/018fdf43-7c5f-7bc9-9969-82646838d255/parent-018fdf43-7c6f-7523-a89b-b82be5de4482.json",
    ];

    let mut combined_fake_emf = FakeEmfJson {
        foobar_name: "cargo-foobar".into(),
        _foobar: FoobarFakeEmf {
            timestamp: 0,
            telemetry: Vec::new(),
        },
    };

    for log in log_files {
        // ResourceSpan
        let json: serde_json::Value = serde_json::de::from_reader(File::open(&log)?)?;
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
    }

    let mut out = File::create("combined-fake-emf.json")?;
    serde_json::ser::to_writer_pretty(&mut out, &combined_fake_emf)?;

    Ok(())
}
