//! 어셈블리 폴더에서 **training file 학습에 쓸 게놈 하나**를 고른다.
//!
//! 이 파일은 플랫폼 중립이다 (§4.1). 파일을 읽어 세는 것뿐이라 WSL 을 거치지
//! 않는다 — 500개를 9p 너머로 읽으면 같은 일을 몇 배 느리게 하게 된다.
//!
//! **왜 폴더 전체를 학습에 쓰지 않는가:** Prodigal 학습은 게놈 하나(~5Mb)면
//! 통계가 수렴한다. 수백 개를 이어붙여도 모델은 거의 같은 곳으로 가는 반면,
//! 그중 섞여 있을 저품질·오염 어셈블리가 조용히 모델에 들어간다. 얻는 것 없이
//! 위험만 늘어나므로 **가장 완성도 높은 것 하나를 고른다.**

use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};

/// 확장자로 FASTA 를 판정한다. `commands::inspect_input_dir` 와 같은 목록이어야
/// 한다 — 한쪽만 늘리면 "폴더에는 61개인데 후보는 0개" 같은 모순이 보인다.
pub const FASTA_EXT: [&str; 6] = ["fasta", "fa", "fna", "ffn", "faa", "frn"];

/// 학습에 필요한 최소 염기 수.
///
/// Prodigal 계열은 이보다 짧으면 학습이 불안정하다고 경고한다. 참조 게놈 대신
/// 유전자 몇 개짜리 FASTA 를 넣는 사고가 흔해서, 여기서 미리 막는다.
const MIN_BASES: u64 = 100_000;

/// 훑은 게놈 하나의 통계.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenomeStat {
    pub path: String,
    pub file_name: String,
    /// `>` 로 시작하는 줄 수 = contig 수. 완성 게놈이면 1~2 다.
    pub contigs: usize,
    pub bases: u64,
}

/// 폴더 하나를 훑은 결과. 첫 후보가 권장값이다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenomeScan {
    /// 점수 순 상위 후보. UI 는 이것으로 드롭다운을 만들고 사용자가 바꿀 수 있게 한다.
    pub candidates: Vec<GenomeStat>,
    /// 폴더에서 발견한 FASTA 총 개수 (후보로 추려지기 전).
    pub scanned: usize,
    /// 크기 이상치를 걸러내는 기준이 된 중앙값.
    pub median_bases: u64,
    /// 왜 이것이 뽑혔는지 한 줄. UI 와 MCP 가 그대로 보여준다.
    pub reason: String,
}

/// 후보 목록의 상한. 사용자가 훑어볼 수 있는 만큼만 돌려준다.
const MAX_CANDIDATES: usize = 10;

/// 폴더의 FASTA 를 모두 읽어 통계를 내고 순위를 매긴다.
///
/// 파일을 전부 읽어야 하는 이유는 contig 수 때문이다 — 파일 크기만으로는
/// 완성 게놈과 조각난 draft 가 구별되지 않는다(둘 다 총 길이는 비슷하다).
pub fn scan_dir(dir: &Path) -> Result<GenomeScan> {
    crate::paths::validate_host_path(dir)?;
    if !dir.is_dir() {
        return Err(Error::InvalidInput(format!(
            "폴더를 찾을 수 없습니다: {}",
            dir.display()
        )));
    }

    let mut stats = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !FASTA_EXT.contains(&ext.as_str()) {
            continue;
        }
        // 읽다 실패한 파일 하나 때문에 500개 스캔을 버리지 않는다.
        if let Ok(stat) = measure(&path) {
            stats.push(stat);
        }
    }

    if stats.is_empty() {
        return Err(Error::InvalidInput(format!(
            "이 폴더에는 FASTA 파일이 없습니다.\n어셈블리(.fasta/.fna/.fa)가 든 폴더를 고르세요: {}",
            dir.display()
        )));
    }

    rank(stats)
}

