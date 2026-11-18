use serde_json::{Value, json};

pub const MIN_Y: i32 = -512;
pub const HEIGHT: i32 = 1024;
pub const GROUND_Y: i32 = -400;
pub const PEAK_Y: i32 = 400;

pub use pumpkin_util::y_offset::extreme_overworld_y as map_y;

pub fn source_y_density() -> Value {
    add(
        gradient(MIN_Y, GROUND_Y, -64.0, 63.0),
        gradient(GROUND_Y, PEAK_Y, 0.0, 256.0),
    )
}

fn gradient(from_y: i32, to_y: i32, from_value: f64, to_value: f64) -> Value {
    json!({
        "type": "minecraft:y_clamped_gradient",
        "from_y": from_y, "to_y": to_y, "from_value": from_value, "to_value": to_value,
    })
}

fn add(left: Value, right: Value) -> Value {
    json!({"type": "minecraft:add", "left": left, "right": right})
}

pub fn map_density_y(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            let kind = fields
                .get("type")
                .and_then(Value::as_str)
                .map(|kind| kind.strip_prefix("minecraft:").unwrap_or(kind));
            let axes = match kind {
                Some("y_clamped_gradient") => Some(("from_y", "to_y")),
                Some("gradient")
                    if matches!(
                        fields.get("axis").and_then(Value::as_str),
                        None | Some("y" | "Y")
                    ) && matches!(
                        fields.get("tiling").and_then(Value::as_str),
                        None | Some("clamp_to_edge")
                    ) =>
                {
                    Some(("from_coordinate", "to_coordinate"))
                }
                _ => None,
            };
            if let Some((from, to)) = axes
                && let (Some(a), Some(b), Some(start), Some(end)) = (
                    fields.get(from).and_then(Value::as_i64),
                    fields.get(to).and_then(Value::as_i64),
                    fields.get("from_value").and_then(Value::as_f64),
                    fields.get("to_value").and_then(Value::as_f64),
                )
            {
                *value = map_gradient(a as i32, b as i32, start, end);
                return;
            }
            for (key, value) in fields {
                if key == "lower_bound" {
                    if let Some(y) = value.as_i64() {
                        *value = json!(map_y(y as i32));
                    }
                } else {
                    map_density_y(value);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(map_density_y),
        _ => {}
    }
}

fn map_gradient(a: i32, b: i32, start: f64, end: f64) -> Value {
    let (low, high, start, end) = if a <= b {
        (a, b, start, end)
    } else {
        (b, a, end, start)
    };
    if start == end || low == high {
        return json!(start);
    }
    let step = (end - start) / f64::from(high - low);
    let mut last = low;
    let mut result = json!(start);
    for next in [-64, 63, 319]
        .into_iter()
        .filter(|&y| low < y && y < high)
        .chain([high])
    {
        let from = map_y(last);
        let to = map_y(next).max(from + 1);
        result = add(
            result,
            gradient(from, to, 0.0, f64::from(next - last) * step),
        );
        last = next;
    }
    result
}

pub fn map_height_anchors(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key == "absolute" {
                    if let Some(y) = value.as_i64() {
                        *value = json!(map_y(y as i32));
                    }
                } else {
                    map_height_anchors(value);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(map_height_anchors),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_vertical_gradient_preserves_ground_and_peak_anchors() {
        let mut value = json!({
            "type": "minecraft:gradient", "axis": "y", "from_coordinate": -64, "to_coordinate": 319,
            "from_value": 0.0, "to_value": 383.0
        });
        map_density_y(&mut value);
        let upper = &value["right"];
        let lower = &value["left"]["right"];
        assert_eq!(
            (lower["from_y"].as_i64(), lower["to_y"].as_i64()),
            (Some(-512), Some(-400))
        );
        assert_eq!(lower["to_value"].as_f64(), Some(127.0));
        assert_eq!(
            (upper["from_y"].as_i64(), upper["to_y"].as_i64()),
            (Some(-400), Some(400))
        );
        assert_eq!(upper["to_value"].as_f64(), Some(256.0));
    }
}
