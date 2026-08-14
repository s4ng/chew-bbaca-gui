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
    /// 입력이 여러 개인 모듈용 (JoinProfiles). 비어 있으면 `input` 을 쓴다.
    pub inputs: Vec<String>,
    /// 두 번째 입력 파일 (RemoveGenes 의 loci 목록)
    pub genes_list: Option<String>,
    /// `--inverse` / `--common` 처럼 모듈마다 뜻이 다른 단일 스위치
    pub flag: bool,
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
        Module::RemoveGenes => {
            // `-o` 가 폴더가 아니라 **파일**이다.
            v.push("-i".into());
            v.push(a.input.clone());
            v.push("-g".into());
            v.push(a.genes_list.clone().unwrap_or_default());
            v.push("-o".into());
            v.push(a.output.clone());
            if a.flag {
                v.push("--inverse".into());
            }
        }
        Module::JoinProfiles => {
            // `-p` 뒤에 파일이 여러 개 온다.
            v.push("-p".into());
            v.extend(a.inputs.iter().cloned());
            v.push("-o".into());
            v.push(a.output.clone());
            if a.flag {
                v.push("--common".into());
            }
        }
        Module::SchemaEvaluator => {
            v.push("-g".into());
            v.push(a.schema.clone().unwrap_or_default());
            v.push("-o".into());
            v.push(a.output.clone());
            if a.flag {
                v.push("--loci-reports".into());
            }
        }
        Module::AlleleCallEvaluator => {
            v.push("-i".into());
            v.push(a.input.clone());
            v.push("-g".into());
            v.push(a.schema.clone().unwrap_or_default());
            v.push("-o".into());
            v.push(a.output.clone());
        }
        Module::PrepExternalSchema => {
            // `-g` 는 어셈블리가 아니라 **변환할 스키마 폴더**다.
            v.push("-g".into());
            v.push(a.input.clone());
            v.push("-o".into());
            v.push(a.output.clone());
            if let Some(ptf) = &a.ptf {
                v.push("--ptf".into());
                v.push(ptf.clone());
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

    // `--cpu` 는 모든 모듈에 있는 인자가 아니다. 없는 모듈에 붙이면 argparse 가
    // "unrecognized arguments" 로 즉시 실패한다. 셋 다 `--help` 로 확인했다.
    let no_cpu = matches!(
        module,
        Module::ExtractCgMLST | Module::RemoveGenes | Module::JoinProfiles
    );
    if !no_cpu {
        v.push("--cpu".into());
        v.push(a.cpu.to_string());
    }
    v
}

/// Prodigal training file(`.trn`)을 만드는 인자.
///
/// **`chewBBACA.py` 가 아니다.** 이것만 다른 프로그램을 부르므로 `build_argv` 와
/// 섞지 않는다 — 그쪽의 argv[0] 은 언제나 `chewBBACA.py` 이고, `Module` 은 곧
/// 그 하위 명령이라는 불변식이 있다.
///
/// 부르는 것이 `prodigal` 이 아니라 `pyrodigal` 인 이유: chewBBACA 3.x 는
/// Prodigal 바이너리를 쓰지 않고 pyrodigal 로 갈아탔고, rootfs 에도 그쪽만
/// 들어 있다(`prodigal` 은 없다). 인자와 `.trn` 형식은 서로 호환된다.
pub fn training_argv(genome: &str, output: &str) -> Vec<String> {
    vec![
        "pyrodigal".into(),
        // 학습은 single 모드에서만 일어난다. meta 는 미리 만들어진 모델을 쓴다.
        "-p".into(),
        "single".into(),
        "-i".into(),
        genome.into(),
        "-t".into(),
        output.into(),
        // 유전자 예측 결과(GFF)는 버린다. 필요한 것은 `-t` 가 쓰는 `.trn` 뿐이다.
        "-o".into(),
        "/dev/null".into(),
    ]
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
    fn prep_external_schema_uses_g_for_the_source_schema() {
        // 이 모듈만 `-g` 가 스키마 입력이다. AlleleCall 의 `-g`(대상 스키마)와 뜻이 다르다.
        let mut a = args();
        a.ptf = Some("/home/chewie/x.trn".into());
        let v = build_argv(Module::PrepExternalSchema, &a);
        assert_eq!(v[1], "PrepExternalSchema");
        let g = v.iter().position(|x| x == "-g").unwrap();
        assert_eq!(v[g + 1], a.input);
        assert!(v.contains(&"--ptf".to_string()));
        // 문서로 확인함 — 이 모듈에는 --cpu 가 있다.
        assert!(v.contains(&"--cpu".to_string()));
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
        let t = v
            .iter()
            .position(|x| x == "--t")
            .expect("--t 가 있어야 한다");
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

    #[test]
    fn training_argv_trains_in_single_mode_and_discards_the_gff() {
        let v = training_argv("/mnt/c/g/ref.fna", "/mnt/c/trn/b-fragilis.trn");
        assert_eq!(v[0], "pyrodigal");

        let p = v.iter().position(|x| x == "-p").unwrap();
        assert_eq!(v[p + 1], "single", "meta 모드는 학습을 하지 않는다");

        let t = v.iter().position(|x| x == "-t").unwrap();
        assert_eq!(v[t + 1], "/mnt/c/trn/b-fragilis.trn");

        // `-o` 를 빠뜨리면 예측 결과가 stdout 으로 쏟아진다.
        let o = v.iter().position(|x| x == "-o").unwrap();
        assert_eq!(v[o + 1], "/dev/null");
    }
}
