//! chewBBACA CLI 인자 조립 (§10.1).
//!
//! **플랫폼 중립이다.** 입력은 이미 백엔드 경로로 변환된 문자열이므로,
//! macOS `NativeRunner` 가 생겨도 이 파일은 그대로 재사용된다.

use crate::models::Module;

/// 백엔드 경로로 모두 변환된 뒤의 실행 인자.
#[derive(Debug, Clone, Default)]
pub struct BackendArgs {
    /// 어셈블리 입력 디렉터리 (ext4 내부로 복사된 사본)
    pub input: String,
    /// 산출물 디렉터리
    pub output: String,
    /// AlleleCall 이 사용할 스키마 디렉터리 (`schema_seed` 의 부모)
    pub schema: Option<String>,
    /// Prodigal training file
    pub ptf: Option<String>,
    /// `--gl` 로 넘길 loci 목록 파일
    pub loci_list: Option<String>,
    pub cds_input: bool,
    pub cpu: u32,
    /// ExtractCgMLST 의 `--t`. 공백으로 구분된 임계값들. 비면 인자를 넣지 않는다.
    pub thresholds: Option<String>,
}

/// `chewBBACA.py` 뒤에 붙는 인자 벡터를 만든다.
pub fn build_argv(module: Module, a: &BackendArgs) -> Vec<String> {
    let mut v: Vec<String> = vec!["chewBBACA.py".into(), module.cli_name().into()];

    match module {
        Module::CreateSchema => {
            v.push("-i".into());
            v.push(a.input.clone());
            v.push("-o".into());
            v.push(a.output.clone());
            if let Some(ptf) = &a.ptf {
                v.push("--ptf".into());
                v.push(ptf.clone());
            }
            if a.cds_input {
                v.push("--cds".into());
            }
        }
        Module::AlleleCall => {
            v.push("-i".into());
            v.push(a.input.clone());
            v.push("-g".into());
            // 스키마 디렉터리가 아니라 그 안의 schema_seed 를 넘긴다.
            v.push(format!(
                "{}/schema_seed",
                a.schema.clone().unwrap_or_default()
            ));
            v.push("-o".into());
            v.push(a.output.clone());
            if let Some(gl) = &a.loci_list {
                v.push("--gl".into());
                v.push(gl.clone());
            }
            // AlleleCall 도 CDS 입력을 받는다. CreateSchema 에만 붙이고 있어서
            // CDS FASTA 를 가진 사용자는 Prodigal 이 다시 도는 결과를 얻고 있었다.
            if a.cds_input {
                v.push("--cds".into());
            }
        }
        Module::ExtractCgMLST => {
            // 입력이 폴더가 아니라 AlleleCall 결과 TSV 파일 하나다.
            v.push("-i".into());
            v.push(a.input.clone());
            v.push("-o".into());
            v.push(a.output.clone());
            if let Some(t) = &a.thresholds {
                // "0.95 0.99 1" 처럼 여러 값을 받는다 — 각각 별도 인자로 넘긴다.
                let vals: Vec<&str> = t.split_whitespace().collect();
                if !vals.is_empty() {
                    v.push("--t".into());
                    v.extend(vals.into_iter().map(String::from));
                }
            }
        }
    }

    // `--cpu` 는 모든 모듈에 있는 인자가 아니다. ExtractCgMLST 에 붙이면
    // argparse 가 "unrecognized arguments" 로 즉시 실패한다.
    if !matches!(module, Module::ExtractCgMLST) {
        v.push("--cpu".into());
        v.push(a.cpu.to_string());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> BackendArgs {
        BackendArgs {
            input: "/home/chewie/work/j1/input".into(),
            output: "/home/chewie/work/j1/output".into(),
            schema: Some("/home/chewie/schemas/s1".into()),
            cpu: 8,
            ..Default::default()
        }
    }

    #[test]
    fn create_schema_argv() {
        let mut a = args();
        a.output = "/home/chewie/schemas/s1".into();
        a.ptf = Some("/home/chewie/schemas/s1/Listeria.trn".into());
        a.cds_input = true;
        let v = build_argv(Module::CreateSchema, &a);
        assert_eq!(v[1], "CreateSchema");
        assert!(v.contains(&"--cds".to_string()));
        assert!(v.contains(&"--ptf".to_string()));
        assert_eq!(v[v.len() - 2], "--cpu");
        assert_eq!(v[v.len() - 1], "8");
    }

    #[test]
    fn allele_call_points_at_schema_seed() {
        let v = build_argv(Module::AlleleCall, &args());
        let g = v.iter().position(|x| x == "-g").unwrap();
        assert_eq!(v[g + 1], "/home/chewie/schemas/s1/schema_seed");
    }

    #[test]
    fn extract_cgmlst_never_gets_cpu() {
        // ExtractCgMLST 에는 --cpu 가 없다. 붙이면 argparse 가 즉시 실패한다.
        let v = build_argv(Module::ExtractCgMLST, &args());
        assert!(!v.contains(&"--cpu".to_string()), "{v:?}");
        assert_eq!(v[1], "ExtractCgMLST");
    }

    #[test]
    fn extract_cgmlst_passes_each_threshold_separately() {
        let mut a = args();
        a.thresholds = Some("0.95 0.99 1".into());
        let v = build_argv(Module::ExtractCgMLST, &a);
        let t = v.iter().position(|x| x == "--t").expect("--t 가 있어야 한다");
        assert_eq!(&v[t + 1..t + 4], ["0.95", "0.99", "1"]);
    }

    #[test]
    fn extract_cgmlst_omits_threshold_when_blank() {
        // 비우면 chewBBACA 기본값(0.95/0.99/1)을 모두 계산하게 둔다.
        let mut a = args();
        a.thresholds = Some("   ".into());
        let v = build_argv(Module::ExtractCgMLST, &a);
        assert!(!v.contains(&"--t".to_string()), "{v:?}");
    }

    #[test]
    fn allele_call_forwards_cds_flag() {
        // AlleleCall 도 --cds 를 받는다. 빠져 있으면 CDS 입력에 Prodigal 이 다시 돈다.
        let mut a = args();
        a.cds_input = true;
        assert!(build_argv(Module::AlleleCall, &a).contains(&"--cds".to_string()));
    }

    #[test]
    fn extract_cgmlst_ignores_cds_flag() {
        // ExtractCgMLST 에는 --cds 가 없다.
        let mut a = args();
        a.cds_input = true;
        assert!(!build_argv(Module::ExtractCgMLST, &a).contains(&"--cds".to_string()));
    }

    #[test]
    fn allele_call_omits_gl_when_absent() {
        let v = build_argv(Module::AlleleCall, &args());
        assert!(!v.contains(&"--gl".to_string()));
    }
}
