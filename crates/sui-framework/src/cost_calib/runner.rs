// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, io::BufWriter, path::Path};

use move_cli::base::test::UnitTestResult;

use move_package::BuildConfig;
use move_unit_test::UnitTestingConfig;

use crate::natives;

const MAX_UNIT_TEST_INSTRUCTIONS: u64 = 1_000_000_000;

#[derive(Debug)]
pub struct CalibTestResult {
    pub name: String,
    pub baseline: f32,
    pub subject: f32,
}

pub fn run_calib(runs: usize) -> HashMap<String, (Vec<(f32, f32)>, f32)> {
    let res = run_calib_tests(None, runs);

    res.into_iter()
        .map(|q| (q.0, (q.1.clone(), summarize_values(&q.1))))
        .collect()
}
fn summarize_values(v: &Vec<(f32, f32)>) -> f32 {
    // Use average for now
    // TODO: investigate other methods
    v.iter().map(|a| a.0 - a.1).sum::<f32>() / v.len() as f32
}

pub fn run_calib_tests(
    config: Option<UnitTestingConfig>,
    runs: usize,
) -> HashMap<String, Vec<(f32, f32)>> {
    use sui_types::{MOVE_STDLIB_ADDRESS, SUI_FRAMEWORK_ADDRESS};

    let pkg_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/sui-framework/sources");

    let config = config
        .unwrap_or_else(|| UnitTestingConfig::default_with_bound(Some(MAX_UNIT_TEST_INSTRUCTIONS)));

    let mut out_map = HashMap::new();

    for _ in 0..runs {
        let config = config.clone();
        let buf = Vec::new();
        let mut test_output_buf = BufWriter::new(buf);
        if move_cli::base::test::run_move_unit_tests(
            &pkg_path,
            BuildConfig::default(),
            UnitTestingConfig {
                report_stacktrace_on_abort: true,
                report_statistics: true,
                filter: Some("calibrate".to_string()),
                num_threads: 1,
                ..config
            },
            natives::all_natives(MOVE_STDLIB_ADDRESS, SUI_FRAMEWORK_ADDRESS),
            false,
            &mut test_output_buf,
        )
        .unwrap()
            == UnitTestResult::Failure
        {
            panic!("Calibration unit test failed");
        };

        let out = extract_calib(String::from_utf8(test_output_buf.into_inner().unwrap()).unwrap());

        out.iter().for_each(|q| {
            out_map
                .entry(q.name.clone())
                .or_insert(vec![])
                .push((q.subject, q.baseline));
        });
    }

    out_map
}

pub fn extract_calib(s: String) -> Vec<CalibTestResult> {
    let lines = s.split('\n').filter(|x| x.starts_with("│ 0x2::"));

    let mut mp = HashMap::new();

    lines.for_each(|x| {
        let tokens: Vec<_> = x.split('│').collect();
        let name = tokens[1]
            .trim()
            .to_owned()
            .split("test_calibrate_")
            .nth(1)
            .unwrap()
            .to_owned();
        let val = tokens[2].trim().parse::<f32>().unwrap();
        mp.insert(name, val);
    });

    let mut ret = vec![];

    let mut mp_clone = mp.clone();

    for (name, val) in &mp {
        let name = name.to_owned();
        let name_baseline = name.clone() + "__baseline";

        if mp.contains_key(&name_baseline) {
            // Remove pair from the map
            mp_clone.remove(&name);
            mp_clone.remove(&name_baseline);

            ret.push(CalibTestResult {
                name,
                baseline: mp[&name_baseline],
                subject: *val,
            });
        }
    }

    // Data without baseline
    mp_clone.iter().for_each(|(name, val)| {
        ret.push(CalibTestResult {
            name: name.to_string(),
            baseline: 0.0,
            subject: *val,
        })
    });

    ret
}
