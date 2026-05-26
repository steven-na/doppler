pub fn seconds_to_base60_string(s: u32) -> String {
    if s >= 60 * 60 {
        let mut minutes = s / 60;
        while minutes > 60 {
            minutes -= 60;
        }
        format!("{}:{:02}:{:02}", s / (60 * 60), minutes, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
