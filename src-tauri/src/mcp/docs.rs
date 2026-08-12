//! 모듈 사용법의 **단일 진실 원천** (`doc/MCP.md` §5).
//!
//! `match` 로 쓴 것이 핵심이다 — `Module` 에 variant 를 더하면 여기서 컴파일이
//! 깨지므로, 문서 없이 모듈이 느는 일이 구조적으로 막힌다.
//!
//! 내용의 출처는 추측이 아니라 실측이다. `doc/NEXT-SESSION.md` §4 의 함정 목록과
//! 각 모듈을 실제로 완주시키며 얻은 것들이 여기 모여 있다. **새 사실을 알게 되면
//! 여기에 적는다** — MCP 클라이언트가 읽는 것도, (나중에) 폼 도움말이 읽을 것도
//! 이 표 하나다.

use crate::models::Module;

pub struct ModuleDoc {
    /// 이 모듈이 무엇을 하는가 (한 문장)
    pub summary: &'static str,
    /// 무엇을 넣는가
    pub inputs: &'static str,
    /// 무엇이 나오는가
    pub outputs: &'static str,
    /// 부르기 전에 만족해야 하는 것. 어기면 실패한다.
    pub prerequisites: &'static [&'static str],
    /// 실제로 사람을 물었던 것들.
    pub gotchas: &'static [&'static str],
}

pub const ALL_MODULES: [Module; 8] = [
    Module::CreateSchema,
    Module::AlleleCall,
    Module::ExtractCgMLST,
    Module::PrepExternalSchema,
    Module::RemoveGenes,
    Module::JoinProfiles,
    Module::SchemaEvaluator,
    Module::AlleleCallEvaluator,
];

