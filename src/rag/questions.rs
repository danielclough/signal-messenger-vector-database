pub fn get_questions() -> Vec<String> {
    vec![
        "Why not load an CSV next time?",
    ].iter().map(|x| x.to_string()).collect()
}
