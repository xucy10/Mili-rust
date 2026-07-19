use valence_nbt::{Compound, List, Value};

pub fn load_enchantments() -> Vec<(String, Compound)> {
    let json_str = include_str!("../extracted/enchantments.json");
    let json: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(json_str).expect("failed to parse enchantments.json");

    let mut result = vec![];
    for (name, value) in json {
        let compound = json_to_compound(&value);
        result.push((format!("minecraft:{name}"), compound));
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn json_to_compound(value: &serde_json::Value) -> Compound {
    let mut compound = Compound::new();
    if let serde_json::Value::Object(map) = value {
        for (k, v) in map {
            compound.insert(k.clone(), json_to_nbt_value(v));
        }
    }
    compound
}

fn json_to_nbt_value(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Compound(Compound::new()),
        serde_json::Value::Bool(b) => Value::Byte(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Value::Int(i as i32)
                } else {
                    Value::Long(i)
                }
            } else if let Some(f) = n.as_f64() {
                Value::Double(f)
            } else {
                Value::Int(0)
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                Value::List(List::End)
            } else {
                match &arr[0] {
                    serde_json::Value::String(_) => {
                        let list: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        Value::List(List::String(list))
                    }
                    serde_json::Value::Number(_) => {
                        if arr[0].as_i64().is_some() {
                            let list: Vec<i32> = arr
                                .iter()
                                .filter_map(|v| v.as_i64().map(|i| i as i32))
                                .collect();
                            Value::List(List::Int(list))
                        } else {
                            let list: Vec<f64> = arr
                                .iter()
                                .filter_map(|v| v.as_f64())
                                .collect();
                            Value::List(List::Double(list))
                        }
                    }
                    serde_json::Value::Bool(_) => {
                        let list: Vec<i8> = arr
                            .iter()
                            .filter_map(|v| v.as_bool().map(|b| if b { 1 } else { 0 }))
                            .collect();
                        Value::List(List::Byte(list))
                    }
                    _ => {
                        let list: Vec<Compound> =
                            arr.iter().map(|v| json_to_compound(v)).collect();
                        Value::List(List::Compound(list))
                    }
                }
            }
        }
        serde_json::Value::Object(_) => Value::Compound(json_to_compound(value)),
    }
}