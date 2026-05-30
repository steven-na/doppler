pub fn print_help(cmd: &str, desc: &str) {
    println!("\"{cmd}\"\n\t{desc}");
}

pub fn truncate_string_and_add_suffix(inp: &str, max_len: usize, suffix: Option<&str>) -> String {
    if inp.len() <= max_len {
        inp.to_string()
    } else {
        let suffix = suffix.unwrap_or("...");
        let mut truncated = inp.chars().take(max_len - suffix.len()).collect::<String>();
        truncated.truncate(truncated.trim_end().len());
        truncated.push_str(suffix);
        truncated
    }
}