pub fn doc(module: Module) -> ModuleDoc {
    match module {
        Module::CreateSchema => ModuleDoc {
            summary: "어셈블리(FASTA) 모음에서 새 cg/wgMLST 스키마를 만든다. 파이프라인의 1단계다.",
            inputs: "어셈블리 FASTA 가 들어 있는 폴더 하나(inputDir)와 스키마 이름(schemaName).",
            outputs:
                "스키마가 앱 저장소에 등록된다. 결과 폴더로 회수되는 것은 없으므로 outputDir 는 생략해도 된다.",
            prerequisites: &[
                "inputDir 는 절대 경로여야 하고 UNC(\\\\server\\share) 경로는 지원하지 않는다.",
            ],
            gotchas: &[
                "입력이 이미 CDS 라면 cdsInput 을 켠다. 켜면 Prodigal 단계가 통째로 사라져 훨씬 빠르지만, 그 결과로 만든 스키마로 AlleleCall 을 돌리면 cds_coordinates.tsv 가 생기지 않아 AlleleCallEvaluator 를 쓸 수 없다.",
                "실제 어셈블리에서는 클러스터별 BLASTp 가 전체 시간의 약 73% 를 차지한다. 완성 게놈 32개 기준 38초(--cpu 8).",
                "같은 이름으로 여러 번 만들 수 있다. 스키마 ID 는 이름과 작업 ID 로 만들어져 서로 덮어쓰지 않는다.",
            ],
        },
        Module::AlleleCall => ModuleDoc {
            summary: "어셈블리를 기존 스키마에 대해 allele calling 한다. 파이프라인의 2단계다.",
            inputs: "어셈블리 폴더(inputDir), 사용할 스키마(schemaId), 결과를 받을 폴더(outputDir).",
            outputs: "results_alleles.tsv 를 포함한 결과 폴더 일습이 outputDir 로 회수된다.",
            prerequisites: &[
                "schemaId 는 chewie_list_schemas 로 얻은 값이어야 한다.",
                "outputDir 는 절대 경로여야 한다.",
            ],
            gotchas: &[
                "lociList 를 주면 그 loci 만 대상으로 한다(--gl). 파일은 한 줄에 loci 이름 하나여야 하고, 프로파일 표(탭으로 나뉜 것)를 넣으면 거절한다.",
                "cdsInput 은 스키마를 만들 때와 같은 값으로 맞추는 편이 안전하다. 켜면 cds_coordinates.tsv 가 생기지 않아 나중에 AlleleCallEvaluator 를 못 쓴다.",
                "대표 서열 정렬이 전체 시간의 약 77% 다. 완성 게놈 32개 × loci 3,127 기준 1분 30초.",
                "이 모듈은 스키마에 새 allele 을 계속 추가한다. 같은 스키마로 여러 번 돌리면 스키마가 자라고, 예전 결과와 loci 수가 달라질 수 있다.",
            ],
        },
        Module::ExtractCgMLST => ModuleDoc {
            summary: "AlleleCall 결과에서 core genome(모든 균주에 존재하는 loci)을 추린다.",
            inputs: "AlleleCall 이 만든 results_alleles.tsv 파일 하나(profilesFile)와 결과 폴더(outputDir).",
            outputs: "임계값별 cgMLSTschema*.txt(loci 목록)와 프로파일 표.",
            prerequisites: &["profilesFile 은 폴더가 아니라 파일 하나다."],
            gotchas: &[
                "결과 폴더에는 TSV 가 일곱 개 있고 확장자로는 구별되지 않는다. cds_coordinates.tsv 를 잘못 넣으면 chewBBACA 가 거절하지 않고 각 행을 균주로 취급해 한참 헛돈다 — 앱이 제출 전에 막지만, 먼저 chewie_inspect 로 확인하는 편이 빠르다.",
                "thresholds 를 비우면 chewBBACA 기본값(0.95 / 0.99 / 1)을 모두 계산한다.",
                "--cpu 인자가 없는 모듈이라 cpu 를 줘도 무시된다.",
            ],
        },
        Module::PrepExternalSchema => ModuleDoc {
            summary: "외부에서 받은 스키마를 chewBBACA 형식으로 변환해 앱 저장소에 등록한다. CreateSchema 와 같은 자리(1단계)를 대신한다.",
            inputs: "loci 마다 FASTA 파일 하나가 들어 있는 폴더(schemaDir)와 등록할 이름(schemaName).",
            outputs: "스키마가 앱 저장소에 등록된다. outputDir 는 생략해도 된다.",
            prerequisites: &["schemaDir 안에 FASTA 파일이 하나도 없으면 제출 단계에서 거절한다."],
            gotchas: &[
                "이 모듈은 -o 아래에 schema_seed/ 를 만들지 않고 loci FASTA 를 바로 푼다. 앱이 그 사실에 맞춰 경로를 겨누므로 사용자가 신경 쓸 것은 없다.",
                "부산물(schema_seed_invalid_loci.txt 등)은 스키마 폴더 최상위에 놓인다.",
                "들여온 스키마로 AlleleCall 이 정상 완주하는 것은 확인되어 있다.",
            ],
        },
        Module::RemoveGenes => ModuleDoc {
            summary: "프로파일 표에서 지정한 loci 를 빼거나, 반대로 그것만 남긴다.",
            inputs: "프로파일 표(profilesFile), 대상 loci 목록 파일(genesList), 결과 폴더(outputDir).",
            outputs: "results_alleles_filtered.tsv 파일 하나가 outputDir 로 회수된다.",
            prerequisites: &[
                "genesList 는 한 줄에 loci 이름 하나인 파일이어야 한다. 탭이 있으면 표로 보고 거절한다.",
            ],
            gotchas: &[
                "keepInstead 를 켜면 목록에 있는 것만 남긴다(--inverse). 기본은 목록에 있는 것을 제거한다.",
                "숫자로 확인하는 습관이 좋다 — loci 3,127 에서 목록 1,270 을 제거하면 1,857 이 남는다.",
            ],
        },
        Module::JoinProfiles => ModuleDoc {
            summary: "여러 번에 나눠 돌린 AlleleCall 프로파일 표를 하나로 합친다.",
            inputs: "합칠 프로파일 표 둘 이상(profilesFiles)과 결과 폴더(outputDir).",
            outputs: "joined_profiles.tsv 파일 하나가 outputDir 로 회수된다.",
            prerequisites: &["profilesFiles 는 두 개 이상이어야 한다."],
            gotchas: &[
                "합칠 결과들의 균주가 겹치면 안 된다. 같은 균주가 든 표 둘을 합치면 모든 균주가 두 번씩 들어간 표가 나오고, 배관은 정상이지만 그 표는 분석에 쓸 수 없다.",
                "스키마가 자란 뒤의 결과를 예전 결과와 합칠 때는 commonOnly 를 켠다(--common). 공통 loci 만 남긴다.",
            ],
        },
        Module::SchemaEvaluator => ModuleDoc {
            summary: "스키마 품질 리포트(HTML)를 만든다.",
            inputs: "평가할 스키마(schemaId)와 결과 폴더(outputDir).",
            outputs: "schema_report.html 과 report_bundle.js. chewie_open_report 로 브라우저에서 연다.",
            prerequisites: &["schemaId 는 chewie_list_schemas 로 얻은 값이어야 한다."],
            gotchas: &[
                "lociReports 를 켜면 loci 마다 MAFFT 를 돌린다. loci 3,127 기준 3초 → 39초가 되고, 회수 파일도 2개 → 3,130개가 된다.",
                "이 모듈은 -o 가 이미 있으면 'Output directory already exists.' 한 줄과 exit 1 로 끝난다. 앱이 실행 직전에 빈 출력 폴더를 도로 지워 이를 피한다.",
            ],
        },
        Module::AlleleCallEvaluator => ModuleDoc {
            summary: "AlleleCall 결과의 품질 리포트(HTML)를 만든다.",
            inputs: "AlleleCall 결과 폴더(resultsDir), 그때 쓴 스키마(schemaId), 결과 폴더(outputDir).",
            outputs: "allelecall_report.html 과 report_bundle.js. chewie_open_report 로 연다.",
            prerequisites: &[
                "resultsDir 안에 results_alleles.tsv 가 있어야 한다.",
                "resultsDir 안에 cds_coordinates.tsv 가 있어야 한다. 이 파일은 AlleleCall 이 Prodigal 로 CDS 를 예측했을 때만 생긴다.",
            ],
            gotchas: &[
                "cdsInput 을 켜고 돌린 AlleleCall 결과에는 cds_coordinates.tsv 가 없어 이 모듈을 쓸 수 없다. 앱이 제출 단계에서 막는다(막지 않으면 파이썬 traceback 만 보게 된다).",
                "MSA 와 NJ 트리가 전체 시간의 대부분이다. 균주 32 × loci 3,127 기준 38초.",
            ],
        },
    }
}

