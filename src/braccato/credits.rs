//! Credits and singer labels, ported from @braccato/parsers/credits (MIT).
const WRITERS: &[&str] = &[
    "词",
    "詞",
    "作词",
    "作詞",
    "曲",
    "作曲",
    "词曲",
    "詞曲",
    "作词作曲",
    "作詞作曲",
    "writtenby",
    "lyricsby",
    "composedby",
    "lyricist",
    "composer",
];
const OTHER: &[&str] = &[
    "编曲",
    "編曲",
    "和声",
    "和聲",
    "混音",
    "吉他",
    "制作人",
    "製作人",
    "演唱",
    "原唱",
    "翻唱",
    "后期",
    "後期",
    "和音",
    "录音",
    "錄音",
    "策划",
    "策劃",
    "伴奏",
    "美工",
    "海报",
    "海報",
    "旁白",
    "producedby",
    "arrangedby",
    "mixing",
    "mastering",
    "vocal",
    "vocals",
    "guitar",
    "bass",
    "drums",
    "producer",
    "arranger",
];
pub fn split_names(text: &str) -> Vec<String> {
    text.split(['/', '、', '，', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
pub fn unique_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = vec![];
    for n in names {
        let n = n.trim();
        if !n.is_empty() && !out.iter().any(|s| s == n) {
            out.push(n.to_string());
        }
    }
    out
}
fn role_parts(role: &str) -> Vec<String> {
    role.to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_ascii_alphabetic())
        .collect::<String>()
        .split(['/', '、', '，', ',', '&', '／', '及', '与', '與'])
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
pub fn is_credit_role(role: &str) -> bool {
    let normalized = role
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    WRITERS.contains(&normalized.as_str()) || OTHER.contains(&normalized.as_str()) || {
        let parts = role_parts(role);
        !parts.is_empty()
            && parts.iter().all(|s| {
                WRITERS.contains(&s.as_str())
                    || OTHER.contains(&s.as_str())
                    || s.chars().count() <= 8 && s.ends_with(['词', '詞', '曲', '声', '聲', '音'])
            })
    }
}
pub fn credit_line(text: &str) -> Option<Vec<String>> {
    let (role, names) = text.split_once([':', '：'])?;
    if !is_credit_role(role) {
        return None;
    }
    let normalized = role
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let writer = WRITERS.contains(&normalized.as_str())
        || role_parts(role)
            .iter()
            .any(|s| WRITERS.contains(&s.as_str()));
    Some(if writer { split_names(names) } else { vec![] })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_and_exact_names() {
        assert_eq!(
            credit_line("Written by: Max / Jane"),
            Some(vec!["Max".into(), "Jane".into()])
        );
        assert_eq!(
            credit_line("Rap作词/编曲：周杰伦"),
            Some(vec!["周杰伦".into()])
        );
        assert_eq!(credit_line("Singer: Hello"), None);
        assert_eq!(
            unique_names([" A ".into(), "A".into(), "a".into()]),
            ["A", "a"]
        );
    }
}
