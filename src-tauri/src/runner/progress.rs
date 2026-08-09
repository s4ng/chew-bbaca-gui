//! 진행률 파싱 (§4.2).
//!
//! chewBBACA 는 기계가 읽으라고 만든 출력을 내지 않는다. 여기 있는 규칙은
//! **휴리스틱**이며, 실제 데이터셋 완주(§11 미검증 항목) 후 실측 로그로
//! 교정해야 한다. 그래서 파싱 실패가 곧 작업 실패가 되지 않도록,
//! 인식하지 못한 줄은 조용히 무시한다.
//!
//! 두 가지 성질을 지킨다.
//! * **단조 증가** — 단계마다 0→100% 를 반복하므로 그대로 노출하면 막대가 되감긴다.
//! * **단계 가중** — 단계별 구간을 미리 배분하고 그 안에서만 채운다.

use std::sync::OnceLock;

use regex::Regex;

/// (출력에 등장하는 소문자 키워드, 전체 진행률에서 차지하는 구간 시작, 구간 폭, 표시 라벨)
const STAGES: &[(&str, f32, f32, &str)] = &[
    // --- CreateSchema
    ("reading the input files", 0.02, 0.06, "입력 읽는 중"),
    ("extracting cds", 0.08, 0.22, "CDS 추출 중"),
    ("removing duplicated", 0.30, 0.10, "중복 서열 제거 중"),
    ("clustering", 0.40, 0.35, "클러스터링 중"),
    ("creating schema", 0.75, 0.20, "스키마 생성 중"),
    // --- AlleleCall
    ("determining self-scores", 0.05, 0.15, "self-score 계산 중"),
    ("aligning", 0.20, 0.30, "정렬 중"),
    ("classif", 0.50, 0.35, "allele 분류 중"),
    ("writing", 0.88, 0.10, "결과 기록 중"),
];

fn percent_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{1,3})\s*%").expect("percent regex"))
}

fn ratio_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "12/240", "[ 12/240 ]" 같은 형태. 앞뒤에 슬래시 경로가 붙는 경우를 피하려고
    // 숫자 양쪽에 경로 문자가 없는 경우만 잡는다.
    RE.get_or_init(|| Regex::new(r"(?:^|[^\w/])(\d+)\s*/\s*(\d+)(?:[^\w/]|$)").expect("ratio regex"))
}

pub struct ProgressParser {
    stage: usize,
    last: f32,
    label: String,
}

impl Default for ProgressParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressParser {
    pub fn new() -> Self {
        Self {
            stage: usize::MAX,
            last: 0.0,
            label: String::from("준비 중"),
        }
    }

    /// 한 줄을 보고 진행률이 **변했을 때만** 값을 돌려준다.
    pub fn observe(&mut self, line: &str) -> Option<(f32, String)> {
        let lower = line.to_lowercase();

        // 1) 단계 전환 감지 — 뒤로 가는 전환은 무시한다.
        if let Some(idx) = STAGES.iter().position(|(k, ..)| lower.contains(k)) {
            if self.stage == usize::MAX || idx > self.stage {
                self.stage = idx;
                self.label = STAGES[idx].3.to_string();
            }
        }

        // 2) 단계 내부 비율
        let inner = parse_fraction(&lower)?;

        let (base, span) = match STAGES.get(self.stage) {
            Some((_, base, span, _)) => (*base, *span),
            // 단계를 아직 모르면 전체의 앞쪽 절반에만 매핑한다 (과대 표시 방지).
            None => (0.0, 0.5),
        };

        let value = (base + span * inner).clamp(0.0, 0.99);
        if value <= self.last + 0.001 {
            return None;
        }
        self.last = value;
        Some((value, self.label.clone()))
    }

    pub fn value(&self) -> f32 {
        self.last
    }
}

fn parse_fraction(lower: &str) -> Option<f32> {
    if let Some(c) = percent_re().captures(lower) {
        let p: f32 = c.get(1)?.as_str().parse().ok()?;
        if p <= 100.0 {
            return Some(p / 100.0);
        }
    }
    if let Some(c) = ratio_re().captures(lower) {
        let done: f32 = c.get(1)?.as_str().parse().ok()?;
        let total: f32 = c.get(2)?.as_str().parse().ok()?;
        if total > 0.0 && done <= total {
            return Some(done / total);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_within_stage_is_weighted() {
        let mut p = ProgressParser::new();
        let (v, label) = p.observe("Clustering sequences... 50%").unwrap();
        // clustering 구간은 0.40 부터 0.35 폭
        assert!((v - 0.575).abs() < 0.01, "got {v}");
        assert_eq!(label, "클러스터링 중");
    }

    #[test]
    fn never_goes_backwards_across_stages() {
        let mut p = ProgressParser::new();
        p.observe("Clustering 90%");
        let before = p.value();
        // 다음 단계가 0% 로 다시 시작해도 되감기지 않는다.
        assert!(p.observe("Creating schema 0%").is_none() || p.value() >= before);
    }

    #[test]
    fn ratio_form_is_recognised() {
        let mut p = ProgressParser::new();
        assert!(p.observe("Aligning genomes 30/60").is_some());
    }

    #[test]
    fn ignores_unrelated_lines() {
        let mut p = ProgressParser::new();
        assert!(p.observe("Loading schema from /home/chewie/schemas/s1").is_none());
        assert!(p.observe("").is_none());
    }
}