/// 모듈 하나의 사용법을 사람이 읽는 텍스트로.
pub fn render(module: Module) -> String {
    let d = doc(module);
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n{}\n\n", module.cli_name(), d.summary));
    s.push_str(&format!("입력: {}\n출력: {}\n", d.inputs, d.outputs));
    if !d.prerequisites.is_empty() {
        s.push_str("\n전제조건\n");
        for p in d.prerequisites {
            s.push_str(&format!("- {p}\n"));
        }
    }
    if !d.gotchas.is_empty() {
        s.push_str("\n주의\n");
        for g in d.gotchas {
            s.push_str(&format!("- {g}\n"));
        }
    }
    s
}

/// 여덟 모듈 전체 요약. 어느 모듈을 쓸지 고르는 데 쓴다.
pub fn render_all() -> String {
    let mut s = String::from(
        "chewBBACA 모듈 여덟 개. 표준 순서는 CreateSchema(또는 PrepExternalSchema) → AlleleCall → ExtractCgMLST 다.\n\n",
    );
    for m in ALL_MODULES {
        s.push_str(&format!("- {}: {}\n", m.cli_name(), doc(m).summary));
    }
    s.push_str("\n각 모듈의 인자와 주의사항은 chewie_module_help(module) 로 읽는다.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_module_has_a_usable_doc() {
        // match 가 컴파일 타임에 누락을 막지만, 빈 문자열로 채우는 것까지는 못 막는다.
        for m in ALL_MODULES {
            let d = doc(m);
            assert!(!d.summary.is_empty(), "{:?} 요약이 비었다", m);
            assert!(!d.inputs.is_empty(), "{:?} 입력 설명이 비었다", m);
            assert!(!d.outputs.is_empty(), "{:?} 출력 설명이 비었다", m);
        }
    }

    #[test]
    fn all_modules_covers_the_enum() {
        // ALL_MODULES 는 손으로 쓴 배열이라 variant 가 늘 때 빠질 수 있다.
        // 모듈 이름 파싱으로 왕복시켜 개수와 내용을 함께 확인한다.
        assert_eq!(ALL_MODULES.len(), 8);
        for m in ALL_MODULES {
            assert_eq!(Module::parse(m.cli_name()), Some(m));
        }
    }

    #[test]
    fn rendered_help_mentions_the_module_name() {
        let text = render(Module::AlleleCallEvaluator);
        assert!(text.contains("AlleleCallEvaluator"));
        assert!(text.contains("cds_coordinates.tsv"), "전제조건이 빠졌다");
    }
}
