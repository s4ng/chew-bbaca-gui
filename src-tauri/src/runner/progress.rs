//! 진행률 파싱 (§4.2).
//!
//! chewBBACA 는 기계가 읽으라고 만든 출력을 내지 않는다. 여기 있는 단계 키워드는
//! **2026-08-10 에 chewBBACA 3.5.4 를 실제로 완주시켜 얻은 로그**에서 뽑았다
//! (CreateSchema / AlleleCall 각 1회, `--cds` 입력). 추측이 아니다.
//!
//! 그래도 파싱 실패가 곧 작업 실패가 되지 않도록, 인식하지 못한 줄은 조용히 무시한다.
//! chewBBACA 가 판올림하며 문구를 바꾸면 진행률만 멈추고 작업은 정상 진행된다.
//!
//! 세 가지 성질을 지킨다.
//! * **단조 증가** — 단계마다 0→100% 를 반복하므로 그대로 노출하면 막대가 되감긴다.
//! * **단계 가중** — 단계별 구간을 미리 배분하고 그 안에서만 채운다.
//! * **모듈별 분리** — 두 모듈이 `CDS deduplication` 같은 이름을 공유하지만 순서와
//!   비중이 다르다. 한 테이블로 합치면 어느 한쪽이 반드시 틀어진다.

use std::sync::OnceLock;

use regex::Regex;

use crate::models::Module;

/// (소문자 키워드, 전체 진행률에서의 구간 시작, 구간 폭, 표시 라벨)
///
/// 키워드는 로그에 나오는 문구를 소문자로 그대로 옮긴 것이다. 순서가 곧 단계 순서이며,
/// **뒤 단계의 키워드가 앞 단계 줄에 우연히 포함되지 않도록** 충분히 길게 잡는다.
type Stage = (&'static str, f32, f32, &'static str);

/// CreateSchema. 실측 로그 기준 비용의 대부분은 두 BLASTp 구간에 있다.
const CREATE_SCHEMA: &[Stage] = &[
    ("renaming cdss for", 0.00, 0.05, "입력 CDS 정리 중"),
    ("identifying distinct cdss", 0.05, 0.05, "중복 CDS 제거 중"),
    ("translating", 0.10, 0.10, "CDS 번역 중"),
    (
        "identifying distinct proteins",
        0.20,
        0.05,
        "중복 단백질 제거 중",
    ),
    ("clustering proteins", 0.25, 0.15, "단백질 클러스터링 중"),
    (
        "performing all-vs-all blastp",
        0.40,
        0.35,
        "클러스터별 BLASTp 중",
    ),
    ("performing final blastp", 0.75, 0.20, "최종 BLASTp 중"),
    ("creating schema seed", 0.95, 0.04, "스키마 생성 중"),
];

/// AlleleCall. 단계 수가 훨씬 많고, 무거운 곳은 대표 서열 정렬과 분류다.
const ALLELE_CALL: &[Stage] = &[
    (
        "determining allele size mode",
        0.00,
        0.03,
        "스키마 사전 계산 중",
    ),
    ("creating hash tables", 0.03, 0.04, "해시 테이블 생성 중"),
    ("renaming cdss for", 0.07, 0.03, "입력 CDS 정리 중"),
    ("identifying distinct cdss", 0.10, 0.03, "중복 CDS 제거 중"),
    (
        "searching for cds exact matches",
        0.13,
        0.07,
        "CDS 정확 일치 검색 중",
    ),
    ("translating", 0.20, 0.08, "CDS 번역 중"),
    (
        "identifying distinct proteins",
        0.28,
        0.03,
        "중복 단백질 제거 중",
    ),
    (
        "searching for protein exact matches",
        0.31,
        0.06,
        "단백질 정확 일치 검색 중",
    ),
    (
        "determining blastp self-score",
        0.37,
        0.06,
        "self-score 계산 중",
    ),
    ("clustering proteins", 0.43, 0.12, "단백질 클러스터링 중"),
    (
        "aligning cluster representatives",
        0.55,
        0.25,
        "대표 서열 정렬 중",
    ),
    (
        "classifying high-scoring matches",
        0.80,
        0.12,
        "allele 분류 중",
    ),
    (
        "assigning allele identifiers",
        0.92,
        0.04,
        "allele 번호 부여 중",
    ),
    (
        "creating file with the allelic profiles",
        0.96,
        0.03,
        "결과 기록 중",
    ),
];

