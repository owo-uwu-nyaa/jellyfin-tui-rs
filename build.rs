fn main() {
    println!("cargo::rerun-if-env-changed=CARGO_CFG_FEATURE");
    let features = std::env::var("CARGO_CFG_FEATURE").expect("should be set by cargo");
    let features: Vec<_> = features.split(",").collect();
    let features = if features.is_empty() {
        "∅".to_string()
    } else {
        features.join(", ")
    };
    println!("cargo::rustc-env=JELLYFIN_TUI_FEATURES={features}");
}