/// FASTA 하나의 contig 수와 염기 수를 센다.
///
/// 바이트로 읽는다 — 게놈 FASTA 는 ASCII 지만, 깨진 파일 하나 때문에 UTF-8
/// 검증에서 실패하면 그 파일만 조용히 빠지는 게 아니라 원인도 알 수 없게 된다.
pub fn measure(path: &Path) -> Result<GenomeStat> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut contigs = 0usize;
    let mut bases = 0u64;

    loop {
        line.clear();
        // 서열이 한 줄로 풀려 있는(unwrapped) FASTA 도 있어 줄 길이를 가정하지 않는다.
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if line.first() == Some(&b'>') {
            contigs += 1;
            continue;
        }
        // 개행·공백을 뺀 것이 염기 수다. `*` 나 `-` 같은 것은 드물어 무시한다.
        bases += line.iter().filter(|b| !b.is_ascii_whitespace()).count() as u64;
    }

    Ok(GenomeStat {
        path: path.to_string_lossy().to_string(),
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        contigs,
        bases,
    })
}

/// 통계 목록에서 순위를 매긴다. 파일을 만지지 않으므로 단위 테스트가 된다.
///
/// 판정 순서:
/// 1. 너무 짧은 것을 버린다 (학습 자체가 불안정하다).
/// 2. **중앙값의 ±20% 밖을 버린다.** 종의 기대 게놈 크기를 코드에 박지 않고
///    폴더 자신을 기준으로 삼는 것이 요점이다. plasmid 만 든 파일은 contig 가
///    1개라 이 단계가 없으면 1위로 올라온다.
/// 3. 남은 것 중 contig 가 가장 적은 것, 같으면 가장 긴 것.
fn rank(mut stats: Vec<GenomeStat>) -> Result<GenomeScan> {
    let scanned = stats.len();

    stats.retain(|s| s.bases >= MIN_BASES);
    if stats.is_empty() {
        return Err(Error::InvalidInput(format!(
            "학습에 쓸 만한 게놈이 없습니다 — FASTA {scanned}개가 모두 {}kb 미만입니다.\n유전자 몇 개짜리 FASTA 가 아니라 게놈 어셈블리가 든 폴더를 고르세요.",
            MIN_BASES / 1000
        )));
    }

    let median = median_bases(&stats);
    let lo = median * 8 / 10;
    let hi = median * 12 / 10;
    // 중앙값 자신이 언제나 이 창 안에 있으므로 결과가 비지 않는다 — 폴백이 필요 없다.
    let mut pool: Vec<GenomeStat> = stats
        .into_iter()
        .filter(|s| s.bases >= lo && s.bases <= hi)
        .collect();

    pool.sort_by(|a, b| a.contigs.cmp(&b.contigs).then(b.bases.cmp(&a.bases)));

    let best = pool[0].clone();
    let reason = format!(
        "FASTA {scanned}개 중 contig 가 가장 적은 것을 골랐습니다 — {} (contig {}개, {}).",
        best.file_name,
        best.contigs,
        human_bases(best.bases)
    );

    pool.truncate(MAX_CANDIDATES);
    Ok(GenomeScan {
        candidates: pool,
        scanned,
        median_bases: median,
        reason,
    })
}

fn median_bases(stats: &[GenomeStat]) -> u64 {
    let mut sizes: Vec<u64> = stats.iter().map(|s| s.bases).collect();
    sizes.sort_unstable();
    sizes[sizes.len() / 2]
}