/// ExtractCgMLST. 임계값마다 같은 문구를 반복하고 진행률 막대가 없어 **거칠다.**
/// 대신 실행이 수 초~수십 초라 거친 표시로도 문제가 되지 않는다.
///
/// "composed of 20/20 genes" 가 비율 파서에 걸려 첫 임계값이 끝나면 구간 끝까지
/// 차오른다 — 되감기지 않으므로 그대로 둔다.
const EXTRACT_CGMLST: &[Stage] = &[
    (
        "determining cgmlst for loci presence threshold",
        0.05,
        0.80,
        "core genome 계산 중",
    ),
    ("html file with cgmlst", 0.90, 0.09, "리포트 생성 중"),
];

/// PrepExternalSchema. 2026-08-11 실측 로그 기준.
///
/// 단계가 사실상 둘뿐이고 변환 구간에만 막대가 붙는다. 처음 만들 때 추측했던
/// `adapting schema` 는 로그에 아예 없는 문구였다 — `Adapting 12 loci...` 다.
const PREP_EXTERNAL: &[Stage] = &[
    (
        "determining the total number of alleles",
        0.02,
        0.06,
        "스키마 훑는 중",
    ),
    ("adapting", 0.08, 0.84, "loci 변환 중"),
    ("number of invalid loci", 0.92, 0.06, "마무리 중"),
];

fn percent_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{1,3})\s*%").expect("percent regex"))
}

fn ratio_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // "12/240", "[ 12/240 ]" 같은 형태. 앞뒤에 슬래시 경로가 붙는 경우를 피하려고
    // 숫자 양쪽에 경로 문자가 없는 경우만 잡는다.
    RE.get_or_init(|| {
        Regex::new(r"(?:^|[^\w/])(\d+)\s*/\s*(\d+)(?:[^\w/]|$)").expect("ratio regex")
    })
}

pub struct ProgressParser {
    stages: &'static [Stage],
    stage: usize,
    last: f32,
    label: String,
}

impl ProgressParser {
    pub fn for_module(module: Module) -> Self {
        Self {
            stages: match module {
                Module::CreateSchema => CREATE_SCHEMA,
                Module::AlleleCall => ALLELE_CALL,
                Module::ExtractCgMLST => EXTRACT_CGMLST,
                Module::PrepExternalSchema => PREP_EXTERNAL,
            },
            stage: usize::MAX,
            last: 0.0,
            label: String::from("준비 중"),
        }
    }

