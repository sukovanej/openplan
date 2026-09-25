use op_diagram_render::{Weight, line_metrics, text_width};

const SIZE: f32 = 14.0;

#[test]
fn text_widths() {
    insta::glob!("labels/*.txt", |path| {
        let labels = std::fs::read_to_string(path).unwrap();
        let rows: Vec<String> = labels
            .lines()
            .map(|label| {
                format!(
                    "{:>9.3} {:>9.3}  {label}",
                    text_width(label, SIZE, Weight::Regular),
                    text_width(label, SIZE, Weight::SemiBold),
                )
            })
            .collect();
        insta::assert_snapshot!(format!(
            "  regular  semibold  at {SIZE} px\n{}",
            rows.join("\n")
        ));
    });
}

#[test]
fn line_metrics_by_size() {
    let rows: Vec<String> = [11.0, 12.0, 14.0, 16.0]
        .into_iter()
        .map(|size| {
            let metrics = line_metrics(size);
            format!(
                "{size:>4} px  ascent {:.3}  descent {:.3}",
                metrics.ascent, metrics.descent
            )
        })
        .collect();
    insta::assert_snapshot!(rows.join("\n"));
}
