use playervf::lyrics::parse_payload;
use std::{hint::black_box, time::Instant};
fn main() {
    let source = (0..1000)
        .map(|i| {
            format!(
                "[{:02}:{:02}.000]<{:02}:{:02}.000>First <{:02}:{:02}.500>second",
                i / 60,
                i % 60,
                i / 60,
                i % 60,
                i / 60,
                i % 60
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let start = Instant::now();
    for _ in 0..50 {
        black_box(parse_payload(black_box(&source)));
    }
    let parse_ms = start.elapsed().as_secs_f64() * 1000.;
    let lines = (0..1000)
        .map(|i| (i * 1000i64, i * 1000i64 + 800))
        .collect::<Vec<_>>();
    let start = Instant::now();
    for i in 0..50000 {
        black_box(
            lines
                .iter()
                .enumerate()
                .filter(|(_, (s, e))| *s <= i * 20 && i * 20 < *e)
                .map(|(i, _)| i)
                .last(),
        );
    }
    let scan_ms = start.elapsed().as_secs_f64() * 1000.;
    let index = playervf::braccato::engine::Timeline::from_spans(lines.iter().copied());
    let mut active = Vec::new();
    let start = Instant::now();
    for i in 0..50000 {
        index.active_into(black_box(i * 20), &mut active);
        black_box(&active);
    }
    let indexed_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut text = playervf::braccato::render::PreparedText::new([playervf::WordRow {
        text: "Dormant lyric".into(),
        start_ms: 1000,
        end_ms: 2000,
        ..Default::default()
    }]);
    let fg = slint::Color::from_rgb_u8(255, 255, 255);
    let bg = slint::Color::from_rgb_u8(90, 90, 90);
    let start = Instant::now();
    for i in 0..50000 {
        black_box(text.render(i, false, fg, bg, 700, false, false));
    }
    let dormant_ms = start.elapsed().as_secs_f64() * 1000.;
    let result = serde_json::json!({"profile":"debug, same workstation; microbenchmark, not end-to-end FPS","lines":1000,"parse_runs":50,"ticks":50000,"before":{"parser_ms":463.799,"scan_ms":520.183},"after":{"parser_ms":parse_ms,"linear_scan_ms":scan_ms,"indexed_lookup_ms":indexed_ms,"dormant_render_ms":dormant_ms,"dormant_rebuilds":text.updates()},"lookup_speedup_vs_same_run_scan":scan_ms/indexed_ms});
    std::fs::write(
        "docs/braccato-performance.json",
        serde_json::to_string_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