    /// 한 줄을 보고 진행률이 **변했을 때만** 값을 돌려준다.
    pub fn observe(&mut self, line: &str) -> Option<(f32, String)> {
        let lower = line.to_lowercase();

        // 1) 단계 전환 감지 — 뒤로 가는 전환은 무시한다.
        //    AlleleCall 의 "Translating schema representative alleles" 가 앞 단계인
        //    "translating"(CDS 번역) 에 걸리는 것이 실제 사례다.
        let mut entered = false;
        if let Some(idx) = self.stages.iter().position(|(k, ..)| lower.contains(k)) {
            if self.stage == usize::MAX || idx > self.stage {
                self.stage = idx;
                self.label = self.stages[idx].3.to_string();
                entered = true;
            }
        }

        // 2) 단계 내부 비율. 막대가 없는 단계도 있으므로, 비율이 없더라도
        //    단계에 막 진입했다면 그 구간의 시작값까지는 올린다 — 그러지 않으면
        //    BLASTp 처럼 막대가 붙은 단계 사이에서 막대가 오래 멈춰 있는다.
        let inner = match parse_fraction(&lower) {
            Some(f) => f,
            None if entered => 0.0,
            None => return None,
        };

        let (base, span) = match self.stages.get(self.stage) {
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

    /// 2026-08-10 실측 로그에서 그대로 옮긴 CreateSchema 출력.
    /// `pump()` 가 `\r` 로 잘라 넣으므로 막대는 한 줄에 하나씩 들어온다.
    const CREATE_SCHEMA_LOG: &[&str] = &[
        "Started at: 2026-08-10T09:28:46",
        "Number of inputs: 4",
        "Renaming CDSs for 4 input files...",
        "Input files contain a total of 80 coding sequences.",
        "Identifying distinct CDSs...",
        "Translating 80 CDS...",
        " [                    ] 0%",
        " [====================] 100%",
        "Identifying distinct proteins...",
        "Clustering proteins...",
        " [====================] 100%",
        "Performing all-vs-all BLASTp per cluster...",
        " [========            ] 40%",
        " [====================] 100%",
        "Performing final BLASTp...",
        " [====================] 100%",
        "Creating schema seed in /tmp/smoke/out",
        "Created schema seed with 20 loci.",
    ];

    /// 같은 날 AlleleCall 로그의 단계 헤더들.
    const ALLELE_CALL_LOG: &[&str] = &[
        "Determining allele size mode for all loci...",
        "Creating hash tables...",
        "Renaming CDSs for 4 input files...",
        "Identifying distinct CDSs...",
        "Searching for CDS exact matches...",
        "Translating 60 CDSs...",
        " [====================] 100%",
        "Identifying distinct proteins...",
        "Searching for Protein exact matches...",
        "Translating schema representative alleles...",
        "Determining BLASTp self-score for each representative...",
        "Clustering proteins...",
        " [====================] 100%",
        "Aligning cluster representatives against clustered proteins...",
        " [==========          ] 50%",
        " [====================] 100%",
        "Classifying high-scoring matches...",
        " [====================] 100%",
        "Assigning allele identifiers to inferred alleles...",
        "Creating file with the allelic profiles (results_alleles.tsv)...",
    ];

    fn run(module: Module, log: &[&str]) -> Vec<(f32, String)> {
        let mut p = ProgressParser::for_module(module);
        log.iter().filter_map(|l| p.observe(l)).collect()
    }

    #[test]
    fn create_schema_log_advances_monotonically_to_the_end() {
        let seen = run(Module::CreateSchema, CREATE_SCHEMA_LOG);
        assert!(!seen.is_empty(), "실측 로그에서 아무것도 인식하지 못했다");
        let mut prev = 0.0;
        for (v, _) in &seen {
            assert!(*v > prev, "되감김: {prev} → {v}");
            prev = *v;
        }
        // 마지막 줄까지 갔으면 스키마 생성 단계에 도달해야 한다.
        assert!(prev >= 0.95, "마지막 진행률이 {prev} 에 그쳤다");
    }

    #[test]
    fn allele_call_log_advances_monotonically_to_the_end() {
        let seen = run(Module::AlleleCall, ALLELE_CALL_LOG);
        let mut prev = 0.0;
        for (v, _) in &seen {
            assert!(*v > prev, "되감김: {prev} → {v}");
            prev = *v;
        }
        assert!(prev >= 0.96, "마지막 진행률이 {prev} 에 그쳤다");
    }

    /// 2026-08-11 실측. 이 모듈은 막대가 변환 구간에만 붙는다.
    const PREP_EXTERNAL_LOG: &[&str] = &[
        "Number of loci to adapt: 12",
        "Determining the total number of alleles and allele mean length per gene...",
        "Adapting 12 loci...",
        " [                    ] 0%",
        " [========            ] 41%",
        " [====================] 100%",
        "Number of invalid loci: 0",
        "Successfully adapted 12/12 loci present in the input schema.",
    ];

    #[test]
    fn prep_external_schema_log_advances_monotonically() {
        let seen = run(Module::PrepExternalSchema, PREP_EXTERNAL_LOG);
        assert!(!seen.is_empty(), "실측 로그에서 아무것도 인식하지 못했다");
        let mut prev = 0.0;
        for (v, _) in &seen {
            assert!(*v > prev, "되감김: {prev} → {v}");
            prev = *v;
        }
        assert!(prev >= 0.92, "마지막 진행률이 {prev} 에 그쳤다");
    }

    #[test]
    fn representative_translation_does_not_rewind_allele_call() {
        // "Translating schema representative alleles" 는 CDS 번역 단계보다 **뒤**에
        // 나오지만 "translating" 을 포함한다. 여기서 되감기면 막대가 튄다.
        let mut p = ProgressParser::for_module(Module::AlleleCall);
        p.observe("Searching for Protein exact matches...");
        let before = p.value();
        p.observe("Translating schema representative alleles...");
        assert!(p.value() >= before, "{} → {}", before, p.value());
    }

    #[test]
    fn stage_header_alone_moves_the_bar() {
        // 막대가 붙지 않는 단계가 많다. 헤더만으로도 구간 시작까지는 올라가야 한다.
        let mut p = ProgressParser::for_module(Module::CreateSchema);
        let (v, label) = p.observe("Performing final BLASTp...").unwrap();
        assert!((v - 0.75).abs() < 0.001, "got {v}");
        assert_eq!(label, "최종 BLASTp 중");
    }

    #[test]
    fn percent_within_stage_is_weighted() {
        let mut p = ProgressParser::for_module(Module::CreateSchema);
        p.observe("Performing all-vs-all BLASTp per cluster...");
        let (v, label) = p.observe(" [==========          ] 50%").unwrap();
        // 0.40 시작 + 0.35 폭의 절반
        assert!((v - 0.575).abs() < 0.01, "got {v}");
        assert_eq!(label, "클러스터별 BLASTp 중");
    }

    #[test]
    fn ignores_unrelated_lines() {
        let mut p = ProgressParser::for_module(Module::AlleleCall);
        assert!(p.observe("BLAST path: /opt/conda/bin").is_none());
        assert!(p.observe("Started at: 2026-08-10T09:29:31").is_none());
        assert!(p.observe("").is_none());
    }
}
