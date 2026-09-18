/// Macros for expert definition and registration.
///
/// `define_expert!` creates an Expert from a JSON weights file at compile time.
/// `expert_registry!` initializes the global ExpertRegistry with all registered experts.

#[macro_export]
macro_rules! define_expert {
    ($name:expr, $domain:expr, $weights_json:expr) => {{
        use $crate::expert_framework::{Expert, ExpertWeights};
        use std::collections::HashMap;

        let json: serde_json::Value = serde_json::from_str($weights_json)
            .expect(&format!("Failed to parse expert weights JSON for {}", $name));

        let weights_vec = json["weights"]
            .as_array()
            .expect("Missing 'weights' field")
            .iter()
            .map(|row| {
                row.as_array()
                    .expect("Each weight row must be an array")
                    .iter()
                    .map(|w| w.as_i64().expect("Weight must be i32") as i32)
                    .collect::<Vec<i32>>()
            })
            .collect::<Vec<Vec<i32>>>();

        let biases = json["biases"]
            .as_array()
            .expect("Missing 'biases' field")
            .iter()
            .map(|b| b.as_i64().expect("Bias must be i32") as i32)
            .collect::<Vec<i32>>();

        let hidden_dim = json["hidden_dim"].as_u64().unwrap_or(0) as usize;
        let scale_shift = json["scale_shift"].as_u64().unwrap_or(8) as u32;

        let weights_2 = if let Some(w2) = json.get("weights_2") {
            Some(
                w2.as_array()
                    .expect("weights_2 must be an array")
                    .iter()
                    .map(|row| {
                        row.as_array()
                            .expect("Each weight row must be an array")
                            .iter()
                            .map(|w| w.as_i64().expect("Weight must be i32") as i32)
                            .collect::<Vec<i32>>()
                    })
                    .collect::<Vec<Vec<i32>>>(),
            )
        } else {
            None
        };

        let biases_2 = if let Some(b2) = json.get("biases_2") {
            Some(
                b2.as_array()
                    .expect("biases_2 must be an array")
                    .iter()
                    .map(|b| b.as_i64().expect("Bias must be i32") as i32)
                    .collect::<Vec<i32>>(),
            )
        } else {
            None
        };

        Expert {
            name: $name,
            domain: $domain,
            weights: ExpertWeights {
                weights: weights_vec,
                biases,
                hidden_dim,
                weights_2,
                biases_2,
                scale_shift,
            },
        }
    }};
}

#[macro_export]
macro_rules! expert_registry {
    (
        $($name:expr => $domain:expr),* $(,)?
    ) => {{
        use $crate::expert_framework::{ExpertRegistry, ExpertDomain};

        let mut registry = ExpertRegistry::new();
        // Registration happens via separate API calls, not macro expansion
        // (weights must be loaded at runtime from files, not compile-time embedded)
        registry
    }};
}

// Helper to load expert from file at runtime
pub fn load_expert_from_json(
    name: &'static str,
    domain: $crate::expert_framework::ExpertDomain,
    json_path: &std::path::Path,
) -> Result<$crate::expert_framework::Expert, Box<dyn std::error::Error>> {
    let json_str = std::fs::read_to_string(json_path)?;
    let json: serde_json::Value = serde_json::from_str(&json_str)?;

    let weights_vec = json["weights"]
        .as_array()
        .ok_or("Missing 'weights' field")?
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or("Each weight row must be an array")
                .and_then(|arr| {
                    Ok(arr
                        .iter()
                        .map(|w| w.as_i64().ok_or("Weight must be i32"))
                        .collect::<Result<Vec<_>, _>>()?)
                })
                .map(|v| v.into_iter().map(|w| w as i32).collect::<Vec<i32>>())
        })
        .collect::<Result<Vec<Vec<i32>>, _>>()?;

    let biases = json["biases"]
        .as_array()
        .ok_or("Missing 'biases' field")?
        .iter()
        .map(|b| b.as_i64().ok_or("Bias must be i32").map(|v| v as i32))
        .collect::<Result<Vec<i32>, _>>()?;

    let hidden_dim = json["hidden_dim"].as_u64().unwrap_or(0) as usize;
    let scale_shift = json["scale_shift"].as_u64().unwrap_or(8) as u32;

    let weights_2 = if let Some(w2) = json.get("weights_2") {
        Some(
            w2.as_array()
                .ok_or("weights_2 must be an array")?
                .iter()
                .map(|row| {
                    row.as_array()
                        .ok_or("Each weight row must be an array")
                        .and_then(|arr| {
                            Ok(arr
                                .iter()
                                .map(|w| w.as_i64().ok_or("Weight must be i32"))
                                .collect::<Result<Vec<_>, _>>()?)
                        })
                        .map(|v| v.into_iter().map(|w| w as i32).collect::<Vec<i32>>())
                })
                .collect::<Result<Vec<Vec<i32>>, _>>()?,
        )
    } else {
        None
    };

    let biases_2 = if let Some(b2) = json.get("biases_2") {
        Some(
            b2.as_array()
                .ok_or("biases_2 must be an array")?
                .iter()
                .map(|b| b.as_i64().ok_or("Bias must be i32").map(|v| v as i32))
                .collect::<Result<Vec<i32>, _>>()?,
        )
    } else {
        None
    };

    Ok($crate::expert_framework::Expert {
        name,
        domain,
        weights: $crate::expert_framework::ExpertWeights {
            weights: weights_vec,
            biases,
            hidden_dim,
            weights_2,
            biases_2,
            scale_shift,
        },
    })
}
