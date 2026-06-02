use std::io;

pub fn print_help(cmd: &str, desc: &str) {
    println!("\"{cmd}\"\n\t{desc}");
}

pub fn truncate_string_and_add_suffix(inp: &str, max_len: usize, suffix: Option<&str>) -> String {
    if inp.len() <= max_len {
        inp.to_string()
    } else {
        let suffix = suffix.unwrap_or("...");
        let mut truncated = inp
            .chars()
            .take(max_len.saturating_sub(suffix.len()))
            .collect::<String>();
        truncated.truncate(truncated.trim_end().len());
        truncated.push_str(suffix);
        truncated
    }
}

pub fn seek_bar_string(cur: u32, max: u32, width: u32) -> io::Result<String> {
    let mut cur = cur;
    if cur > max {
        cur = max;
    }

    let bar_width = width.saturating_sub(2);
    let pos = (cur as f64 / max as f64) * bar_width as f64;
    let pos = pos as u32;
    let mut bar = String::from("|");

    for _ in 0..pos {
        bar.push('─');
    }
    bar.push('●');
    for _ in pos..bar_width {
        bar.push('─');
    }
    bar.push('|');

    Ok(bar)
}