/// 염기 수를 사람이 읽는 단위로. 세균 게놈은 Mb 단위라 소수 둘째 자리면 충분하다.
pub fn human_bases(bases: u64) -> String {
    if bases >= 1_000_000 {
        format!("{:.2} Mb", bases as f64 / 1_000_000.0)
    } else {
        format!("{:.0} kb", bases as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(name: &str, contigs: usize, bases: u64) -> GenomeStat {
        GenomeStat {
            path: format!("C:/g/{name}"),
            file_name: name.to_string(),
            contigs,
            bases,
        }
    }

    #[test]
    fn picks_the_least_fragmented_genome() {
        let scan = rank(vec![
            stat("draft_a.fna", 180, 5_200_000),
            stat("complete.fna", 1, 5_240_000),
            stat("draft_b.fna", 42, 5_180_000),
        ])
        .unwrap();
        assert_eq!(scan.candidates[0].file_name, "complete.fna");
        assert_eq!(scan.scanned, 3);
    }

    #[test]
    fn a_single_contig_plasmid_does_not_win() {
        // **이 테스트가 이 파일의 존재 이유다.** contig 수만 보면 plasmid 가 1위다.
        // 그것으로 학습하면 모델이 통째로 망가지는데 오류는 나지 않는다.
        let scan = rank(vec![
            stat("plasmid.fna", 1, 180_000),
            stat("genome_a.fna", 60, 5_200_000),
            stat("genome_b.fna", 12, 5_210_000),
            stat("genome_c.fna", 90, 5_190_000),
        ])
        .unwrap();
        assert_eq!(scan.candidates[0].file_name, "genome_b.fna");
        assert!(
            !scan.candidates.iter().any(|c| c.file_name == "plasmid.fna"),
            "크기 이상치가 후보에 남았다"
        );
    }

    #[test]
    fn longer_genome_breaks_a_contig_tie() {
        let scan = rank(vec![
            stat("short.fna", 3, 5_000_000),
            stat("long.fna", 3, 5_300_000),
        ])
        .unwrap();
        assert_eq!(scan.candidates[0].file_name, "long.fna");
    }

    #[test]
    fn rejects_a_folder_of_gene_fastas() {
        // 참조 게놈 대신 유전자 FASTA 폴더를 고르는 흔한 사고.
        let err = rank(vec![stat("locus1.fasta", 1, 1_200), stat("locus2.fasta", 1, 900)])
            .unwrap_err();
        assert!(err.to_string().contains("100kb"), "{err}");
    }

    #[test]
    fn wildly_varying_sizes_narrow_to_the_typical_one() {
        // 크기가 제각각인 폴더에서는 중앙값 근처만 남는다. contig 가 더 적은
        // c.fna 가 있어도 9Mb 는 이 종의 게놈으로 보기 어려우므로 후보에서 뺀다.
        // (창 밖의 파일을 굳이 쓰려면 UI 에서 파일을 직접 지정하는 길이 있다.)
        let scan = rank(vec![
            stat("a.fna", 5, 2_000_000),
            stat("b.fna", 9, 5_000_000),
            stat("c.fna", 2, 9_000_000),
        ])
        .unwrap();
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].file_name, "b.fna");
        assert_eq!(scan.scanned, 3, "훑은 개수는 거른 뒤에도 원래 값이어야 한다");
    }

    #[test]
    fn the_size_window_always_keeps_at_least_the_median() {
        // 후보가 0개가 되면 create() 가 pool[0] 에서 패닉한다. 창의 정의상
        // 중앙값은 언제나 남지만, 경계를 손댈 때 깨질 수 있어 못을 박아 둔다.
        for n in 1..=6u64 {
            let stats: Vec<GenomeStat> = (0..n)
                .map(|i| stat(&format!("g{i}.fna"), 1, 1_000_000 * (i + 1)))
                .collect();
            assert!(!rank(stats).unwrap().candidates.is_empty(), "n={n}");
        }
    }

    #[test]
    fn measures_contigs_and_bases_ignoring_newlines() {
        let dir = std::env::temp_dir().join("chewie-fasta-measure");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("g.fna");
        std::fs::write(&path, ">c1 desc\nACGT\nAC\n>c2\nGGGG\n").unwrap();

        let s = measure(&path).unwrap();
        assert_eq!(s.contigs, 2);
        assert_eq!(s.bases, 10);
    }
}
